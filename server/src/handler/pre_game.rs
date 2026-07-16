use crate::{
    handler::matchmaking::{self, JoinQueueOutcome},
    model::messages::{ClientMessage, ServerMessage},
    routes::ws::send_message,
    state::{ServerState, UpgradedWebSocket},
};
use futures_util::{
    StreamExt,
    stream::{SplitSink, SplitStream},
};
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;

pub enum PreGameLoopResult {
    Joined(u64), // match_id
    ClientDisconnected,
    ForceDisconnect,
}

/// Pre-game phase: the client is idle until it sends JoinQueue. Once queued it
/// waits for a pairing (or cancels with LeaveQueue). Pairing outcomes:
/// - Matched immediately: reply MatchFound, return Joined(match_id).
/// - Waiting: reply Queued, then select over the oneshot / LeaveQueue / disconnect.
pub async fn pre_game_loop(
    ws_sender: &mut SplitSink<UpgradedWebSocket, Message>,
    ws_receiver: &mut SplitStream<UpgradedWebSocket>,
    server_state: &ServerState,
    player_id: i64,
    username: String,
    shutdown_rx: &mut mpsc::Receiver<()>,
) -> PreGameLoopResult {
    loop {
        tokio::select! {
            _ = shutdown_rx.recv() => {
                return PreGameLoopResult::ForceDisconnect;
            },
            maybe_msg = ws_receiver.next() => {
                match maybe_msg {
                    Some(Ok(Message::Text(text))) => {
                        if let Ok(ClientMessage::JoinQueue) = serde_json::from_str(&text) {
                            match matchmaking::join_queue(server_state, player_id, username.clone()).await {
                                JoinQueueOutcome::Matched(match_id) => {
                                    let _ = send_message(ws_sender, ServerMessage::MatchFound).await;
                                    return PreGameLoopResult::Joined(match_id);
                                }
                                JoinQueueOutcome::Waiting(mut match_rx) => {
                                    if send_message(ws_sender, ServerMessage::Queued).await.is_err() {
                                        matchmaking::leave_queue(server_state, player_id).await;
                                        return PreGameLoopResult::ClientDisconnected;
                                    }
                                    // Waiting-in-queue inner loop.
                                    loop {
                                        tokio::select! {
                                            _ = shutdown_rx.recv() => {
                                                matchmaking::leave_queue(server_state, player_id).await;
                                                return PreGameLoopResult::ForceDisconnect;
                                            },
                                            result = &mut match_rx => {
                                                match result {
                                                    Ok(match_id) => {
                                                        let _ = send_message(ws_sender, ServerMessage::MatchFound).await;
                                                        return PreGameLoopResult::Joined(match_id);
                                                    }
                                                    // Sender dropped: our entry was replaced
                                                    // (defensive; shouldn't happen for a live
                                                    // connection). Back to idle.
                                                    Err(_) => break,
                                                }
                                            },
                                            maybe_msg = ws_receiver.next() => {
                                                match maybe_msg {
                                                    Some(Ok(Message::Text(text))) => {
                                                        if let Ok(ClientMessage::LeaveQueue) = serde_json::from_str(&text) {
                                                            if matchmaking::leave_queue(server_state, player_id).await {
                                                                break; // back to idle pre-game
                                                            }
                                                            // false: a pairing already took our
                                                            // entry — the match wins; keep
                                                            // waiting for match_rx to fire.
                                                        }
                                                    },
                                                    Some(Ok(_)) => {},
                                                    Some(Err(_)) | None => {
                                                        matchmaking::leave_queue(server_state, player_id).await;
                                                        return PreGameLoopResult::ClientDisconnected;
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    },
                    Some(Ok(_)) => {},
                    Some(Err(_)) | None => return PreGameLoopResult::ClientDisconnected,
                }
            }
        }
    }
}
