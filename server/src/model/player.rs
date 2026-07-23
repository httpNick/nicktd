use bevy_ecs::prelude::Resource;
use serde::Serialize;

use crate::model::family::Family;
use crate::model::unit_kind::UnitKind;

/// ECS Resource wrapping the lobby's player list so systems can read and award gold.
#[derive(Resource, Clone, Debug, Default)]
pub struct Players(pub Vec<Player>);

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Player {
    pub id: i64,
    pub username: String,
    pub gold: u32,
    /// Permanent income awarded to this player at the end of each combat round.
    pub income: u32,
    /// Units queued to be sent to the opponent's board on the next combat phase.
    pub spawning_queue: Vec<UnitKind>,
    /// Current king upgrade tier (0 = base, max 4).
    pub king_tier: u32,
    /// Sends of each shape this wave (Square/Triangle/Circle); resets each wave.
    pub sends_this_wave: [u32; 3],
    /// Price of the NEXT send of each shape — server-computed so the client
    /// displays exactly what will be charged. Index order matches
    /// `unit_config::send_unit_catalog()` (shape_index order): entry `i`
    /// here is the cost for `send_unit_catalog()[i]`.
    pub next_send_costs: [u32; 3],
    /// Number of creeps this player's board has leaked this wave; resets each wave.
    pub leaks_this_wave: u32,
    /// Family locked in for this match on first `PickFamily`; `None` until picked.
    pub family: Option<Family>,
}

impl Player {
    pub fn new(id: i64, username: String, gold: u32) -> Self {
        let mut player = Self {
            id,
            username,
            gold,
            income: 0,
            spawning_queue: Vec::new(),
            king_tier: 0,
            sends_this_wave: [0; 3],
            next_send_costs: [0; 3],
            leaks_this_wave: 0,
            family: None,
        };
        player.refresh_send_costs(1);
        player
    }

    pub fn can_afford(&self, amount: u32) -> bool {
        self.gold >= amount
    }

    pub fn try_spend_gold(&mut self, amount: u32) -> bool {
        if self.can_afford(amount) {
            self.gold -= amount;
            true
        } else {
            false
        }
    }

    pub fn add_gold(&mut self, amount: u32) {
        self.gold += amount;
    }

    /// Recomputes `next_send_costs` from the current wave and counters.
    pub fn refresh_send_costs(&mut self, wave: u32) {
        use crate::model::unit_kind::UnitKind;
        use crate::model::unit_config::{sent_unit_cost, shape_index};
        for shape in [UnitKind::Square, UnitKind::Triangle, UnitKind::Circle] {
            let i = shape_index(shape);
            self.next_send_costs[i] = sent_unit_cost(shape, wave, self.sends_this_wave[i]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_player_has_no_family_picked() {
        let player = Player::new(1, "test".to_string(), 100);
        assert_eq!(player.family, None);
    }

    #[test]
    fn player_new_initialises_king_tier_to_zero() {
        let player = Player::new(1, "test".to_string(), 100);
        assert_eq!(player.king_tier, 0);
    }

    #[test]
    fn player_has_gold_field() {
        let player = Player::new(1, "test".to_string(), 100);
        assert_eq!(player.gold, 100);
    }

    #[test]
    fn player_new_initialises_income_and_queue_to_zero() {
        let player = Player::new(1, "test".to_string(), 100);
        assert_eq!(player.income, 0);
        assert!(player.spawning_queue.is_empty());
    }

    #[test]
    fn player_income_and_spawning_queue_are_mutable() {
        use crate::model::unit_kind::UnitKind;
        let mut player = Player::new(1, "test".to_string(), 100);
        player.income = 5;
        player.spawning_queue.push(UnitKind::Square);
        assert_eq!(player.income, 5);
        assert_eq!(player.spawning_queue.len(), 1);
        assert_eq!(player.spawning_queue[0], UnitKind::Square);
    }

    #[test]
    fn test_player_can_afford() {
        let player = Player::new(1, "test".to_string(), 100);
        assert!(player.can_afford(50));
        assert!(player.can_afford(100));
        assert!(!player.can_afford(101));
    }

    #[test]
    fn test_player_try_spend_gold() {
        let mut player = Player::new(1, "test".to_string(), 100);

        // Success case
        assert!(player.try_spend_gold(40));
        assert_eq!(player.gold, 60);

        // Success case (boundary)
        assert!(player.try_spend_gold(60));
        assert_eq!(player.gold, 0);

        // Failure case
        assert!(!player.try_spend_gold(1));
        assert_eq!(player.gold, 0);
    }

    // --- Task 1 TDD tests ---

    #[test]
    fn new_player_has_base_send_costs_and_zero_counters() {
        let player = Player::new(1, "test".to_string(), 100);
        assert_eq!(player.sends_this_wave, [0, 0, 0]);
        assert_eq!(player.next_send_costs, [8, 20, 50]); // wave 1, n=0
    }

    #[test]
    fn refresh_send_costs_uses_wave_and_counters() {
        let mut player = Player::new(1, "test".to_string(), 100);
        player.sends_this_wave = [1, 0, 0]; // one scout already sent
        player.refresh_send_costs(1);
        assert_eq!(player.next_send_costs, [12, 20, 50]); // ceil(8 × 1.4)
        player.sends_this_wave = [0, 0, 0];
        player.refresh_send_costs(2);
        assert_eq!(player.next_send_costs[0], 10); // ceil(8 × 1.2)
    }
}
