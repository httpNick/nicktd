use futures_util::{stream::SplitSink, StreamExt, SinkExt};
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{broadcast, Mutex};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::WebSocketStream;
use uuid::Uuid;

mod model;
mod handler;
use model::{lobby::Lobby, messages::{ServerMessage, LobbyInfo}};

const NUM_LOBBIES: usize = 5;
type ServerState = Arc<Mutex<Vec<Lobby>>>;

#[tokio::main]
async fn main() {
    env_logger::init();
    let server_state = Arc::new(Mutex::new((0..NUM_LOBBIES).map(|_| Lobby::new()).collect()));
    let (lobby_tx, _) = broadcast::channel(16);

    let listener = TcpListener::bind("127.0.0.1:9001").await.unwrap();

    while let Ok((stream, _)) = listener.accept().await {
        tokio::spawn(handle_connection(
            stream,
            server_state.clone(),
            lobby_tx.clone(),
        ));
    }
}

async fn handle_connection(
    stream: TcpStream,
    server_state: ServerState,
    lobby_tx: broadcast::Sender<String>,
) {
    let ws_stream = tokio_tungstenite::accept_async(stream).await.unwrap();
    let (mut ws_sender, mut ws_receiver) = ws_stream.split();

    let player_id = Uuid::new_v4();
    send_message(&mut ws_sender, ServerMessage::PlayerId(player_id)).await;

    loop {
        let mut lobby_rx = lobby_tx.subscribe();
        broadcast_lobby_status(&server_state, &lobby_tx).await;

        if let Some(lobby_id) = handler::pre_game::pre_game_loop(&mut ws_sender, &mut ws_receiver, &server_state, &mut lobby_rx, player_id, &lobby_tx).await {
            handler::in_game::in_game_loop(&mut ws_sender, &mut ws_receiver, &server_state, lobby_id, player_id).await;
            handler::cleanup::cleanup(lobby_id, player_id, &server_state, &lobby_tx).await;
        } else {
            break;
        }
    }
}

pub(crate) async fn broadcast_lobby_status(state: &ServerState, lobby_tx: &broadcast::Sender<String>) {
    let lobbies = state.lock().await;
    let lobby_infos: Vec<LobbyInfo> = lobbies.iter().enumerate().map(|(id, lobby)| LobbyInfo { id, player_count: lobby.players.len() }).collect();
    let msg = ServerMessage::LobbyStatus(lobby_infos);
    let msg_str = serde_json::to_string(&msg).unwrap();
    lobby_tx.send(msg_str).unwrap();
}

pub(crate) async fn send_message(sender: &mut SplitSink<WebSocketStream<TcpStream>, Message>, msg: ServerMessage) {
    let msg_str = serde_json::to_string(&msg).unwrap();
    sender.send(Message::Text(msg_str)).await.unwrap();
}
