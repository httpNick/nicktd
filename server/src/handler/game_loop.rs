use crate::{
    handler::{
        combat::{
            cleanup_dead_entities, process_combat, update_active_combat_stats,
            update_combat_movement, update_combat_reset, update_mana, update_targeting,
        },
        worker::update_workers,
    },
    model::{
        components::Position,
        game_state::{DeltaTime, GamePhase, NetworkChannel},
        messages::{CombatEvent, ServerMessage},
        player::Players,
    },
    state::ServerState,
};
use bevy_ecs::message::{MessageReader, Messages};
use bevy_ecs::prelude::{Res, ResMut, Schedule, SystemSet};
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_ecs::schedule::common_conditions::resource_equals;
use std::time::Duration;

/// Bevy system: reads buffered [`CombatEvent`] messages and broadcasts them to all
/// connected clients via the [`NetworkChannel`] resource.
pub fn broadcast_events(mut reader: MessageReader<CombatEvent>, channel: Res<NetworkChannel>) {
    let events: Vec<CombatEvent> = reader.read().cloned().collect();
    if !events.is_empty() {
        let msg = ServerMessage::CombatEvents(events);
        if let Ok(s) = serde_json::to_string(&msg) {
            let _ = channel.0.send(s);
        }
    }
}

/// System sets grouping related game logic for ordered, phase-gated execution.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
enum GameSystemSet {
    /// Advances the `Messages` double-buffer; runs only in Combat phase.
    CombatInit,
    /// Target acquisition and stat updates; runs only in Combat phase.
    Targeting,
    /// Movement and mana regeneration; runs only in Combat phase.
    Movement,
    /// Damage resolution and network broadcast; runs only in Combat phase.
    Damage,
    /// Dead entity cleanup and combat reset; runs only in Combat phase.
    Cleanup,
    /// Worker movement and gold deposit; runs in all phases.
    Workers,
}

/// Bevy system: advances the [`Messages<CombatEvent>`] double-buffer by one tick.
/// Must run exactly once per tick, before any system reads or writes combat messages.
fn update_combat_messages(mut messages: ResMut<Messages<CombatEvent>>) {
    messages.update();
}

/// Builds and returns the main game schedule.
///
/// Combat system sets are chained and gated to [`GamePhase::Combat`].
/// Worker systems run in every phase, after combat cleanup.
pub fn build_main_schedule() -> Schedule {
    let mut schedule = Schedule::default();

    // Chain combat sets in sequential order.
    schedule.configure_sets(
        (
            GameSystemSet::CombatInit,
            GameSystemSet::Targeting,
            GameSystemSet::Movement,
            GameSystemSet::Damage,
            GameSystemSet::Cleanup,
        )
            .chain(),
    );

    // Gate all combat sets to the Combat phase only.
    for set in [
        GameSystemSet::CombatInit,
        GameSystemSet::Targeting,
        GameSystemSet::Movement,
        GameSystemSet::Damage,
        GameSystemSet::Cleanup,
    ] {
        schedule.configure_sets(set.run_if(resource_equals(GamePhase::Combat)));
    }

    // Workers run last, in every phase.
    schedule.configure_sets(GameSystemSet::Workers.after(GameSystemSet::Cleanup));

    // CombatInit: advance the message double-buffer.
    schedule.add_systems(update_combat_messages.in_set(GameSystemSet::CombatInit));

    // Targeting: acquire targets, then update derived stats.
    schedule.add_systems(update_targeting.in_set(GameSystemSet::Targeting));
    schedule.add_systems(
        update_active_combat_stats
            .in_set(GameSystemSet::Targeting)
            .after(update_targeting),
    );

    // Movement: move units, then regenerate mana.
    schedule.add_systems(update_combat_movement.in_set(GameSystemSet::Movement));
    schedule.add_systems(
        update_mana
            .in_set(GameSystemSet::Movement)
            .after(update_combat_movement),
    );

    // Damage: resolve combat, then broadcast resulting events.
    schedule.add_systems(process_combat.in_set(GameSystemSet::Damage));
    schedule.add_systems(
        broadcast_events
            .in_set(GameSystemSet::Damage)
            .after(process_combat),
    );

    // Cleanup: remove dead entities, then reset cooldowns.
    schedule.add_systems(cleanup_dead_entities.in_set(GameSystemSet::Cleanup));
    schedule.add_systems(
        update_combat_reset
            .in_set(GameSystemSet::Cleanup)
            .after(cleanup_dead_entities),
    );

    // Workers: move workers and deposit gold.
    schedule.add_systems(update_workers.in_set(GameSystemSet::Workers));

    schedule
}

pub const TICK_RATE: f32 = 30.0;

pub async fn run_game_loop(server_state: ServerState, lobby_id: usize, generation: u32) {
    let mut interval = tokio::time::interval(Duration::from_secs_f32(1.0 / TICK_RATE));
    let tick_delta = 1.0 / TICK_RATE;
    let mut schedule = build_main_schedule();

    loop {
        interval.tick().await;
        let mut lobbies = server_state.lobbies.lock().await;
        if let Some(lobby) = lobbies.get_mut(lobby_id) {
            // Exit if the lobby has been reset for a new game.
            if lobby.game_generation != generation {
                break;
            }
            // Insert per-tick resources.
            lobby
                .game_state
                .world
                .insert_resource(DeltaTime(tick_delta));
            lobby
                .game_state
                .world
                .insert_resource(lobby.game_state.phase);

            // Build phase orchestration: spawn workers and tick the phase timer.
            if lobby.game_state.phase == GamePhase::Build {
                if lobby.is_full() {
                    let mut worker_query = lobby
                        .game_state
                        .world
                        .query::<&crate::model::components::Worker>();
                    if worker_query.iter(&lobby.game_state.world).count() == 0 {
                        for (idx, player) in lobby.players.iter().enumerate() {
                            for _ in 0..3 {
                                let targets = crate::model::components::TargetPositions {
                                    vein: crate::handler::worker::VEIN_POSITIONS[idx],
                                    cart: crate::handler::worker::CART_POSITIONS[idx],
                                };
                                crate::handler::spawn::spawn_worker(
                                    &mut lobby.game_state.world,
                                    player.id,
                                    targets,
                                );
                            }
                        }
                    }

                    lobby.game_state.phase_timer -= tick_delta;
                    if lobby.game_state.phase_timer <= 0.0 {
                        lobby.game_state.phase_timer = 0.0;
                        lobby.game_state.phase = GamePhase::Combat;
                        // Sync the updated phase into the world immediately so combat
                        // systems run on this same tick.
                        lobby
                            .game_state
                            .world
                            .insert_resource(lobby.game_state.phase);

                        use crate::model::constants::{BOARD_SIZE, RIGHT_BOARD_START};
                        let spawn_x_left = BOARD_SIZE / 2.0;
                        let spawn_x_right = RIGHT_BOARD_START + (BOARD_SIZE / 2.0);

                        let wave_config =
                            crate::handler::wave::get_wave_config(lobby.game_state.wave_number);
                        for x in [spawn_x_left, spawn_x_right] {
                            for shape in &wave_config.enemies {
                                crate::handler::spawn::spawn_enemy(
                                    &mut lobby.game_state.world,
                                    Position { x, y: 30.0 },
                                    *shape,
                                    lobby.game_state.wave_number,
                                );
                            }
                        }

                        // Drain each player's spawning queue and send their units to the
                        // opponent's board.
                        let queues: Vec<Vec<crate::model::shape::Shape>> = lobby
                            .players
                            .iter()
                            .map(|p| p.spawning_queue.clone())
                            .collect();
                        for (player_idx, queue) in queues.iter().enumerate() {
                            let opponent_x = if player_idx == 0 {
                                spawn_x_right
                            } else {
                                spawn_x_left
                            };
                            for &shape in queue {
                                let sent_profile =
                                    crate::model::unit_config::get_sent_unit_profile(shape);
                                crate::handler::spawn::spawn_sent_enemy(
                                    &mut lobby.game_state.world,
                                    Position {
                                        x: opponent_x,
                                        y: 30.0,
                                    },
                                    shape,
                                    lobby.game_state.wave_number,
                                    sent_profile.bounty,
                                );
                            }
                        }
                        for player in &mut lobby.players {
                            player.spawning_queue.clear();
                        }
                    }
                }
            }

            // Sync Players resource, run the main schedule, then sync back.
            lobby
                .game_state
                .world
                .insert_resource(Players(lobby.players.clone()));
            schedule.run(&mut lobby.game_state.world);
            lobby.players = lobby.game_state.world.resource::<Players>().0.clone();

            // Wave completion: transition out of Combat when all enemies are gone.
            if lobby.game_state.phase == GamePhase::Combat
                && check_wave_cleared(&mut lobby.game_state.world)
            {
                if lobby.game_state.wave_number >= 6 {
                    lobby.game_state.phase = GamePhase::Victory;
                } else {
                    lobby.game_state.phase = GamePhase::Build;
                    lobby.game_state.wave_number += 1;
                    lobby.game_state.phase_timer = 30.0;

                    // Award wave completion bonus and permanent income.
                    for player in &mut lobby.players {
                        player.gold += 50;
                        player.gold += player.income;
                    }
                }
            }

            lobby.broadcast_gamestate();
        } else {
            // Lobby no longer exists, stop the loop.
            break;
        }
    }
}

pub fn check_wave_cleared(world: &mut bevy_ecs::prelude::World) -> bool {
    let mut query = world.query::<&crate::model::components::Enemy>();
    query.iter(world).count() == 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::components::Worker;
    use crate::model::game_state::GamePhase;
    use crate::model::lobby::Lobby;
    use crate::model::player::Player;
    use crate::model::shape::Shape;
    use bevy_ecs::prelude::{Entity, With};

    // --- Task 5 TDD: schedule phase-gating and event replay prevention ---

    #[test]
    fn schedule_combat_systems_do_not_run_in_build_phase() {
        use crate::model::components::DamageType;
        use bevy_ecs::prelude::World;

        let mut world = World::new();
        world.init_resource::<Messages<CombatEvent>>();
        world.insert_resource(DeltaTime(1.0 / 30.0));
        world.insert_resource(Players::default());

        let (tx, mut rx) = tokio::sync::broadcast::channel::<String>(16);
        world.insert_resource(NetworkChannel(tx));

        // Set Build phase — combat systems must not run.
        world.insert_resource(GamePhase::Build);

        // Pre-write a combat event; it should not be broadcast in Build phase.
        world
            .resource_mut::<Messages<CombatEvent>>()
            .write(CombatEvent {
                attacker_id: 1,
                target_id: 2,
                attack_type: DamageType::PhysicalBasic,
                start_pos: Position { x: 0.0, y: 0.0 },
                end_pos: Position { x: 10.0, y: 0.0 },
            });

        let mut schedule = build_main_schedule();
        schedule.run(&mut world);

        assert!(
            rx.try_recv().is_err(),
            "No combat events should be broadcast when the phase is Build"
        );
    }

    #[test]
    fn schedule_does_not_replay_combat_events_across_ticks() {
        use crate::model::components::DamageType;
        use bevy_ecs::prelude::World;

        let mut world = World::new();
        world.init_resource::<Messages<CombatEvent>>();
        world.insert_resource(DeltaTime(1.0 / 30.0));
        world.insert_resource(Players::default());

        let (tx, mut rx) = tokio::sync::broadcast::channel::<String>(16);
        world.insert_resource(NetworkChannel(tx));

        // Set Combat phase so combat systems run.
        world.insert_resource(GamePhase::Combat);

        // Write one combat event before the first tick.
        world
            .resource_mut::<Messages<CombatEvent>>()
            .write(CombatEvent {
                attacker_id: 1,
                target_id: 2,
                attack_type: DamageType::PhysicalBasic,
                start_pos: Position { x: 0.0, y: 0.0 },
                end_pos: Position { x: 10.0, y: 0.0 },
            });

        let mut schedule = build_main_schedule();

        // Tick 1: the pre-written event is broadcast exactly once.
        schedule.run(&mut world);
        assert!(
            rx.try_recv().is_ok(),
            "Tick 1 should broadcast the pre-written combat event"
        );

        // Tick 2: no new events written; the previous event must not be replayed.
        schedule.run(&mut world);
        assert!(
            rx.try_recv().is_err(),
            "Tick 2 should not replay the event from tick 1"
        );
    }

    // --- Existing tests (unchanged) ---

    #[test]
    fn test_enemy_spawning_on_both_boards() {
        use crate::model::components::Enemy;
        use crate::model::constants::{BOARD_SIZE, RIGHT_BOARD_START};
        let mut lobby = Lobby::new();
        lobby.players.push(Player::new(1, "p1".into(), 100));
        lobby.players.push(Player::new(2, "p2".into(), 100));
        lobby.game_state.phase_timer = 0.0; // Trigger transition

        // --- SIMULATED TICK START ---
        if lobby.game_state.phase_timer <= 0.0 {
            lobby.game_state.phase = crate::model::game_state::GamePhase::Combat;

            // Spawn one enemy for each board
            let spawn_x_left = BOARD_SIZE / 2.0;
            let spawn_x_right = RIGHT_BOARD_START + (BOARD_SIZE / 2.0);

            for x in [spawn_x_left, spawn_x_right] {
                crate::handler::spawn::spawn_enemy(
                    &mut lobby.game_state.world,
                    Position { x, y: 30.0 },
                    Shape::Triangle,
                    lobby.game_state.wave_number,
                );
            }
        }
        // --- SIMULATED TICK END ---

        let mut query = lobby.game_state.world.query::<(&Enemy, &Position)>();
        let mut left_enemy = false;
        let mut right_enemy = false;
        for (_, pos) in query.iter(&lobby.game_state.world) {
            if pos.x < BOARD_SIZE {
                left_enemy = true;
            }
            if pos.x >= RIGHT_BOARD_START {
                right_enemy = true;
            }
        }
        assert!(left_enemy, "Should spawn enemy on left board");
        assert!(right_enemy, "Should spawn enemy on right board");
    }

    #[test]
    fn test_starting_workers_spawned_when_lobby_full() {
        let mut lobby = Lobby::new();
        lobby.players.push(Player::new(1, "p1".into(), 100));
        lobby.players.push(Player::new(2, "p2".into(), 100));

        assert!(lobby.is_full());

        // We can't easily run run_game_loop in a unit test because it's async and loops forever.
        // But we can extract the logic or test the side effect if we had a "tick" function.
        // For now, I'll simulate a single tick of the Build phase logic here.

        let _tick_delta = 1.0 / TICK_RATE;

        // --- SIMULATED TICK START ---
        if lobby.is_full() {
            // Spawn starting workers once if not already spawned
            let mut worker_query = lobby.game_state.world.query::<&Worker>();
            if worker_query.iter(&lobby.game_state.world).count() == 0 {
                for (idx, player) in lobby.players.iter().enumerate() {
                    for _ in 0..3 {
                        let targets = crate::model::components::TargetPositions {
                            vein: crate::handler::worker::VEIN_POSITIONS[idx],
                            cart: crate::handler::worker::CART_POSITIONS[idx],
                        };
                        crate::handler::spawn::spawn_worker(
                            &mut lobby.game_state.world,
                            player.id,
                            targets,
                        );
                    }
                }
            }
        }
        // --- SIMULATED TICK END ---

        let mut query = lobby.game_state.world.query::<&Worker>();
        let worker_count = query.iter(&lobby.game_state.world).count();
        assert_eq!(
            worker_count, 6,
            "Should spawn 3 workers for each of the 2 players"
        );
    }

    #[test]
    fn test_check_wave_cleared() {
        let mut world = bevy_ecs::prelude::World::new();
        assert!(
            check_wave_cleared(&mut world),
            "Empty world should be cleared"
        );

        crate::handler::spawn::spawn_enemy(
            &mut world,
            Position { x: 0.0, y: 0.0 },
            Shape::Square,
            1,
        );
        assert!(
            !check_wave_cleared(&mut world),
            "World with enemy should not be cleared"
        );
    }

    #[test]
    fn test_combat_to_build_transition() {
        let mut lobby = Lobby::new();
        lobby.game_state.phase = GamePhase::Combat;
        lobby.game_state.wave_number = 1;
        lobby.players.push(Player::new(1, "p1".into(), 100));

        // Simulate combat ending
        // --- SIMULATED TICK START ---
        if lobby.game_state.phase == GamePhase::Combat
            && check_wave_cleared(&mut lobby.game_state.world)
        {
            if lobby.game_state.wave_number >= 6 {
                lobby.game_state.phase = GamePhase::Victory;
            } else {
                lobby.game_state.phase = GamePhase::Build;
                lobby.game_state.wave_number += 1;
                lobby.game_state.phase_timer = 30.0;

                for player in &mut lobby.players {
                    player.gold += 50;
                }
            }
        }
        // --- SIMULATED TICK END ---

        assert_eq!(lobby.game_state.phase, GamePhase::Build);
        assert_eq!(lobby.game_state.wave_number, 2);
        assert_eq!(lobby.game_state.phase_timer, 30.0);
        assert_eq!(lobby.players[0].gold, 150);
    }

    #[test]
    fn test_victory_condition() {
        let mut lobby = Lobby::new();
        lobby.game_state.phase = GamePhase::Combat;
        lobby.game_state.wave_number = 6;

        // Simulate combat ending on wave 6
        // --- SIMULATED TICK START ---
        if lobby.game_state.phase == GamePhase::Combat
            && check_wave_cleared(&mut lobby.game_state.world)
        {
            if lobby.game_state.wave_number >= 6 {
                lobby.game_state.phase = GamePhase::Victory;
            } else {
                lobby.game_state.phase = GamePhase::Build;
                lobby.game_state.wave_number += 1;
                lobby.game_state.phase_timer = 30.0;
            }
        }
        // --- SIMULATED TICK END ---

        assert_eq!(lobby.game_state.phase, GamePhase::Victory);
    }

    #[test]
    fn test_full_game_progression_logic() {
        let mut lobby = Lobby::new();
        lobby.players.push(Player::new(1, "p1".into(), 100));
        lobby.players.push(Player::new(2, "p2".into(), 100));

        // Wave 1 Build phase
        assert_eq!(lobby.game_state.wave_number, 1);
        assert_eq!(lobby.game_state.phase, GamePhase::Build);

        // Transition to Combat
        lobby.game_state.phase_timer = 0.0;
        // (Simulate build phase end)
        if lobby.game_state.phase == GamePhase::Build && lobby.game_state.phase_timer <= 0.0 {
            lobby.game_state.phase = GamePhase::Combat;
            // Spawn enemies
            let wave_config = crate::handler::wave::get_wave_config(lobby.game_state.wave_number);
            for x in [300.0, 1100.0] {
                for shape in &wave_config.enemies {
                    crate::handler::spawn::spawn_enemy(
                        &mut lobby.game_state.world,
                        Position { x, y: 30.0 },
                        *shape,
                        lobby.game_state.wave_number,
                    );
                }
            }
        }
        assert_eq!(lobby.game_state.phase, GamePhase::Combat);
        assert!(!check_wave_cleared(&mut lobby.game_state.world));

        // Clear wave 1
        let mut enemies = lobby
            .game_state
            .world
            .query_filtered::<Entity, With<crate::model::components::Enemy>>();
        let enemy_entities: Vec<Entity> = enemies.iter(&lobby.game_state.world).collect();
        for e in enemy_entities {
            lobby.game_state.world.despawn(e);
        }
        assert!(check_wave_cleared(&mut lobby.game_state.world));

        // Transition to Build (Wave 2)
        if lobby.game_state.phase == GamePhase::Combat
            && check_wave_cleared(&mut lobby.game_state.world)
        {
            if lobby.game_state.wave_number >= 6 {
                lobby.game_state.phase = GamePhase::Victory;
            } else {
                lobby.game_state.phase = GamePhase::Build;
                lobby.game_state.wave_number += 1;
                lobby.game_state.phase_timer = 30.0;
                for player in &mut lobby.players {
                    player.gold += 50;
                }
            }
        }
        assert_eq!(lobby.game_state.phase, GamePhase::Build);
        assert_eq!(lobby.game_state.wave_number, 2);
        assert_eq!(lobby.players[0].gold, 150);

        // Jump to Wave 6 Combat
        lobby.game_state.wave_number = 6;
        lobby.game_state.phase = GamePhase::Combat;
        // Spawn boss
        crate::handler::spawn::spawn_enemy(
            &mut lobby.game_state.world,
            Position { x: 300.0, y: 30.0 },
            Shape::Circle,
            6,
        );
        assert!(!check_wave_cleared(&mut lobby.game_state.world));

        // Clear Boss
        let mut enemies = lobby
            .game_state
            .world
            .query_filtered::<Entity, With<crate::model::components::Enemy>>();
        let enemy_entities: Vec<Entity> = enemies.iter(&lobby.game_state.world).collect();
        for e in enemy_entities {
            lobby.game_state.world.despawn(e);
        }
        assert!(check_wave_cleared(&mut lobby.game_state.world));

        // Transition to Victory
        if lobby.game_state.phase == GamePhase::Combat
            && check_wave_cleared(&mut lobby.game_state.world)
        {
            if lobby.game_state.wave_number >= 6 {
                lobby.game_state.phase = GamePhase::Victory;
            } else {
                lobby.game_state.phase = GamePhase::Build;
                lobby.game_state.wave_number += 1;
            }
        }
        assert_eq!(lobby.game_state.phase, GamePhase::Victory);
    }

    // --- Task 6 TDD: full schedule pipeline integration ---

    /// Verifies that running `build_main_schedule` in Combat phase executes the targeting,
    /// movement, damage, and broadcast pipeline. A unit adjacent to an enemy should acquire
    /// a target and trigger a combat event that is broadcast over the network channel.
    #[test]
    fn schedule_runs_full_combat_pipeline_in_combat_phase() {
        use crate::handler::spawn::{spawn_enemy, spawn_unit};
        use crate::model::components::{Health, Target};
        use crate::model::shape::Shape;
        use bevy_ecs::prelude::{With, World};

        let mut world = World::new();
        world.init_resource::<Messages<CombatEvent>>();
        world.insert_resource(DeltaTime(1.0 / 30.0));
        world.insert_resource(Players::default());

        let (tx, mut rx) = tokio::sync::broadcast::channel::<String>(64);
        world.insert_resource(NetworkChannel(tx));
        world.insert_resource(GamePhase::Combat);

        // Spawn a unit on the left board and an enemy directly next to it so the unit
        // is immediately within attack range and can fire on the first tick.
        let unit = spawn_unit(
            &mut world,
            Position { x: 100.0, y: 300.0 },
            Shape::Square,
            1,
        );
        let enemy = spawn_enemy(
            &mut world,
            Position { x: 120.0, y: 300.0 },
            Shape::Triangle,
            1,
        );

        let mut schedule = build_main_schedule();

        // Tick 1 — advance the message buffer so it is ready for writes, then run combat.
        schedule.run(&mut world);

        // The unit must have acquired the enemy as its target after one tick.
        let target = world.entity(unit).get::<Target>();
        assert!(
            target.is_some(),
            "Unit should acquire enemy as target after one combat tick"
        );
        assert_eq!(
            target.unwrap().0,
            enemy,
            "Unit target should be the spawned enemy"
        );

        // The enemy must still be alive (not immediately killed by a single tick unless
        // attack timer fires; health must exist).
        assert!(
            world.entity(enemy).get::<Health>().is_some(),
            "Enemy entity should still exist after the first tick"
        );

        // Tick 2 — let the attack timer fire so damage is dealt and a CombatEvent is emitted.
        // Run enough ticks until a broadcast arrives (up to 10 ticks @ 30 FPS).
        let mut broadcast_received = rx.try_recv().is_ok();
        for _ in 0..10 {
            if broadcast_received {
                break;
            }
            schedule.run(&mut world);
            broadcast_received = rx.try_recv().is_ok();
        }

        assert!(
            broadcast_received,
            "A CombatEvent broadcast should be emitted over the network channel within 10 ticks"
        );

        // Verify workers still run in Combat phase (Workers system set is not phase-gated).
        let mut worker_query =
            world.query_filtered::<Entity, With<crate::model::components::Worker>>();
        // No workers were spawned in this test; the system must not panic when there are none.
        let _ = worker_query.iter(&world).count();
    }

    /// Verifies that workers move correctly through the schedule in Build phase,
    /// proving the Workers SystemSet runs regardless of GamePhase.
    #[test]
    fn schedule_runs_workers_in_build_phase() {
        use crate::handler::spawn::spawn_worker;
        use crate::handler::worker::{CART_POSITIONS, VEIN_POSITIONS};
        use crate::model::components::{Position as Pos, TargetPositions};
        use bevy_ecs::prelude::World;

        let mut world = World::new();
        world.init_resource::<Messages<CombatEvent>>();
        world.insert_resource(DeltaTime(1.0 / 30.0));
        world.insert_resource(Players(vec![crate::model::player::Player::new(
            1,
            "p1".into(),
            100,
        )]));

        let (tx, _rx) = tokio::sync::broadcast::channel::<String>(16);
        world.insert_resource(NetworkChannel(tx));
        world.insert_resource(GamePhase::Build);

        let targets = TargetPositions {
            vein: VEIN_POSITIONS[0],
            cart: CART_POSITIONS[0],
        };
        let worker = spawn_worker(&mut world, 1, targets);
        let initial_pos = *world.entity(worker).get::<Pos>().unwrap();

        let mut schedule = build_main_schedule();
        schedule.run(&mut world);

        let after_pos = *world.entity(worker).get::<Pos>().unwrap();
        assert!(
            after_pos.y < initial_pos.y || after_pos.x != initial_pos.x,
            "Worker should move during Build phase via the schedule"
        );
    }

    // --- Task 3.2: Offensive unit spawning ---

    #[test]
    fn queued_units_spawn_on_opponent_board_at_combat_start() {
        use crate::model::components::{Bounty, Enemy};
        use crate::model::constants::{BOARD_SIZE, RIGHT_BOARD_START};
        use crate::model::shape::Shape;
        use crate::model::unit_config::{SENT_SQUARE_BOUNTY, get_sent_unit_profile};

        let mut lobby = Lobby::new();
        lobby.players.push(Player::new(1, "p1".into(), 100));
        lobby.players.push(Player::new(2, "p2".into(), 100));

        // Player 0 queues a Square to send to Player 1's board.
        lobby.players[0].spawning_queue.push(Shape::Square);

        let spawn_x_left = BOARD_SIZE / 2.0;
        let spawn_x_right = RIGHT_BOARD_START + (BOARD_SIZE / 2.0);

        // Simulate the queue draining logic from run_game_loop.
        let queues: Vec<Vec<Shape>> = lobby
            .players
            .iter()
            .map(|p| p.spawning_queue.clone())
            .collect();
        for (player_idx, queue) in queues.iter().enumerate() {
            let opponent_x = if player_idx == 0 {
                spawn_x_right
            } else {
                spawn_x_left
            };
            for &shape in queue {
                let sent_profile = get_sent_unit_profile(shape);
                crate::handler::spawn::spawn_sent_enemy(
                    &mut lobby.game_state.world,
                    Position {
                        x: opponent_x,
                        y: 30.0,
                    },
                    shape,
                    lobby.game_state.wave_number,
                    sent_profile.bounty,
                );
            }
        }
        for player in &mut lobby.players {
            player.spawning_queue.clear();
        }

        // Sent enemy should be on the right board (opponent of player 0).
        let mut query = lobby
            .game_state
            .world
            .query::<(&Enemy, &Position, &Bounty)>();
        let enemies: Vec<_> = query.iter(&lobby.game_state.world).collect();
        assert_eq!(enemies.len(), 1, "One sent unit should be spawned");
        assert!(
            enemies[0].1.x >= RIGHT_BOARD_START,
            "Sent unit should spawn on the opponent's (right) board"
        );
        assert_eq!(
            enemies[0].2.0, SENT_SQUARE_BOUNTY,
            "Sent unit should carry the correct bounty"
        );

        // Queue must be cleared.
        assert!(
            lobby.players[0].spawning_queue.is_empty(),
            "Spawning queue should be cleared after combat starts"
        );
    }

    #[test]
    fn both_players_queues_spawn_on_each_others_boards() {
        use crate::model::components::{Bounty, Enemy};
        use crate::model::constants::{BOARD_SIZE, RIGHT_BOARD_START};
        use crate::model::shape::Shape;
        use crate::model::unit_config::get_sent_unit_profile;

        let mut lobby = Lobby::new();
        lobby.players.push(Player::new(1, "p1".into(), 100));
        lobby.players.push(Player::new(2, "p2".into(), 100));

        lobby.players[0].spawning_queue.push(Shape::Square); // Player 0 → right board
        lobby.players[1].spawning_queue.push(Shape::Triangle); // Player 1 → left board

        let spawn_x_left = BOARD_SIZE / 2.0;
        let spawn_x_right = RIGHT_BOARD_START + (BOARD_SIZE / 2.0);

        let queues: Vec<Vec<Shape>> = lobby
            .players
            .iter()
            .map(|p| p.spawning_queue.clone())
            .collect();
        for (player_idx, queue) in queues.iter().enumerate() {
            let opponent_x = if player_idx == 0 {
                spawn_x_right
            } else {
                spawn_x_left
            };
            for &shape in queue {
                let sent_profile = get_sent_unit_profile(shape);
                crate::handler::spawn::spawn_sent_enemy(
                    &mut lobby.game_state.world,
                    Position {
                        x: opponent_x,
                        y: 30.0,
                    },
                    shape,
                    lobby.game_state.wave_number,
                    sent_profile.bounty,
                );
            }
        }
        for player in &mut lobby.players {
            player.spawning_queue.clear();
        }

        let mut query = lobby
            .game_state
            .world
            .query::<(&Enemy, &Position, &Bounty)>();
        let enemies: Vec<_> = query.iter(&lobby.game_state.world).collect();
        assert_eq!(enemies.len(), 2, "Two sent units should be spawned");

        let right_board_enemies = enemies
            .iter()
            .filter(|(_, p, _)| p.x >= RIGHT_BOARD_START)
            .count();
        let left_board_enemies = enemies.iter().filter(|(_, p, _)| p.x < BOARD_SIZE).count();
        assert_eq!(
            right_board_enemies, 1,
            "Player 0's unit should be on right board"
        );
        assert_eq!(
            left_board_enemies, 1,
            "Player 1's unit should be on left board"
        );

        assert!(lobby.players[0].spawning_queue.is_empty());
        assert!(lobby.players[1].spawning_queue.is_empty());
    }

    // --- Task 5.1: End-to-end unit sending flow ---

    /// Traces a complete purchase: gold deducted, income increases, unit queued,
    /// then spawns on the opponent's board alongside the regular wave at combat start,
    /// and the queue is fully drained afterwards.
    #[test]
    fn test_send_unit_end_to_end_flow() {
        use crate::model::components::{Bounty, Enemy};
        use crate::model::constants::{BOARD_SIZE, RIGHT_BOARD_START};
        use crate::model::shape::Shape;
        use crate::model::unit_config::{
            SENT_SQUARE_BOUNTY, SENT_SQUARE_COST, SENT_SQUARE_INCOME, get_sent_unit_profile,
        };

        let mut lobby = Lobby::new();
        lobby.players.push(Player::new(1, "p1".into(), 100));
        lobby.players.push(Player::new(2, "p2".into(), 100));

        // --- STEP 1: Player 0 purchases a Square sent unit ---
        let shape = Shape::Square;
        let profile = get_sent_unit_profile(shape);
        assert!(lobby.players[0].try_spend_gold(profile.send_cost));
        lobby.players[0].spawning_queue.push(shape);
        lobby.players[0].income += profile.income;

        // Verify purchase atomicity
        assert_eq!(
            lobby.players[0].gold,
            100 - SENT_SQUARE_COST,
            "Gold deducted"
        );
        assert_eq!(
            lobby.players[0].income, SENT_SQUARE_INCOME,
            "Income increased"
        );
        assert_eq!(lobby.players[0].spawning_queue.len(), 1, "Unit queued");
        assert_eq!(lobby.players[0].spawning_queue[0], Shape::Square);

        // --- STEP 2: Combat transition — regular wave + queue drain ---
        let spawn_x_left = BOARD_SIZE / 2.0;
        let spawn_x_right = RIGHT_BOARD_START + (BOARD_SIZE / 2.0);

        // Spawn regular wave on both boards
        let wave_config = crate::handler::wave::get_wave_config(lobby.game_state.wave_number);
        for x in [spawn_x_left, spawn_x_right] {
            for &wave_shape in &wave_config.enemies {
                crate::handler::spawn::spawn_enemy(
                    &mut lobby.game_state.world,
                    Position { x, y: 30.0 },
                    wave_shape,
                    lobby.game_state.wave_number,
                );
            }
        }

        // Drain each player's spawning queue to the opponent's board
        let queues: Vec<Vec<Shape>> = lobby
            .players
            .iter()
            .map(|p| p.spawning_queue.clone())
            .collect();
        for (player_idx, queue) in queues.iter().enumerate() {
            let opponent_x = if player_idx == 0 {
                spawn_x_right
            } else {
                spawn_x_left
            };
            for &q_shape in queue {
                let sent_profile = get_sent_unit_profile(q_shape);
                crate::handler::spawn::spawn_sent_enemy(
                    &mut lobby.game_state.world,
                    Position {
                        x: opponent_x,
                        y: 30.0,
                    },
                    q_shape,
                    lobby.game_state.wave_number,
                    sent_profile.bounty,
                );
            }
        }
        for player in &mut lobby.players {
            player.spawning_queue.clear();
        }

        // --- STEP 3: Verify post-combat-start state ---

        // Queue must be fully cleared
        assert!(lobby.players[0].spawning_queue.is_empty(), "Queue drained");
        assert!(
            lobby.players[1].spawning_queue.is_empty(),
            "Opponent queue unchanged"
        );

        // Sent unit must be on the opponent's (right) board with correct bounty
        let mut sent_query = lobby
            .game_state
            .world
            .query::<(&Enemy, &Position, &Bounty)>();
        let bounty_enemies: Vec<_> = sent_query.iter(&lobby.game_state.world).collect();
        assert_eq!(bounty_enemies.len(), 1, "Exactly one sent unit spawned");
        assert!(
            bounty_enemies[0].1.x >= RIGHT_BOARD_START,
            "Sent unit must spawn on opponent's right board"
        );
        assert_eq!(
            bounty_enemies[0].2.0, SENT_SQUARE_BOUNTY,
            "Sent unit carries correct bounty"
        );

        // Regular wave enemies must also be present
        let total_enemies = lobby
            .game_state
            .world
            .query::<&Enemy>()
            .iter(&lobby.game_state.world)
            .count();
        assert!(
            total_enemies > 1,
            "Regular wave enemies must coexist with sent unit"
        );
    }

    // --- Task 5.2: Targeted unit-test coverage ---

    /// Verifies income accumulates across multiple purchases and is awarded at wave end.
    #[test]
    fn task_5_2_income_accumulates_and_awards_at_wave_end() {
        use crate::model::shape::Shape;
        use crate::model::unit_config::{
            SENT_SQUARE_INCOME, SENT_TRIANGLE_INCOME, get_sent_unit_profile,
        };

        let mut lobby = Lobby::new();
        lobby.game_state.phase = GamePhase::Combat;
        lobby.game_state.wave_number = 1;
        lobby.players.push(Player::new(1, "p1".into(), 200));

        // Simulate two purchases accumulating income
        let sq = get_sent_unit_profile(Shape::Square);
        lobby.players[0].try_spend_gold(sq.send_cost);
        lobby.players[0].income += sq.income;
        let tr = get_sent_unit_profile(Shape::Triangle);
        lobby.players[0].try_spend_gold(tr.send_cost);
        lobby.players[0].income += tr.income;

        assert_eq!(
            lobby.players[0].income,
            SENT_SQUARE_INCOME + SENT_TRIANGLE_INCOME,
            "Income should accumulate across purchases"
        );

        // Wave ends — award base bonus + accumulated income
        if lobby.game_state.phase == GamePhase::Combat
            && check_wave_cleared(&mut lobby.game_state.world)
        {
            if lobby.game_state.wave_number < 6 {
                lobby.game_state.phase = GamePhase::Build;
                lobby.game_state.wave_number += 1;
                lobby.game_state.phase_timer = 30.0;
                for player in &mut lobby.players {
                    player.gold += 50;
                    player.gold += player.income;
                }
            }
        }

        let expected =
            200 - sq.send_cost - tr.send_cost + 50 + SENT_SQUARE_INCOME + SENT_TRIANGLE_INCOME;
        assert_eq!(
            lobby.players[0].gold, expected,
            "Gold should include base wave bonus plus accumulated income"
        );
    }

    /// Verifies the spawning queue is fully drained after combat begins,
    /// including when multiple units were queued.
    #[test]
    fn task_5_2_spawning_queue_fully_drained_after_combat() {
        use crate::model::constants::{BOARD_SIZE, RIGHT_BOARD_START};
        use crate::model::shape::Shape;
        use crate::model::unit_config::get_sent_unit_profile;

        let mut lobby = Lobby::new();
        lobby.players.push(Player::new(1, "p1".into(), 200));
        lobby.players.push(Player::new(2, "p2".into(), 200));

        // Queue three units for player 0
        lobby.players[0].spawning_queue.push(Shape::Square);
        lobby.players[0].spawning_queue.push(Shape::Triangle);
        lobby.players[0].spawning_queue.push(Shape::Circle);
        assert_eq!(lobby.players[0].spawning_queue.len(), 3);

        // Simulate queue drain
        let spawn_x_right = RIGHT_BOARD_START + (BOARD_SIZE / 2.0);
        let queues: Vec<Vec<Shape>> = lobby
            .players
            .iter()
            .map(|p| p.spawning_queue.clone())
            .collect();
        for (player_idx, queue) in queues.iter().enumerate() {
            let opponent_x = if player_idx == 0 {
                spawn_x_right
            } else {
                BOARD_SIZE / 2.0
            };
            for &shape in queue {
                let sent_profile = get_sent_unit_profile(shape);
                crate::handler::spawn::spawn_sent_enemy(
                    &mut lobby.game_state.world,
                    Position {
                        x: opponent_x,
                        y: 30.0,
                    },
                    shape,
                    1,
                    sent_profile.bounty,
                );
            }
        }
        for player in &mut lobby.players {
            player.spawning_queue.clear();
        }

        // All three units spawned on the right board
        let spawned = lobby
            .game_state
            .world
            .query::<&crate::model::components::Enemy>()
            .iter(&lobby.game_state.world)
            .count();
        assert_eq!(spawned, 3, "All three queued units must spawn");

        // Queue must be completely empty
        assert!(
            lobby.players[0].spawning_queue.is_empty(),
            "Queue must be fully drained"
        );
    }

    // --- Task 2.1: Income distribution ---

    #[test]
    fn test_income_awarded_at_wave_end() {
        let mut lobby = Lobby::new();
        lobby.game_state.phase = GamePhase::Combat;
        lobby.game_state.wave_number = 1;
        lobby.players.push(Player::new(1, "p1".into(), 100));
        lobby.players.push(Player::new(2, "p2".into(), 100));
        lobby.players[0].income = 5;
        lobby.players[1].income = 3;

        // Wave is clear; simulate the phase transition logic from run_game_loop.
        if lobby.game_state.phase == GamePhase::Combat
            && check_wave_cleared(&mut lobby.game_state.world)
        {
            if lobby.game_state.wave_number >= 6 {
                lobby.game_state.phase = GamePhase::Victory;
            } else {
                lobby.game_state.phase = GamePhase::Build;
                lobby.game_state.wave_number += 1;
                lobby.game_state.phase_timer = 30.0;
                for player in &mut lobby.players {
                    player.gold += 50;
                    player.gold += player.income;
                }
            }
        }

        assert_eq!(
            lobby.players[0].gold, 155,
            "Player 1 should receive 50 base + 5 income = 155 total"
        );
        assert_eq!(
            lobby.players[1].gold, 153,
            "Player 2 should receive 50 base + 3 income = 153 total"
        );
    }

    #[test]
    fn test_income_zero_does_not_affect_wave_bonus() {
        let mut lobby = Lobby::new();
        lobby.game_state.phase = GamePhase::Combat;
        lobby.game_state.wave_number = 1;
        lobby.players.push(Player::new(1, "p1".into(), 100));
        // income defaults to 0

        if lobby.game_state.phase == GamePhase::Combat
            && check_wave_cleared(&mut lobby.game_state.world)
        {
            if lobby.game_state.wave_number < 6 {
                lobby.game_state.phase = GamePhase::Build;
                lobby.game_state.wave_number += 1;
                lobby.game_state.phase_timer = 30.0;
                for player in &mut lobby.players {
                    player.gold += 50;
                    player.gold += player.income;
                }
            }
        }

        assert_eq!(
            lobby.players[0].gold, 150,
            "Player with 0 income should only receive 50 base wave bonus"
        );
    }

    // --- Event System Tests (Task 2) ---

    #[test]
    fn broadcast_events_sends_combat_events_via_network_channel() {
        use crate::model::components::{DamageType, Position};
        use crate::model::messages::CombatEvent;
        use bevy_ecs::message::Messages;
        use bevy_ecs::prelude::World;
        use bevy_ecs::system::RunSystemOnce;

        let mut world = World::new();
        world.init_resource::<Messages<CombatEvent>>();

        let (tx, mut rx) = tokio::sync::broadcast::channel::<String>(16);
        world.insert_resource(NetworkChannel(tx));

        // Write a combat event directly to the Messages resource
        world
            .resource_mut::<Messages<CombatEvent>>()
            .write(CombatEvent {
                attacker_id: 1,
                target_id: 2,
                attack_type: DamageType::PhysicalBasic,
                start_pos: Position { x: 0.0, y: 0.0 },
                end_pos: Position { x: 10.0, y: 0.0 },
            });

        world.run_system_once(broadcast_events).unwrap();

        let msg = rx.try_recv().expect("Expected a broadcast message");
        assert!(
            msg.contains("\"type\":\"CombatEvents\""),
            "Broadcast should contain CombatEvents message"
        );
        assert!(
            msg.contains("\"attacker_id\":1"),
            "Broadcast should contain attacker_id"
        );
    }
}
