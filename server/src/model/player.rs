use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub struct Player {
    pub id: i64,
    pub username: String,
    pub gold: u32,
}

impl Player {
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
        let player = Player { id: 1, username: "test".to_string(), gold: 100 };
        assert_eq!(player.gold, 100);
    }

    #[test]
    fn test_player_can_afford() {
        let player = Player { id: 1, username: "test".to_string(), gold: 100 };
        assert!(player.can_afford(50));
        assert!(player.can_afford(100));
        assert!(!player.can_afford(101));
    }

    #[test]
    fn test_player_try_spend_gold() {
        let mut player = Player { id: 1, username: "test".to_string(), gold: 100 };
        
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
