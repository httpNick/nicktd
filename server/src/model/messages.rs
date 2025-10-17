use serde::{Deserialize, Serialize};
use uuid::Uuid;
use super::shape::Shape;
use super::game_state::GameState;

#[derive(Serialize, Deserialize, Debug)]
pub struct PlaceMessage { pub shape: Shape, pub row: usize, pub col: usize, }

#[derive(Serialize, Deserialize, Debug)]
pub struct SellMessage { pub row: usize, pub col: usize, }

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "action", content = "payload", rename_all = "camelCase")]
pub enum ClientMessage {
    JoinLobby(usize),
    Place(PlaceMessage),
    Sell(SellMessage),
    LeaveLobby,
}

#[derive(Serialize, Clone, Debug)]
pub struct LobbyInfo { pub id: usize, pub player_count: usize, }

#[derive(Serialize, Clone, Debug)]
#[serde(tag = "type", content = "data")]
pub enum ServerMessage {
    LobbyStatus(Vec<LobbyInfo>),
    GameState(GameState),
    PlayerId(Uuid),
    Error(String),
}
