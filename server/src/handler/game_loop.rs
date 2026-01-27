use crate::{
    model::{
        components::Position,
        messages::ServerMessage,
        shape::Shape,
    },
    state::ServerState,
    handler::{
        combat::{update_targeting, update_combat_movement, process_combat, cleanup_dead_entities, update_mana, update_active_combat_stats},
        worker::update_workers,
    },
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
                        lobby.game_state.phase_timer -= tick_delta;
                        if lobby.game_state.phase_timer <= 0.0 {
                            lobby.game_state.phase = crate::model::game_state::GamePhase::Combat;
                            // Spawn one enemy at the top center
                            crate::handler::spawn::spawn_enemy(
                                &mut lobby.game_state.world,
                                Position { x: 300.0, y: 30.0 },
                                Shape::Triangle,
                            );
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
