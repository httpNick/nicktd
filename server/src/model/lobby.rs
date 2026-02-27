use super::components::{
    Dead, Enemy, Health, Mana, PlayerIdComponent, Position, ShapeComponent, Worker, WorkerState,
};
use super::game_state::GameState;
use super::messages::{SerializableGameState, ServerMessage, Unit};
use super::player::Player;
use bevy_ecs::prelude::{Entity, Without};
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
            game_state: GameState::new(),
            players: Vec::new(),
            tx,
        }
    }

    pub fn is_full(&self) -> bool {
        self.players.len() >= 2
    }

    pub fn broadcast_gamestate(&mut self) {
        let mut query = self.game_state.world.query_filtered::<(
            Entity,
            &Position,
            &ShapeComponent,
            Option<&PlayerIdComponent>,
            Option<&Enemy>,
            Option<&Health>,
            Option<&Worker>,
            Option<&Mana>,
            Option<&WorkerState>,
        ), Without<Dead>>();

        let units: Vec<Unit> = query
            .iter(&self.game_state.world)
            .map(
                |(
                    entity,
                    pos,
                    shape,
                    maybe_owner,
                    maybe_enemy,
                    maybe_health,
                    maybe_worker,
                    maybe_mana,
                    maybe_worker_state,
                )| {
                    Unit {
                        id: entity.index(),
                        x: pos.x,
                        y: pos.y,
                        shape: shape.0.clone(),
                        owner_id: maybe_owner.map_or(-1, |owner| owner.0),
                        is_enemy: maybe_enemy.is_some(),
                        current_hp: maybe_health.map_or(100.0, |h| h.current),
                        max_hp: maybe_health.map_or(100.0, |h| h.max),
                        is_worker: maybe_worker.is_some(),
                        current_mana: maybe_mana.map(|m| m.current),
                        max_mana: maybe_mana.map(|m| m.max),
                        worker_state: maybe_worker_state.map(|ws| format!("{ws:?}")),
                    }
                },
            )
            .collect();

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

    pub fn broadcast_message(&self, message: &ServerMessage) {
        if let Ok(msg_str) = serde_json::to_string(message) {
            let _ = self.tx.send(msg_str);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::handler::spawn::{spawn_unit, spawn_worker};
    use crate::model::components::{Dead, Position, TargetPositions};
    use crate::model::shape::Shape;

    #[test]
    fn broadcast_gamestate_includes_entity_id_and_worker_state() {
        let mut lobby = Lobby::new();

        // Spawn a tower and record its entity index
        let tower_entity = spawn_unit(
            &mut lobby.game_state.world,
            Position { x: 100.0, y: 300.0 },
            Shape::Square,
            1,
        );
        let tower_id = tower_entity.index();

        // Spawn a worker starting at cart position (500.0, 50.0)
        let targets = TargetPositions {
            vein: Position { x: 300.0, y: 50.0 },
            cart: Position { x: 500.0, y: 50.0 },
        };
        let worker_entity = spawn_worker(&mut lobby.game_state.world, 1, targets);
        let worker_id = worker_entity.index();

        let mut rx = lobby.tx.subscribe();
        lobby.broadcast_gamestate();

        let msg = rx.try_recv().unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&msg).unwrap();
        let units = parsed["data"]["units"].as_array().unwrap();

        assert_eq!(units.len(), 2);

        let tower_unit = units
            .iter()
            .find(|u| (u["x"].as_f64().unwrap() - 100.0).abs() < 0.01)
            .expect("tower unit not found");
        assert_eq!(
            tower_unit["id"].as_u64().unwrap(),
            tower_id as u64,
            "tower id should match entity index"
        );
        assert!(
            tower_unit["worker_state"].is_null(),
            "tower has no worker_state"
        );

        let worker_unit = units
            .iter()
            .find(|u| (u["x"].as_f64().unwrap() - 500.0).abs() < 0.01)
            .expect("worker unit not found");
        assert_eq!(
            worker_unit["id"].as_u64().unwrap(),
            worker_id as u64,
            "worker id should match entity index"
        );
        assert_eq!(
            worker_unit["worker_state"].as_str().unwrap(),
            "MovingToVein",
            "worker should start in MovingToVein state"
        );
    }

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
        lobby.players.push(Player {
            id: 1,
            username: "test".to_string(),
            gold: 100,
        });

        let mut rx = lobby.tx.subscribe();
        lobby.broadcast_gamestate();

        let msg = rx.try_recv().unwrap();
        assert!(msg.contains("\"players\":"));
        assert!(msg.contains("\"gold\":100"));
    }

    #[test]
    fn dead_tower_excluded_from_broadcast() {
        let mut lobby = Lobby::new();

        // Spawn one living tower and one dead tower on the world
        let _living = spawn_unit(
            &mut lobby.game_state.world,
            Position { x: 100.0, y: 300.0 },
            Shape::Square,
            1,
        );

        let dead = spawn_unit(
            &mut lobby.game_state.world,
            Position { x: 200.0, y: 300.0 },
            Shape::Square,
            1,
        );
        lobby.game_state.world.entity_mut(dead).insert(Dead);

        let mut rx = lobby.tx.subscribe();
        lobby.broadcast_gamestate();

        let msg = rx.try_recv().unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&msg).unwrap();
        let units = parsed["data"]["units"].as_array().unwrap();

        assert_eq!(
            units.len(),
            1,
            "Only the living tower should appear in the broadcast"
        );
        let x = units[0]["x"].as_f64().unwrap();
        assert!(
            (x - 100.0).abs() < 0.01,
            "The living tower (x=100) should be in the broadcast, not the dead one (x=200)"
        );
    }

    #[test]
    fn revived_tower_reappears_in_next_broadcast() {
        let mut lobby = Lobby::new();

        // Spawn a tower and mark it Dead
        let tower = spawn_unit(
            &mut lobby.game_state.world,
            Position { x: 100.0, y: 300.0 },
            Shape::Square,
            1,
        );
        lobby.game_state.world.entity_mut(tower).insert(Dead);

        // First broadcast: tower should be absent
        let mut rx = lobby.tx.subscribe();
        lobby.broadcast_gamestate();
        let msg = rx.try_recv().unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&msg).unwrap();
        let units = parsed["data"]["units"].as_array().unwrap();
        assert_eq!(units.len(), 0, "Dead tower should not appear in broadcast");

        // Revive the tower (remove Dead marker)
        lobby.game_state.world.entity_mut(tower).remove::<Dead>();

        // Second broadcast: tower should reappear at home position with full health
        lobby.broadcast_gamestate();
        let msg2 = rx.try_recv().unwrap();
        let parsed2: serde_json::Value = serde_json::from_str(&msg2).unwrap();
        let units2 = parsed2["data"]["units"].as_array().unwrap();
        assert_eq!(
            units2.len(),
            1,
            "Revived tower should reappear in the next broadcast"
        );
        let unit = &units2[0];
        assert!(
            (unit["x"].as_f64().unwrap() - 100.0).abs() < 0.01,
            "Revived tower should be at its home x position"
        );
        assert!(
            (unit["y"].as_f64().unwrap() - 300.0).abs() < 0.01,
            "Revived tower should be at its home y position"
        );
        let current_hp = unit["current_hp"].as_f64().unwrap();
        let max_hp = unit["max_hp"].as_f64().unwrap();
        assert!(
            (current_hp - max_hp).abs() < 0.01,
            "Revived tower should have full health in the broadcast"
        );
    }
}
