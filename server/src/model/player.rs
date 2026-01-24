use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub struct Player {
    pub id: i64,
    pub username: String,
    pub gold: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn player_has_gold_field() {
        let player = Player { id: 1, username: "test".to_string(), gold: 100 };
        assert_eq!(player.gold, 100);
    }
}
