use crate::model::{messages::ClientMessage, placed_shape::PlacedShape, player::Player};
use crate::ServerState;
use futures_util::{stream::SplitSink, SinkExt, StreamExt};
use tokio::net::TcpStream;
use tokio::sync::broadcast;
use tokio_tungstenite::{tungstenite::Message, WebSocketStream};
use uuid::Uuid;

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
                                        lobby.game_state.shapes.push(PlacedShape { shape: p.shape, row: p.row, col: p.col, owner_id: player_id });
                                        lobby.broadcast_gamestate();
                                    }
                                    ClientMessage::Sell(s) => {
                                        lobby.game_state.shapes.retain(|shape| !(shape.row == s.row && shape.col == s.col && shape.owner_id == player_id));
                                        lobby.broadcast_gamestate();
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
