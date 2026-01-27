use super::game_state::GamePhase;
use super::player::Player;
use super::shape::Shape;
use super::components::{DamageType, Position};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Clone, Debug)]
pub struct CombatEvent {
    pub attacker_id: u32,
    pub target_id: u32,
    pub attack_type: DamageType,
    pub start_pos: Position,
    pub end_pos: Position,
}

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
    pub current_mana: Option<f32>,
    pub max_mana: Option<f32>,
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
    CombatEvents(Vec<CombatEvent>),
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

    #[test]
    fn unit_serialization_includes_mana() {
        let unit = Unit {
            shape: Shape::Circle,
            x: 100.0,
            y: 100.0,
            owner_id: 1,
            is_enemy: false,
            current_hp: 100.0,
            max_hp: 100.0,
            is_worker: false,
            current_mana: Some(50.0),
            max_mana: Some(100.0),
        };

        let json = serde_json::to_string(&unit).unwrap();
        assert!(json.contains("\"current_mana\":50.0"));
        assert!(json.contains("\"max_mana\":100.0"));
    }
}
