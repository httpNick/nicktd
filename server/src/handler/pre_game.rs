use crate::{
    model::{
        messages::{ClientMessage, ServerMessage},
        player::Player,
    },
    routes::ws::{broadcast_lobby_status, send_message},
    state::{ServerState, UpgradedWebSocket},
};
use futures_util::{
    SinkExt, StreamExt,
    stream::{SplitSink, SplitStream},
};
use tokio::sync::{broadcast, mpsc};
use tokio_tungstenite::tungstenite::Message;

pub enum PreGameLoopResult {
    Joined(usize),
    ClientDisconnected,
    ForceDisconnect,
}

pub async fn pre_game_loop(
    ws_sender: &mut SplitSink<UpgradedWebSocket, Message>,
    ws_receiver: &mut SplitStream<UpgradedWebSocket>,
    server_state: &ServerState,
    lobby_rx: &mut broadcast::Receiver<String>,
    player_id: i64,
    username: String,
    shutdown_rx: &mut mpsc::Receiver<()>,
) -> PreGameLoopResult {
    let mut result = PreGameLoopResult::ClientDisconnected;

    loop {
        tokio::select! {
            _ = shutdown_rx.recv() => {
                result = PreGameLoopResult::ForceDisconnect;
                break;
            },
            Ok(msg) = lobby_rx.recv() => {
                if ws_sender.send(Message::Text(msg.into())).await.is_err() {
                    break;
                }
            },
            maybe_msg = ws_receiver.next() => {
                match maybe_msg {
                    Some(Ok(msg)) => {
                        if let Message::Text(text) = msg {
                            if let Ok(ClientMessage::JoinLobby(lobby_id)) = serde_json::from_str(&text) {
                                let mut should_break = false;
                                {
                                    let mut lobbies = server_state.lobbies.lock().await;
                                    if let Some(lobby) = lobbies.get_mut(lobby_id) {
                                        if lobby.players.len() < 2 {
                                            lobby.players.push(Player { id: player_id, username: username.clone(), gold: 100 });
                                            if lobby.players.len() == 2 {
                                                tokio::spawn(crate::handler::game_loop::run_game_loop(server_state.clone(), lobby_id));
                                            }
                                            result = PreGameLoopResult::Joined(lobby_id);
                                            should_break = true;
                                        } else {
                                            let _ = send_message(ws_sender, ServerMessage::Error("Lobby is full".into())).await;
                                        }
                                    } else {
                                        let _ = send_message(ws_sender, ServerMessage::Error("Lobby does not exist".into())).await;
                                    }
                                }

                                if should_break {
                                    broadcast_lobby_status(server_state).await;
                                    break;
                                }
                            }
                        }
                    },
                    Some(Err(_)) | None => {
                        break;
                    }
                }
            }
        }
    }

    result
}
