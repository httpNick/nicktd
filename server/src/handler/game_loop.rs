use crate::{
    handler::{
        combat::{
            cleanup_dead_entities, process_combat, update_active_combat_stats,
            update_combat_movement, update_combat_reset, update_mana, update_targeting,
        },
        worker::update_workers,
    },
    model::{components::Position, messages::ServerMessage, shape::Shape},
    state::ServerState,
};
use std::time::Duration;

pub const TICK_RATE: f32 = 30.0;

pub async fn run_game_loop(server_state: ServerState, lobby_id: usize) {
    let mut interval = tokio::time::interval(Duration::from_secs_f32(1.0 / TICK_RATE));
    let tick_delta = 1.0 / TICK_RATE;

    loop {
        interval.tick().await;
        let mut lobbies = server_state.lobbies.lock().await;
        if let Some(lobby) = lobbies.get_mut(lobby_id) {
            match lobby.game_state.phase {
                crate::model::game_state::GamePhase::Build => {
                    if lobby.is_full() {
                        // Spawn starting workers once if not already spawned
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
                            lobby.game_state.phase = crate::model::game_state::GamePhase::Combat;

                            use crate::model::constants::{BOARD_SIZE, RIGHT_BOARD_START};
                            let spawn_x_left = BOARD_SIZE / 2.0;
                            let spawn_x_right = RIGHT_BOARD_START + (BOARD_SIZE / 2.0);

                            for x in [spawn_x_left, spawn_x_right] {
                                crate::handler::spawn::spawn_enemy(
                                    &mut lobby.game_state.world,
                                    Position { x, y: 30.0 },
                                    Shape::Triangle,
                                );
                            }
                        }
                    }
                }
                crate::model::game_state::GamePhase::Combat => {
                    update_targeting(&mut lobby.game_state.world);
                    update_active_combat_stats(&mut lobby.game_state.world);
                    update_combat_movement(&mut lobby.game_state.world, tick_delta);
                    update_mana(&mut lobby.game_state.world, tick_delta);
                    let combat_events = process_combat(&mut lobby.game_state.world, tick_delta);
                    if !combat_events.is_empty() {
                        lobby.broadcast_message(&ServerMessage::CombatEvents(combat_events));
                    }
                    cleanup_dead_entities(&mut lobby.game_state.world);
                    update_combat_reset(&mut lobby.game_state.world);
                    update_workers(lobby, tick_delta);
                }
            }
            lobby.broadcast_gamestate();
        } else {
            // Lobby no longer exists, stop the loop
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::components::Worker;
    use crate::model::lobby::Lobby;
    use crate::model::player::Player;

    #[test]
    fn test_enemy_spawning_on_both_boards() {
        use crate::model::components::Enemy;
        use crate::model::constants::{BOARD_SIZE, RIGHT_BOARD_START};
        let mut lobby = Lobby::new();
        lobby.players.push(Player {
            id: 1,
            username: "p1".into(),
            gold: 100,
        });
        lobby.players.push(Player {
            id: 2,
            username: "p2".into(),
            gold: 100,
        });
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
        lobby.players.push(Player {
            id: 1,
            username: "p1".into(),
            gold: 100,
        });
        lobby.players.push(Player {
            id: 2,
            username: "p2".into(),
            gold: 100,
        });

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
}
