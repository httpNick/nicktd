use crate::{database, model::game_state::GameState, routes::ws::broadcast_lobby_status, state::ServerState};


pub async fn cleanup(
    lobby_id: usize,
    player_id: i64,
    server_state: &ServerState,
) {
    {
        let mut lobbies = server_state.lobbies.lock().await;
        if let Some(lobby) = lobbies.get_mut(lobby_id) {
            lobby.players.retain(|p| p.id != player_id);
            if lobby.players.is_empty() {
                lobby.game_state = GameState::new();
            }
        }
    }
    if let Err(e) = database::clear_session(&server_state.db_pool, player_id).await {
        log::error!("Failed to clear session for player {}: {}", player_id, e);
    }
    broadcast_lobby_status(server_state).await;
}
