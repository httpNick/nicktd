use crate::{model::game_state::GameState, routes::ws::broadcast_lobby_status, state::ServerState};

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
    broadcast_lobby_status(server_state).await;
}
