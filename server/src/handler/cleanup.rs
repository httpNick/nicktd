use crate::ServerState;
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::broadcast_lobby_status;

pub async fn cleanup(
    lobby_id: usize,
    player_id: Uuid,
    server_state: &ServerState,
    lobby_tx: &broadcast::Sender<String>,
) {
    let mut lobbies = server_state.lock().await;
    if let Some(lobby) = lobbies.get_mut(lobby_id) {
        lobby.players.retain(|p| p.id != player_id);
    }
    broadcast_lobby_status(server_state, lobby_tx).await;
}
