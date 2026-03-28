// All shared message types are now defined in the `common` crate; re-export for backward compatibility.
pub use common::messages::{
    ClientMessage, CombatEvent, LobbyInfo, PlaceMessage, PlayerView, SerializableGameState,
    ServerMessage, Unit, UnitInfoData,
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::components::DamageType;
    use crate::model::components::Position;
    use crate::model::shape::Shape;

    #[test]
    fn deserialize_send_unit() {
        let json = r#"{"action": "sendUnit", "payload": {"shape": "Square"}}"#;
        let msg: ClientMessage = serde_json::from_str(json).unwrap();
        match msg {
            ClientMessage::SendUnit { shape } => assert_eq!(shape, Shape::Square),
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
    fn unit_serialization_includes_mana() {
        let unit = Unit {
            id: 7,
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
            worker_state: None,
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
            shape: Shape::Circle,
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
            damage_type: Some(DamageType::FireMagical),
            armor: None,
            is_boss: false,
            sell_value: Some(56),
        };

        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("\"entity_id\":42"));
        assert!(json.contains("\"attack_damage\":10.0"));
        assert!(json.contains("\"attack_rate\":0.8"));
        assert!(json.contains("\"attack_range\":150.0"));
        assert!(json.contains("\"damage_type\":\"FireMagical\""));
        assert!(json.contains("\"armor\":null"));
        assert!(json.contains("\"is_boss\":false"));
        assert!(json.contains("\"sell_value\":56"));
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

        assert!(json.contains("\"type\":\"CombatEvents\""));
        assert!(json.contains("\"data\":["));
        assert!(json.contains("\"attacker_id\":1"));
        assert!(json.contains("\"attack_type\":\"FireMagical\""));
        assert!(json.contains("\"x\":10.0"));
    }
}
