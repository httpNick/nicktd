use super::components::{
    Dead, Enemy, Health, King, Mana, PlayerIdComponent, Position, ShapeComponent, Worker,
    WorkerState,
};
use super::game_state::{GameState, NetworkChannel};
use super::messages::{
    CombatEvent, GameStateDelta, PhaseInfo, SerializableGameState, ServerMessage, Unit,
};
use super::player::{Player, Players};
use bevy_ecs::message::Messages;
use bevy_ecs::prelude::{Entity, With, Without};
use std::collections::HashMap;
use tokio::sync::broadcast;

pub struct Lobby {
    pub game_state: GameState,
    pub players: Vec<Player>,
    pub tx: broadcast::Sender<String>,
    /// Set when a king dies; `Some(id)` = that player won, `None` = draw.
    pub winner_id: Option<i64>,
    /// Incremented on every broadcast (snapshot or delta) so clients can detect gaps.
    pub seq: u64,
    /// Last-broadcast unit state, keyed by entity bits, used to compute deltas.
    broadcast_cache: HashMap<u64, Unit>,
    /// Last-broadcast player list, used to detect when `players` must be resent.
    last_players: Vec<Player>,
    /// Last-broadcast phase/timer/winner snapshot, used to detect when `phase_info`
    /// must be resent.
    last_phase_info: Option<PhaseInfo>,
}

impl Lobby {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(16);
        let mut game_state = GameState::new();
        game_state.world.init_resource::<Messages<CombatEvent>>();
        game_state.world.insert_resource(NetworkChannel(tx.clone()));
        game_state.world.insert_resource(Players::default());
        Lobby {
            game_state,
            players: Vec::new(),
            tx,
            winner_id: None,
            seq: 0,
            broadcast_cache: HashMap::new(),
            last_players: Vec::new(),
            last_phase_info: None,
        }
    }

    pub fn is_full(&self) -> bool {
        self.players.len() >= 2
    }

    /// Queries the world for every non-dead unit and builds the wire representation.
    /// Extracted so both the full snapshot and the delta path share one query.
    fn snapshot_units(&mut self) -> Vec<Unit> {
        let king_entities: std::collections::HashSet<Entity> = self
            .game_state
            .world
            .query_filtered::<Entity, With<King>>()
            .iter(&self.game_state.world)
            .collect();

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

        query
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
                        id: entity.to_bits(),
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
                        is_king: king_entities.contains(&entity),
                    }
                },
            )
            .collect()
    }

    /// Builds a full-state snapshot, bumps `seq`, and rebaselines the delta cache
    /// from it, so the next `broadcast_changes` call diffs against exactly what was
    /// just built. Used only by `broadcast_gamestate` (which sends it to everyone);
    /// single-client direct replies go through `full_state_message`, which must NOT
    /// bump `seq` or touch the cache.
    fn build_full_state(&mut self) -> SerializableGameState {
        let units = self.snapshot_units();

        self.seq += 1;
        let serializable_state = SerializableGameState {
            units: units.clone(),
            players: self.players.clone(),
            phase: self.game_state.phase,
            phase_timer: self.game_state.phase_timer,
            winner_id: self.winner_id,
            seq: self.seq,
        };

        self.broadcast_cache = units
            .into_iter()
            .map(|mut unit| {
                quantize(&mut unit);
                (unit.id, unit)
            })
            .collect();
        self.last_players = self.players.clone();
        self.last_phase_info = Some(PhaseInfo {
            phase: self.game_state.phase,
            phase_timer: self.game_state.phase_timer.floor(),
            winner_id: self.winner_id,
        });

        serializable_state
    }

    /// Sends the full game state to every subscriber and rebuilds the delta cache
    /// from it, so the next `broadcast_changes` call diffs against exactly what was
    /// just sent.
    pub fn broadcast_gamestate(&mut self) {
        let serializable_state = self.build_full_state();
        let msg = ServerMessage::GameState(serializable_state);
        let msg_str = serde_json::to_string(&msg).unwrap();
        let _ = self.tx.send(msg_str);
    }

    /// Full-state message for ONE client, e.g. lag recovery or a direct
    /// `RequestFullState`. Stamped with the CURRENT `seq` — unlike
    /// `build_full_state`, this does NOT bump `seq` or rebaseline the shared delta
    /// cache/last_players/last_phase_info, because doing so while sending to only
    /// one client would create a permanent seq gap for every other subscriber (who
    /// then resyncs too, bumping seq again, gapping the first client, forever).
    /// The resyncing client sets `lastSeq = seq`; the next broadcast delta is
    /// `seq + 1` and applies contiguously for every client.
    pub fn full_state_message(&mut self) -> ServerMessage {
        let units = self.snapshot_units();
        ServerMessage::GameState(SerializableGameState {
            units,
            players: self.players.clone(),
            phase: self.game_state.phase,
            phase_timer: self.game_state.phase_timer,
            winner_id: self.winner_id,
            seq: self.seq,
        })
    }

    /// Sends only what changed since the last broadcast (snapshot or delta): added,
    /// updated, and removed units, plus `players`/`phase_info` when those changed.
    /// Sends nothing and does not bump `seq` when there is nothing to report.
    pub fn broadcast_changes(&mut self) {
        let mut current = self.snapshot_units();
        for unit in &mut current {
            quantize(unit);
        }

        let mut added = Vec::new();
        let mut updated = Vec::new();
        let mut seen_ids: std::collections::HashSet<u64> =
            std::collections::HashSet::with_capacity(current.len());
        for unit in &current {
            seen_ids.insert(unit.id);
            match self.broadcast_cache.get(&unit.id) {
                None => added.push(unit.clone()),
                Some(cached) if cached != unit => updated.push(unit.clone()),
                Some(_) => {}
            }
        }
        let removed: Vec<u64> = self
            .broadcast_cache
            .keys()
            .filter(|id| !seen_ids.contains(id))
            .copied()
            .collect();

        let players_changed = self.players != self.last_players;

        let current_phase_info = PhaseInfo {
            phase: self.game_state.phase,
            phase_timer: self.game_state.phase_timer.floor(),
            winner_id: self.winner_id,
        };
        let phase_info = if self.last_phase_info.as_ref() != Some(&current_phase_info) {
            Some(current_phase_info)
        } else {
            None
        };

        if added.is_empty()
            && updated.is_empty()
            && removed.is_empty()
            && !players_changed
            && phase_info.is_none()
        {
            return;
        }

        self.seq += 1;
        let delta = GameStateDelta {
            seq: self.seq,
            added: added.clone(),
            updated: updated.clone(),
            removed: removed.clone(),
            players: if players_changed {
                Some(self.players.clone())
            } else {
                None
            },
            phase_info: phase_info.clone(),
        };

        let msg = ServerMessage::GameStateDelta(delta);
        let msg_str = serde_json::to_string(&msg).unwrap();
        let _ = self.tx.send(msg_str);

        for unit in added.into_iter().chain(updated.into_iter()) {
            self.broadcast_cache.insert(unit.id, unit);
        }
        for id in &removed {
            self.broadcast_cache.remove(id);
        }
        if players_changed {
            self.last_players = self.players.clone();
        }
        if let Some(info) = phase_info {
            self.last_phase_info = Some(info);
        }
    }
}

/// Rounds a unit's position to the nearest 0.1 px so f32 movement noise from combat
/// systems never marks a visually-stationary unit as "moved" in the diff.
fn quantize(unit: &mut Unit) {
    unit.x = (unit.x * 10.0).round() / 10.0;
    unit.y = (unit.y * 10.0).round() / 10.0;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::handler::spawn::{spawn_unit, spawn_worker};
    use crate::model::components::{Dead, Position, TargetPositions};
    use crate::model::shape::Shape;

    #[test]
    fn lobby_world_has_combat_event_messages_resource() {
        let lobby = Lobby::new();
        assert!(
            lobby
                .game_state
                .world
                .get_resource::<Messages<CombatEvent>>()
                .is_some(),
            "Messages<CombatEvent> must be registered in the world at lobby creation"
        );
    }

    #[test]
    fn lobby_world_has_network_channel_resource() {
        let lobby = Lobby::new();
        assert!(
            lobby
                .game_state
                .world
                .get_resource::<NetworkChannel>()
                .is_some(),
            "NetworkChannel must be inserted into the world at lobby creation"
        );
    }

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
        let tower_id = tower_entity.to_bits();

        // Spawn a worker starting at cart position (500.0, 50.0)
        let targets = TargetPositions {
            vein: Position { x: 300.0, y: 50.0 },
            cart: Position { x: 500.0, y: 50.0 },
        };
        let worker_entity = spawn_worker(&mut lobby.game_state.world, 1, targets);
        let worker_id = worker_entity.to_bits();

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
            tower_id,
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
            worker_id,
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
        lobby.players.push(Player::new(1, "test".to_string(), 100));

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
    fn broadcast_changes_sends_nothing_when_idle() {
        let mut lobby = Lobby::new();
        lobby.players.push(Player::new(1, "p1".into(), 100));
        spawn_unit(
            &mut lobby.game_state.world,
            Position { x: 100.0, y: 300.0 },
            Shape::Square,
            1,
        );
        let mut rx = lobby.tx.subscribe();

        lobby.broadcast_gamestate(); // baseline snapshot, builds cache
        let _ = rx.try_recv().unwrap();

        lobby.broadcast_changes(); // nothing moved
        assert!(rx.try_recv().is_err(), "idle tick must broadcast nothing");
    }

    #[test]
    fn broadcast_changes_sends_only_the_moved_unit() {
        let mut lobby = Lobby::new();
        lobby.players.push(Player::new(1, "p1".into(), 100));
        let stationary = spawn_unit(
            &mut lobby.game_state.world,
            Position { x: 100.0, y: 300.0 },
            Shape::Square,
            1,
        );
        let mover = spawn_unit(
            &mut lobby.game_state.world,
            Position { x: 200.0, y: 300.0 },
            Shape::Square,
            1,
        );
        let mut rx = lobby.tx.subscribe();
        lobby.broadcast_gamestate();
        let _ = rx.try_recv().unwrap();

        lobby
            .game_state
            .world
            .get_mut::<Position>(mover)
            .unwrap()
            .x = 250.0;
        lobby.broadcast_changes();

        let msg = rx.try_recv().unwrap();
        let v: serde_json::Value = serde_json::from_str(&msg).unwrap();
        assert_eq!(v["type"], "GameStateDelta");
        let updated = v["data"]["updated"].as_array().unwrap();
        assert_eq!(updated.len(), 1);
        assert_eq!(updated[0]["id"].as_u64().unwrap(), mover.to_bits());
        assert!(v["data"]["added"].as_array().unwrap().is_empty());
        assert!(v["data"]["removed"].as_array().unwrap().is_empty());
        let _ = stationary;
    }

    #[test]
    fn broadcast_changes_reports_removed_units() {
        let mut lobby = Lobby::new();
        lobby.players.push(Player::new(1, "p1".into(), 100));
        let e = spawn_unit(
            &mut lobby.game_state.world,
            Position { x: 100.0, y: 300.0 },
            Shape::Square,
            1,
        );
        let mut rx = lobby.tx.subscribe();
        lobby.broadcast_gamestate();
        let _ = rx.try_recv().unwrap();

        lobby.game_state.world.despawn(e);
        lobby.broadcast_changes();

        let msg = rx.try_recv().unwrap();
        let v: serde_json::Value = serde_json::from_str(&msg).unwrap();
        assert_eq!(v["data"]["removed"][0].as_u64().unwrap(), e.to_bits());
    }

    #[test]
    fn delta_seq_increments_and_snapshot_rebaselines() {
        let mut lobby = Lobby::new();
        lobby.players.push(Player::new(1, "p1".into(), 100));
        let e = spawn_unit(
            &mut lobby.game_state.world,
            Position { x: 100.0, y: 300.0 },
            Shape::Square,
            1,
        );
        let mut rx = lobby.tx.subscribe();

        lobby.broadcast_gamestate(); // seq 1 (snapshot)
        let snap: serde_json::Value = serde_json::from_str(&rx.try_recv().unwrap()).unwrap();
        let snap_seq = snap["data"]["seq"].as_u64().unwrap();

        lobby.game_state.world.get_mut::<Position>(e).unwrap().x = 150.0;
        lobby.broadcast_changes(); // seq 2 (delta)
        let delta: serde_json::Value = serde_json::from_str(&rx.try_recv().unwrap()).unwrap();
        assert_eq!(delta["data"]["seq"].as_u64().unwrap(), snap_seq + 1);
    }

    #[test]
    fn broadcast_changes_sends_phase_info_only_on_whole_second_or_transition() {
        let mut lobby = Lobby::new();
        lobby.players.push(Player::new(1, "p1".into(), 100));
        // Start mid-second: 30.0 -> 29.99 would cross floor(30)->floor(29) immediately
        // and make the "no broadcast" assertion wrong. 29.5 avoids the boundary.
        lobby.game_state.phase_timer = 29.5;
        let mut rx = lobby.tx.subscribe();
        lobby.broadcast_gamestate();
        let _ = rx.try_recv().unwrap();

        lobby.game_state.phase_timer -= 0.01; // 29.49: same whole second
        lobby.broadcast_changes();
        assert!(
            rx.try_recv().is_err(),
            "sub-second timer tick must not broadcast"
        );

        lobby.game_state.phase_timer -= 1.0; // 28.49: crosses a whole second
        lobby.broadcast_changes();
        let v: serde_json::Value = serde_json::from_str(&rx.try_recv().unwrap()).unwrap();
        assert!(v["data"]["phase_info"].is_object());
    }

    #[test]
    fn full_state_message_does_not_consume_a_seq_slot() {
        let mut lobby = Lobby::new();
        lobby.players.push(Player::new(1, "p1".into(), 100));
        let e = spawn_unit(
            &mut lobby.game_state.world,
            Position { x: 100.0, y: 300.0 },
            Shape::Square,
            1,
        );

        // Two subscribers standing in for the two clients in the lobby.
        let mut rx_a = lobby.tx.subscribe();
        let mut rx_b = lobby.tx.subscribe();

        // Baseline snapshot both clients receive normally.
        lobby.broadcast_gamestate();
        let baseline: serde_json::Value =
            serde_json::from_str(&rx_a.try_recv().unwrap()).unwrap();
        let _ = rx_b.try_recv().unwrap();
        let baseline_seq = baseline["data"]["seq"].as_u64().unwrap();

        // Simulate a direct resync reply to client A only (not broadcast).
        let direct_msg = lobby.full_state_message();
        let direct_json = serde_json::to_value(&direct_msg).unwrap();
        assert_eq!(
            direct_json["data"]["seq"].as_u64().unwrap(),
            baseline_seq,
            "direct snapshot must be stamped with the current seq, not a bumped one"
        );

        // Neither client should have received anything on the broadcast channel
        // from the direct snapshot call.
        assert!(rx_a.try_recv().is_err());
        assert!(rx_b.try_recv().is_err());

        // Now mutate a unit and broadcast a delta as the game loop normally would.
        lobby.game_state.world.get_mut::<Position>(e).unwrap().x = 150.0;
        lobby.broadcast_changes();

        let delta_a: serde_json::Value = serde_json::from_str(&rx_a.try_recv().unwrap()).unwrap();
        let delta_b: serde_json::Value = serde_json::from_str(&rx_b.try_recv().unwrap()).unwrap();
        assert_eq!(
            delta_a["data"]["seq"].as_u64().unwrap(),
            baseline_seq + 1,
            "the direct snapshot must not have consumed a seq slot"
        );
        assert_eq!(
            delta_b["data"]["seq"].as_u64().unwrap(),
            baseline_seq + 1,
            "both subscribers must see the same contiguous seq"
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
