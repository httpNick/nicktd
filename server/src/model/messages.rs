use serde::{Deserialize, Serialize};
use uuid::Uuid;
use super::shape::Shape;
use super::game_state::GamePhase;

#[derive(Serialize, Deserialize, Debug)]
pub struct PlaceMessage { pub shape: Shape, pub row: u32, pub col: u32, }

#[derive(Serialize, Deserialize, Debug)]
pub struct SellMessage { pub row: u32, pub col: u32, }

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "action", content = "payload", rename_all = "camelCase")]
pub enum ClientMessage {
    JoinLobby(usize),
    Place(PlaceMessage),
    Sell(SellMessage),
    SkipToCombat,
    LeaveLobby,
}

#[derive(Serialize, Clone, Debug)]
pub struct LobbyInfo { pub id: usize, pub player_count: usize, }

#[derive(Serialize, Clone, Debug)]
pub struct Unit {
    pub shape: Shape,
    pub x: f32,
    pub y: f32,
    pub owner_id: Uuid,
    pub is_enemy: bool,
}

#[derive(Serialize, Clone, Debug)]
pub struct SerializableGameState {
    pub units: Vec<Unit>,
    pub phase: GamePhase,
    pub phase_timer: f32,
}

#[derive(Serialize, Clone, Debug)]
#[serde(tag = "type", content = "data")]
pub enum ServerMessage {
    LobbyStatus(Vec<LobbyInfo>),
    GameState(SerializableGameState),
    PlayerId(Uuid),
    Error(String),
}
