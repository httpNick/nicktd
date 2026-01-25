use super::game_state::GamePhase;
use super::player::Player;
use super::shape::Shape;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct PlaceMessage {
    pub shape: Shape,
    pub row: u32,
    pub col: u32,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SellMessage {
    pub row: u32,
    pub col: u32,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "action", content = "payload", rename_all = "camelCase")]
pub enum ClientMessage {
    JoinLobby(usize),
    Place(PlaceMessage),
    Sell(SellMessage),
    SkipToCombat,
    LeaveLobby,
    HireWorker {},
}

#[derive(Serialize, Clone, Debug)]
pub struct LobbyInfo {
    pub id: usize,
    pub player_count: usize,
}

#[derive(Serialize, Clone, Debug)]
pub struct Unit {
    pub shape: Shape,
    pub x: f32,
    pub y: f32,
    pub owner_id: i64,
    pub is_enemy: bool,
    pub current_hp: f32,
    pub max_hp: f32,
    pub is_worker: bool,
}

#[derive(Serialize, Clone, Debug)]
pub struct SerializableGameState {
    pub units: Vec<Unit>,
    pub players: Vec<Player>,
    pub phase: GamePhase,
    pub phase_timer: f32,
}

#[derive(Serialize, Clone, Debug)]
#[serde(tag = "type", content = "data")]
pub enum ServerMessage {
    LobbyStatus(Vec<LobbyInfo>),
    GameState(SerializableGameState),
    PlayerId(i64),
    Error(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_hire_worker() {
        let json = r#"{"action": "hireWorker", "payload": {}}"#;
        let msg: ClientMessage = serde_json::from_str(json).unwrap();
        match msg {
            ClientMessage::HireWorker {} => assert!(true),
            _ => panic!("Wrong message type"),
        }
    }
}
