pub mod components;
pub mod game_phase;
pub mod messages;
pub mod shape;

pub use components::{DamageType, Position};
pub use game_phase::GamePhase;
pub use shape::Shape;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::messages::{
        ClientMessage, CombatEvent, LobbyInfo, PlayerView, SerializableGameState, ServerMessage,
        Unit, UnitInfoData,
    };

    // ── Position / Shape / DamageType (from task 1.1) ────────────────────────

    #[test]
    fn position_serializes_and_deserializes() {
        let pos = Position { x: 1.5, y: 2.5 };
        let json = serde_json::to_string(&pos).unwrap();
        let restored: Position = serde_json::from_str(&json).unwrap();
        assert_eq!(pos, restored);
    }

    #[test]
    fn shape_serializes_to_string() {
        assert_eq!(serde_json::to_string(&Shape::Circle).unwrap(), "\"Circle\"");
        assert_eq!(serde_json::to_string(&Shape::Square).unwrap(), "\"Square\"");
        assert_eq!(
            serde_json::to_string(&Shape::Triangle).unwrap(),
            "\"Triangle\""
        );
    }

    #[test]
    fn shape_deserializes_from_string() {
        let s: Shape = serde_json::from_str("\"Circle\"").unwrap();
        assert_eq!(s, Shape::Circle);
    }

    #[test]
    fn damage_type_serializes_to_string() {
        assert_eq!(
            serde_json::to_string(&DamageType::PhysicalPierce).unwrap(),
            "\"PhysicalPierce\""
        );
        assert_eq!(
            serde_json::to_string(&DamageType::PhysicalBasic).unwrap(),
            "\"PhysicalBasic\""
        );
        assert_eq!(
            serde_json::to_string(&DamageType::FireMagical).unwrap(),
            "\"FireMagical\""
        );
    }

    #[test]
    fn damage_type_deserializes_from_string() {
        let d: DamageType = serde_json::from_str("\"FireMagical\"").unwrap();
        assert_eq!(d, DamageType::FireMagical);
    }

    #[test]
    fn position_is_component() {
        use bevy_ecs::prelude::World;
        let mut world = World::new();
        let entity = world.spawn(Position { x: 3.0, y: 4.0 }).id();
        let pos = world.get::<Position>(entity).unwrap();
        assert_eq!(pos.x, 3.0);
        assert_eq!(pos.y, 4.0);
    }

    // ── GamePhase ─────────────────────────────────────────────────────────────

    // RED: GamePhase variants serialize to expected strings
    #[test]
    fn game_phase_serializes_correctly() {
        assert_eq!(
            serde_json::to_string(&GamePhase::Build).unwrap(),
            "\"Build\""
        );
        assert_eq!(
            serde_json::to_string(&GamePhase::Combat).unwrap(),
            "\"Combat\""
        );
        assert_eq!(
            serde_json::to_string(&GamePhase::Victory).unwrap(),
            "\"Victory\""
        );
    }

    // RED: GamePhase deserializes from string
    #[test]
    fn game_phase_deserializes_correctly() {
        let p: GamePhase = serde_json::from_str("\"Combat\"").unwrap();
        assert_eq!(p, GamePhase::Combat);
    }

    // RED: GamePhase can be inserted as a Bevy Resource
    #[test]
    fn game_phase_is_resource() {
        use bevy_ecs::prelude::World;
        let mut world = World::new();
        world.insert_resource(GamePhase::Build);
        let phase = world.get_resource::<GamePhase>().unwrap();
        assert_eq!(*phase, GamePhase::Build);
    }

    // ── PlayerView ────────────────────────────────────────────────────────────

    // RED: PlayerView serializes with gold field
    #[test]
    fn player_view_serializes_with_gold() {
        let pv = PlayerView {
            id: 1,
            username: "alice".to_string(),
            gold: 150,
            income: 5,
        };
        let json = serde_json::to_string(&pv).unwrap();
        assert!(json.contains("\"gold\":150"));
        assert!(json.contains("\"income\":5"));
        assert!(json.contains("\"username\":\"alice\""));
    }

    // RED: PlayerView round-trip serialization
    #[test]
    fn player_view_round_trip() {
        let pv = PlayerView {
            id: 42,
            username: "bob".to_string(),
            gold: 0,
            income: 0,
        };
        let json = serde_json::to_string(&pv).unwrap();
        let restored: PlayerView = serde_json::from_str(&json).unwrap();
        assert_eq!(pv.id, restored.id);
        assert_eq!(pv.username, restored.username);
        assert_eq!(pv.gold, restored.gold);
    }

    // ── ClientMessage ─────────────────────────────────────────────────────────

    // RED: ClientMessage::SendUnit deserializes from camelCase JSON
    #[test]
    fn client_message_send_unit_deserializes() {
        let json = r#"{"action":"sendUnit","payload":{"shape":"Square"}}"#;
        let msg: ClientMessage = serde_json::from_str(json).unwrap();
        match msg {
            ClientMessage::SendUnit { shape } => assert_eq!(shape, Shape::Square),
            _ => panic!("wrong variant"),
        }
    }

    // RED: ClientMessage::HireWorker deserializes
    #[test]
    fn client_message_hire_worker_deserializes() {
        let json = r#"{"action":"hireWorker","payload":{}}"#;
        let msg: ClientMessage = serde_json::from_str(json).unwrap();
        assert!(matches!(msg, ClientMessage::HireWorker {}));
    }

    // RED: ClientMessage::Place deserializes with PlaceMessage payload
    #[test]
    fn client_message_place_deserializes() {
        let json = r#"{"action":"place","payload":{"shape":"Circle","row":2,"col":3}}"#;
        let msg: ClientMessage = serde_json::from_str(json).unwrap();
        match msg {
            ClientMessage::Place(pm) => {
                assert_eq!(pm.shape, Shape::Circle);
                assert_eq!(pm.row, 2);
                assert_eq!(pm.col, 3);
            }
            _ => panic!("wrong variant"),
        }
    }

    // ── ServerMessage ─────────────────────────────────────────────────────────

    // RED: ServerMessage serializes with type/data tag format
    #[test]
    fn server_message_lobby_status_serializes_with_type_tag() {
        let msg = ServerMessage::LobbyStatus(vec![LobbyInfo {
            id: 1,
            player_count: 2,
        }]);
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"LobbyStatus\""));
        assert!(json.contains("\"data\":"));
    }

    // RED: ServerMessage::Error serializes correctly
    #[test]
    fn server_message_error_serializes() {
        let msg = ServerMessage::Error("oops".to_string());
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"Error\""));
        assert!(json.contains("\"oops\""));
    }

    // RED: ServerMessage deserializes from JSON (frontend needs this)
    #[test]
    fn server_message_error_deserializes() {
        let json = r#"{"type":"Error","data":"fail"}"#;
        let msg: ServerMessage = serde_json::from_str(json).unwrap();
        match msg {
            ServerMessage::Error(s) => assert_eq!(s, "fail"),
            _ => panic!("wrong variant"),
        }
    }

    // ── CombatEvent ───────────────────────────────────────────────────────────

    // RED: CombatEvent serializes all fields
    #[test]
    fn combat_event_serializes_all_fields() {
        let event = CombatEvent {
            attacker_id: 1,
            target_id: 2,
            attack_type: DamageType::FireMagical,
            start_pos: Position { x: 10.0, y: 10.0 },
            end_pos: Position { x: 20.0, y: 20.0 },
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"attacker_id\":1"));
        assert!(json.contains("\"attack_type\":\"FireMagical\""));
        assert!(json.contains("\"x\":10.0"));
    }

    // ── SerializableGameState ─────────────────────────────────────────────────

    // RED: SerializableGameState uses Vec<PlayerView>
    #[test]
    fn serializable_game_state_serializes_player_views() {
        let state = SerializableGameState {
            units: vec![],
            players: vec![PlayerView {
                id: 1,
                username: "nick".to_string(),
                gold: 200,
                income: 10,
            }],
            phase: GamePhase::Build,
            phase_timer: 25.0,
        };
        let json = serde_json::to_string(&state).unwrap();
        assert!(json.contains("\"gold\":200"));
        assert!(json.contains("\"phase\":\"Build\""));
        assert!(json.contains("\"phase_timer\":25.0"));
    }

    // RED: Unit serializes with id and shape
    #[test]
    fn unit_serializes_correctly() {
        let unit = Unit {
            id: 5,
            shape: Shape::Circle,
            x: 100.0,
            y: 200.0,
            owner_id: 1,
            is_enemy: false,
            current_hp: 100.0,
            max_hp: 100.0,
            is_worker: false,
            current_mana: None,
            max_mana: None,
            worker_state: None,
        };
        let json = serde_json::to_string(&unit).unwrap();
        assert!(json.contains("\"id\":5"));
        assert!(json.contains("\"shape\":\"Circle\""));
    }

    // RED: UnitInfoData serializes with optional fields
    #[test]
    fn unit_info_data_serializes() {
        let info = UnitInfoData {
            entity_id: 7,
            attack_damage: Some(15.0),
            attack_rate: None,
            attack_range: None,
            damage_type: Some(DamageType::PhysicalPierce),
            armor: None,
            is_boss: false,
            sell_value: Some(30),
        };
        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("\"entity_id\":7"));
        assert!(json.contains("\"attack_damage\":15.0"));
        assert!(json.contains("\"damage_type\":\"PhysicalPierce\""));
    }
}
