use super::game_state::GameState;
use super::player::Player;
use super::messages::ServerMessage;
use tokio::sync::broadcast;

pub struct Lobby {
    pub game_state: GameState,
    pub players: Vec<Player>,
    pub tx: broadcast::Sender<String>,
}

impl Lobby {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(16);
        Lobby {
            game_state: GameState::default(),
            players: Vec::new(),
            tx,
        }
    }

    pub fn broadcast_gamestate(&self) {
        let msg = ServerMessage::GameState(self.game_state.clone());
        let msg_str = serde_json::to_string(&msg).unwrap();
        self.tx.send(msg_str).unwrap();
    }
}
