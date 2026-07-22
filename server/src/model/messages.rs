use super::components::{DamageType, Position};
use super::family::Family;
use super::game_state::GamePhase;
use super::player::Player;
use super::unit_kind::UnitKind;
use bevy_ecs::message::Message;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Clone, Debug, Message)]
pub struct CombatEvent {
    pub attacker_id: u64,
    pub target_id: u64,
    pub attack_type: DamageType,
    pub start_pos: Position,
    pub end_pos: Position,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct PlaceMessage {
    pub shape: UnitKind,
    pub row: u32,
    pub col: u32,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "action", content = "payload", rename_all = "camelCase")]
pub enum ClientMessage {
    Place(PlaceMessage),
    SellById {
        entity_id: u64,
    },
    SkipToCombat,
    LeaveLobby,
    HireWorker {},
    RequestUnitInfo {
        entity_id: u64,
    },
    SendUnit {
        shape: UnitKind,
    },
    UpgradeKing {},
    /// Client detected a seq gap (missed a delta) and asks for a direct resync.
    RequestFullState,
    JoinQueue,
    LeaveQueue,
    PickFamily {
        family: Family,
    },
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Unit {
    /// Full entity bits (index + generation) so stale IDs never match recycled entities.
    pub id: u64,
    pub shape: UnitKind,
    pub x: f32,
    pub y: f32,
    pub owner_id: i64,
    pub is_enemy: bool,
    pub current_hp: f32,
    pub max_hp: f32,
    pub is_worker: bool,
    pub current_mana: Option<f32>,
    pub max_mana: Option<f32>,
    pub worker_state: Option<String>,
    pub is_king: bool,
}

#[derive(Serialize, Clone, Debug)]
pub struct UnitInfoData {
    pub entity_id: u64,
    pub attack_damage: Option<f32>,
    pub attack_rate: Option<f32>,
    pub attack_range: Option<f32>,
    pub damage_type: Option<DamageType>,
    pub armor: Option<f32>,
    pub is_boss: bool,
    pub sell_value: Option<u32>,
}

#[derive(Serialize, Clone, Debug)]
pub struct SerializableGameState {
    pub units: Vec<Unit>,
    pub players: Vec<Player>,
    pub phase: GamePhase,
    pub phase_timer: f32,
    pub winner_id: Option<i64>,
    pub seq: u64,
}

/// Snapshot of the fields a client needs to detect a phase/timer/winner change.
/// `phase_timer` is stored floored to the whole second so sub-second ticks don't
/// spuriously mark this "changed" (see the diff rule in `Lobby::broadcast_changes`).
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct PhaseInfo {
    pub phase: GamePhase,
    pub phase_timer: f32,
    pub winner_id: Option<i64>,
}

/// Delta-compressed game state broadcast: only units that were added, changed, or
/// removed since the last broadcast (snapshot or delta). `players`/`phase_info` are
/// only populated when they actually changed.
#[derive(Serialize, Clone, Debug)]
pub struct GameStateDelta {
    pub seq: u64,
    pub added: Vec<Unit>,
    pub updated: Vec<Unit>,
    pub removed: Vec<u64>,
    pub players: Option<Vec<Player>>,
    pub phase_info: Option<PhaseInfo>,
}

/// One entry in the server-driven mercenary send catalog. Sent to the client
/// once per match (right after `MatchFound`) so the Mercenary Panel can be
/// built purely from server data — adding a new sendable unit requires no
/// client change. See `send_unit_catalog` doc comment for the order
/// contract with `Player::next_send_costs`.
#[derive(Serialize, Clone, Debug)]
pub struct SendUnitCatalogEntry {
    pub shape: UnitKind,
    pub name: &'static str,
    pub base_cost: u32,
    pub income: u32,
    pub bounty: u32,
}

/// One entry in the server-sent build catalog for the picking player's
/// family. Sent as `ServerMessage::BuildCatalog` right after a successful
/// `PickFamily`. The client builds its shop buttons purely from this list —
/// adding a tower to a family requires no client change.
#[derive(Serialize, Clone, Debug)]
pub struct BuildCatalogEntry {
    pub unit_kind: UnitKind,
    pub name: &'static str,
    pub cost: u32,
}

#[derive(Serialize, Clone, Debug)]
#[serde(tag = "type", content = "data")]
pub enum ServerMessage {
    GameState(SerializableGameState),
    GameStateDelta(GameStateDelta),
    CombatEvents(Vec<CombatEvent>),
    PlayerId(i64),
    Error(String),
    UnitInfo(UnitInfoData),
    /// Ack: the player is in the matchmaking queue ("searching…").
    Queued,
    /// A match was created; the client should proceed to the game screen.
    MatchFound,
    /// Server-driven mercenary send catalog, sent once right after
    /// `MatchFound`. Order matches `Player::next_send_costs` by index.
    SendUnitCatalog(Vec<SendUnitCatalogEntry>),
    /// Families the player may pick from, sent once right after `MatchFound`.
    FamilyOptions(Vec<Family>),
    /// Server-driven build catalog for the picking player's chosen family,
    /// sent once in reply to a successful `PickFamily`.
    BuildCatalog(Vec<BuildCatalogEntry>),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_upgrade_king() {
        let json = r#"{"action": "upgradeKing", "payload": {}}"#;
        let msg: ClientMessage = serde_json::from_str(json).unwrap();
        match msg {
            ClientMessage::UpgradeKing {} => assert!(true),
            _ => panic!("Wrong message type"),
        }
    }

    #[test]
    fn serializable_game_state_includes_winner_id() {
        use crate::model::game_state::GamePhase;
        let state = SerializableGameState {
            units: vec![],
            players: vec![],
            phase: GamePhase::Build,
            phase_timer: 0.0,
            winner_id: Some(42),
            seq: 1,
        };
        let json = serde_json::to_string(&state).unwrap();
        assert!(json.contains("\"winner_id\":42"));
    }

    #[test]
    fn deserialize_send_unit() {
        let json = r#"{"action": "sendUnit", "payload": {"shape": "Square"}}"#;
        let msg: ClientMessage = serde_json::from_str(json).unwrap();
        match msg {
            ClientMessage::SendUnit { shape } => assert_eq!(shape, UnitKind::Square),
            _ => panic!("Wrong message type"),
        }
    }

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
    fn deserialize_request_full_state() {
        let json = r#"{"action":"requestFullState"}"#;
        let msg: ClientMessage = serde_json::from_str(json).unwrap();
        match msg {
            ClientMessage::RequestFullState => assert!(true),
            _ => panic!("Wrong message type"),
        }
    }

    #[test]
    fn unit_serialization_includes_mana() {
        let unit = Unit {
            id: 7,
            shape: UnitKind::Circle,
            x: 100.0,
            y: 100.0,
            owner_id: 1,
            is_enemy: false,
            current_hp: 100.0,
            max_hp: 100.0,
            is_worker: false,
            current_mana: Some(50.0),
            max_mana: Some(100.0),
            worker_state: None,
            is_king: false,
        };

        let json = serde_json::to_string(&unit).unwrap();
        assert!(json.contains("\"id\":7"));
        assert!(json.contains("\"current_mana\":50.0"));
        assert!(json.contains("\"max_mana\":100.0"));
    }

    #[test]
    fn unit_serialization_includes_id_and_worker_state() {
        let unit = Unit {
            id: 5,
            shape: UnitKind::Circle,
            x: 650.0,
            y: 50.0,
            owner_id: 1,
            is_enemy: false,
            current_hp: 100.0,
            max_hp: 100.0,
            is_worker: true,
            current_mana: None,
            max_mana: None,
            worker_state: Some("Mining".into()),
            is_king: false,
        };

        let json = serde_json::to_string(&unit).unwrap();
        assert!(json.contains("\"id\":5"));
        assert!(json.contains("\"worker_state\":\"Mining\""));
    }

    #[test]
    fn unit_info_data_serialization() {
        let info = UnitInfoData {
            entity_id: 42,
            attack_damage: Some(10.0),
            attack_rate: Some(0.8),
            attack_range: Some(150.0),
            damage_type: Some(DamageType::FIRE_MAGICAL),
            armor: None,
            is_boss: false,
            sell_value: Some(56),
        };

        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("\"entity_id\":42"));
        assert!(json.contains("\"attack_damage\":10.0"));
        assert!(json.contains("\"attack_rate\":0.8"));
        assert!(json.contains("\"attack_range\":150.0"));
        assert!(json.contains("\"damage_type\":{\"school\":\"Magical\",\"element\":\"Fire\"}"));
        assert!(json.contains("\"armor\":null"));
        assert!(json.contains("\"is_boss\":false"));
        assert!(json.contains("\"sell_value\":56"));
    }

    #[test]
    fn combat_events_serialization_format() {
        let event = CombatEvent {
            attacker_id: 1,
            target_id: 2,
            attack_type: DamageType::FIRE_MAGICAL,
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
        assert!(json.contains("\"attack_type\":{\"school\":\"Magical\",\"element\":\"Fire\"}"));
        assert!(json.contains("\"x\":10.0"));
    }

    #[test]
    fn deserialize_join_queue() {
        let json = r#"{"action":"joinQueue"}"#;
        let msg: ClientMessage = serde_json::from_str(json).unwrap();
        assert!(matches!(msg, ClientMessage::JoinQueue));
    }

    #[test]
    fn deserialize_leave_queue() {
        let json = r#"{"action":"leaveQueue"}"#;
        let msg: ClientMessage = serde_json::from_str(json).unwrap();
        assert!(matches!(msg, ClientMessage::LeaveQueue));
    }

    #[test]
    fn serialize_queued_and_match_found() {
        assert_eq!(
            serde_json::to_string(&ServerMessage::Queued).unwrap(),
            r#"{"type":"Queued"}"#
        );
        assert_eq!(
            serde_json::to_string(&ServerMessage::MatchFound).unwrap(),
            r#"{"type":"MatchFound"}"#
        );
    }

    #[test]
    fn deserialize_pick_family() {
        use crate::model::family::Family;
        let json = r#"{"action":"pickFamily","payload":{"family":"Basic"}}"#;
        let msg: ClientMessage = serde_json::from_str(json).unwrap();
        match msg {
            ClientMessage::PickFamily { family } => assert_eq!(family, Family::Basic),
            _ => panic!("Wrong message type"),
        }
    }

    #[test]
    fn serialize_family_options() {
        use crate::model::family::Family;
        let msg = ServerMessage::FamilyOptions(vec![Family::Basic]);
        let json = serde_json::to_string(&msg).unwrap();
        assert_eq!(json, r#"{"type":"FamilyOptions","data":["Basic"]}"#);
    }

    #[test]
    fn serialize_build_catalog() {
        let msg = ServerMessage::BuildCatalog(vec![BuildCatalogEntry {
            unit_kind: UnitKind::Square,
            name: "Square",
            cost: 25,
        }]);
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.starts_with(r#"{"type":"BuildCatalog","data":["#));
        assert!(json.contains(r#""unit_kind":"Square""#));
        assert!(json.contains(r#""name":"Square""#));
        assert!(json.contains(r#""cost":25"#));
    }

    #[test]
    fn serialize_send_unit_catalog() {
        let msg = ServerMessage::SendUnitCatalog(vec![SendUnitCatalogEntry {
            shape: UnitKind::Square,
            name: "Scout",
            base_cost: 8,
            income: 1,
            bounty: 6,
        }]);
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.starts_with(r#"{"type":"SendUnitCatalog","data":["#));
        assert!(json.contains(r#""shape":"Square""#));
        assert!(json.contains(r#""name":"Scout""#));
        assert!(json.contains(r#""base_cost":8"#));
        assert!(json.contains(r#""income":1"#));
        assert!(json.contains(r#""bounty":6"#));
    }
}
