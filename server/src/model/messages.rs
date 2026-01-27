use super::components::{DamageType, Position};
use super::game_state::GamePhase;
use super::player::Player;
use super::shape::Shape;
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

    #[test]
    fn combat_events_serialization_format() {
        let event = CombatEvent {
            attacker_id: 1,
            target_id: 2,
            attack_type: DamageType::FireMagical,
            start_pos: Position { x: 10.0, y: 10.0 },
            end_pos: Position { x: 20.0, y: 20.0 },
        };
        let msg = ServerMessage::CombatEvents(vec![event]);
        let json = serde_json::to_string(&msg).unwrap();

        // Check for correct message type tag
        assert!(json.contains("\"type\":\"CombatEvents\""));
        // Check data content exists
        assert!(json.contains("\"data\":["));
        // Check specific fields
        assert!(json.contains("\"attacker_id\":1"));
        assert!(json.contains("\"attack_type\":\"FireMagical\""));
        assert!(json.contains("\"x\":10.0"));
    }
}
