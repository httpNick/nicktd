use bevy_ecs::prelude::Resource;
use serde::Serialize;

use crate::model::shape::Shape;

/// ECS Resource wrapping the lobby's player list so systems can read and award gold.
#[derive(Resource, Clone, Debug, Default)]
pub struct Players(pub Vec<Player>);

#[derive(Clone, Debug, Serialize)]
pub struct Player {
    pub id: i64,
    pub username: String,
    pub gold: u32,
    /// Permanent income awarded to this player at the end of each combat round.
    pub income: u32,
    /// Units queued to be sent to the opponent's board on the next combat phase.
    pub spawning_queue: Vec<Shape>,
}

impl Player {
    pub fn new(id: i64, username: String, gold: u32) -> Self {
        Self {
            id,
            username,
            gold,
            income: 0,
            spawning_queue: Vec::new(),
        }
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
}

#[cfg(test)]
mod tests {
    use super::*;

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
        use crate::model::shape::Shape;
        let mut player = Player::new(1, "test".to_string(), 100);
        player.income = 5;
        player.spawning_queue.push(Shape::Square);
        assert_eq!(player.income, 5);
        assert_eq!(player.spawning_queue.len(), 1);
        assert_eq!(player.spawning_queue[0], Shape::Square);
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
}
