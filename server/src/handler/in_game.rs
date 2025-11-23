use crate::model::components::{PlayerIdComponent, Position, ShapeComponent};
use crate::model::messages::ClientMessage;
use crate::ServerState;
use bevy_ecs::prelude::Entity;
use futures_util::{stream::SplitSink, SinkExt, StreamExt};
use tokio::net::TcpStream;
use tokio_tungstenite::{tungstenite::Message, WebSocketStream};
use uuid::Uuid;

const SQUARE_SIZE: u32 = 60;

pub async fn in_game_loop(
    ws_sender: &mut SplitSink<WebSocketStream<TcpStream>, Message>,
    ws_receiver: &mut futures_util::stream::SplitStream<WebSocketStream<TcpStream>>,
    server_state: &ServerState,
    lobby_id: usize,
    player_id: Uuid,
) {
    let mut game_rx = {
        let lobbies = server_state.lock().await;
        lobbies[lobby_id].tx.subscribe()
    };
    server_state.lock().await[lobby_id].broadcast_gamestate();

    loop {
        tokio::select! {
            maybe_msg = ws_receiver.next() => {
                match maybe_msg {
                    Some(Ok(msg)) => {
                        if let Message::Text(text) = msg {
                            if let Ok(client_msg) = serde_json::from_str::<ClientMessage>(&text) {
                                let mut lobbies = server_state.lock().await;
                                let lobby = &mut lobbies[lobby_id];
                                match client_msg {
                                    ClientMessage::Place(p) => {
                                        let x = (p.col * SQUARE_SIZE) as f32 + (SQUARE_SIZE as f32 / 2.0);
                                        let y = (p.row * SQUARE_SIZE) as f32 + (SQUARE_SIZE as f32 / 2.0);

                                        lobby.game_state.world.spawn((
                                            Position { x, y },
                                            ShapeComponent(p.shape),
                                            PlayerIdComponent(player_id),
                                        ));
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
                                    ClientMessage::LeaveLobby => break,
                                    _ => {}
                                }
                            }
                        }
                    },
                    Some(Err(_)) | None => break,
                }
            },
            Ok(msg) = game_rx.recv() => {
                if ws_sender.send(Message::Text(msg)).await.is_err() { break; }
            }
        }
    }
}
