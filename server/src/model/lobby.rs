use super::game_state::GameState;
use super::player::Player;
use super::messages::{ServerMessage, Unit, SerializableGameState};
use tokio::sync::broadcast;
use super::components::{Enemy, PlayerIdComponent, Position, ShapeComponent};

pub struct Lobby {
    pub game_state: GameState,
    pub players: Vec<Player>,
    pub tx: broadcast::Sender<String>,
}

impl Lobby {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(16);
        Lobby {
            game_state: GameState::new(),
            players: Vec::new(),
            tx,
        }
    }

    pub fn is_full(&self) -> bool {
        self.players.len() >= 2
    }

    pub fn broadcast_gamestate(&mut self) {
        let mut query = self.game_state.world.query::<(&Position, &ShapeComponent, Option<&PlayerIdComponent>, Option<&Enemy>)>();
        let units: Vec<Unit> = query.iter(&self.game_state.world).map(|(pos, shape, maybe_owner, maybe_enemy)| {
            Unit {
                x: pos.x,
                y: pos.y,
                shape: shape.0.clone(),
                owner_id: maybe_owner.map_or(-1, |owner| owner.0),
                is_enemy: maybe_enemy.is_some(),
            }
        }).collect();

        let serializable_state = SerializableGameState {
            units,
            players: self.players.clone(),
            phase: self.game_state.phase,
            phase_timer: self.game_state.phase_timer,
        };

        let msg = ServerMessage::GameState(serializable_state);
        let msg_str = serde_json::to_string(&msg).unwrap();
        let _ = self.tx.send(msg_str);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn broadcast_gamestate_after_receiver_is_dropped_does_not_panic() {
        let mut lobby = Lobby::new();
        let rx = lobby.tx.subscribe();
        drop(rx); // the player leaves
        lobby.broadcast_gamestate();
    }

    #[test]
    fn broadcast_gamestate_includes_players_and_gold() {
        let mut lobby = Lobby::new();
        lobby.players.push(Player { id: 1, username: "test".to_string(), gold: 100 });
        
        let mut rx = lobby.tx.subscribe();
        lobby.broadcast_gamestate();
        
        let msg = rx.try_recv().unwrap();
        assert!(msg.contains("\"players\":"));
        assert!(msg.contains("\"gold\":100"));
    }
}
