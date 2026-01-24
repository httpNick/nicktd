use crate::{
    model::{
        components::{PlayerIdComponent, Position, TargetPositions},
        messages::ClientMessage,
    },
    state::{ServerState, UpgradedWebSocket},
};
use bevy_ecs::prelude::Entity;
use futures_util::{
    stream::{SplitSink, SplitStream},
    SinkExt, StreamExt,
};
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;

const SQUARE_SIZE: u32 = 60;

pub enum InGameLoopResult {
    PlayerLeft,
    ClientDisconnected,
    ForceDisconnect,
}

pub async fn in_game_loop(
    ws_sender: &mut SplitSink<UpgradedWebSocket, Message>,
    ws_receiver: &mut SplitStream<UpgradedWebSocket>,
    server_state: &ServerState,
    lobby_id: usize,
    player_id: i64,
    shutdown_rx: &mut mpsc::Receiver<()>,
) -> InGameLoopResult {
    let mut game_rx = {
        let lobbies = server_state.lobbies.lock().await;
        lobbies[lobby_id].tx.subscribe()
    };
    server_state.lobbies.lock().await[lobby_id].broadcast_gamestate();

    loop {
        tokio::select! {
            _ = shutdown_rx.recv() => {
                break InGameLoopResult::ForceDisconnect;
            },
            maybe_msg = ws_receiver.next() => {
                match maybe_msg {
                    Some(Ok(msg)) => {
                        if let Message::Text(text) = msg {
                            if let Ok(client_msg) = serde_json::from_str::<ClientMessage>(&text) {
                                let mut lobbies = server_state.lobbies.lock().await;
                                let lobby = &mut lobbies[lobby_id];
                                match client_msg {
                                    ClientMessage::Place(p) => {
                                        let x = (p.col * SQUARE_SIZE) as f32 + (SQUARE_SIZE as f32 / 2.0);
                                        let y = (p.row * SQUARE_SIZE) as f32 + (SQUARE_SIZE as f32 / 2.0);

                                        crate::handler::spawn::spawn_unit(
                                            &mut lobby.game_state.world,
                                            Position { x, y },
                                            p.shape,
                                            player_id,
                                        );
                                        lobby.broadcast_gamestate();
                                    }
                                    ClientMessage::Sell(s) => {
                                        let mut entity_to_sell: Option<Entity> = None;
                                        let x_min = (s.col * SQUARE_SIZE) as f32;
                                        let x_max = x_min + SQUARE_SIZE as f32;
                                        let y_min = (s.row * SQUARE_SIZE) as f32;
                                        let y_max = y_min + SQUARE_SIZE as f32;

                                        let mut query = lobby.game_state.world.query::<(Entity, &Position, &PlayerIdComponent)>();
                                        for (entity, position, owner) in query.iter(&lobby.game_state.world) {
                                            if position.x >= x_min && position.x < x_max && position.y >= y_min && position.y < y_max && owner.0 == player_id {
                                                entity_to_sell = Some(entity);
                                                break;
                                            }
                                        }

                                        if let Some(entity) = entity_to_sell {
                                            lobby.game_state.world.despawn(entity);
                                            lobby.broadcast_gamestate();
                                        }
                                    }
                                    ClientMessage::SkipToCombat => {
                                        lobby.game_state.phase_timer = 0.0;
                                    }
                                    ClientMessage::HireWorker {} => {
                                        let player_idx = lobby.players.iter().position(|p| p.id == player_id);
                                        if let Some(idx) = player_idx {
                                            if lobby.players[idx].gold >= 50 {
                                                lobby.players[idx].gold -= 50;
                                                
                                                let targets = TargetPositions {
                                                    vein: crate::handler::worker::VEIN_POSITIONS[idx],
                                                    cart: crate::handler::worker::CART_POSITIONS[idx],
                                                };

                                                crate::handler::spawn::spawn_worker(
                                                    &mut lobby.game_state.world,
                                                    player_id,
                                                    targets,
                                                );
                                                lobby.broadcast_gamestate();
                                            }
                                        }
                                    }
                                    ClientMessage::LeaveLobby => break InGameLoopResult::PlayerLeft,
                                    _ => {}
                                }
                            }
                        }
                    },
                    Some(Err(_)) | None => break InGameLoopResult::ClientDisconnected,
                }
            },
            Ok(msg) = game_rx.recv() => {
                if ws_sender.send(Message::Text(msg.into())).await.is_err() { break InGameLoopResult::ClientDisconnected; }
            }
        }
    }
}
