use crate::model::{messages::{ClientMessage, ServerMessage}, player::Player};
use crate::ServerState;
use futures_util::{stream::SplitSink, SinkExt, StreamExt};
use tokio::net::TcpStream;
use tokio::sync::broadcast;
use tokio_tungstenite::{tungstenite::Message, WebSocketStream};
use uuid::Uuid;

use crate::send_message;

pub async fn pre_game_loop(
    ws_sender: &mut SplitSink<WebSocketStream<TcpStream>, Message>,
    ws_receiver: &mut futures_util::stream::SplitStream<WebSocketStream<TcpStream>>,
    server_state: &ServerState,
    lobby_rx: &mut broadcast::Receiver<String>,
    player_id: Uuid,
    lobby_tx: &broadcast::Sender<String>,
) -> Option<usize> {
    let mut lobby_id_opt: Option<usize> = None;

    loop {
        tokio::select! {
            Ok(msg) = lobby_rx.recv() => {
                if ws_sender.send(Message::Text(msg)).await.is_err() {
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
                                    let mut lobbies = server_state.lock().await;
                                    if let Some(lobby) = lobbies.get_mut(lobby_id) {
                                        if lobby.players.len() < 2 {
                                            lobby.players.push(Player { id: player_id });
                                            if lobby.players.len() == 2 {
                                                tokio::spawn(crate::handler::game_loop::run_game_loop(server_state.clone(), lobby_id));
                                            }
                                            lobby_id_opt = Some(lobby_id);
                                            should_break = true;
                                        } else {
                                            send_message(ws_sender, ServerMessage::Error("Lobby is full".into())).await;
                                        }
                                    } else {
                                        send_message(ws_sender, ServerMessage::Error("Lobby does not exist".into())).await;
                                    }
                                }

                                if should_break {
                                    crate::broadcast_lobby_status(server_state, lobby_tx).await;
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

    lobby_id_opt
}
