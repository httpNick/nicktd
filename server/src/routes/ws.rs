use crate::{
    database, handler,
    model::{
        jwt,
        messages::{LobbyInfo, ServerMessage},
    },
    state::{ServerState, UpgradedWebSocket},
};
use futures_util::stream::SplitSink;
use futures_util::{SinkExt, StreamExt};
use http_body_util::Full;
use hyper::body::{Bytes, Incoming as Body};
use hyper::{Request, Response, StatusCode};
use log::{error, info};
use std::collections::HashMap;
use tokio_tungstenite::tungstenite::protocol::Message;

pub async fn handle_ws_upgrade(
    req: &mut Request<Body>,
    state: ServerState,
) -> Response<Full<Bytes>> {
    let account_id = match get_account_id_from_req(&req, &state).await {
        Some(id) => id,
        None => {
            return Response::builder()
                .status(StatusCode::UNAUTHORIZED)
                .body(Full::new(Bytes::from("Invalid or missing token")))
                .unwrap();
        }
    };

    if !hyper_tungstenite::is_upgrade_request(req) {
        return Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body(Full::new(Bytes::from("Not a WebSocket upgrade request")))
            .unwrap();
    }

    let (response, websocket) = match hyper_tungstenite::upgrade(req, None) {
        Ok((res, ws)) => (res, ws),
        Err(e) => {
            error!("WebSocket upgrade error: {}", e);
            return Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .body(Full::new(Bytes::from(format!(
                    "Failed to upgrade WebSocket: {}",
                    e
                ))))
                .unwrap();
        }
    };

    tokio::spawn(async move {
        match websocket.await {
            Ok(ws_stream) => {
                handle_connection(ws_stream, state, account_id).await;
            }
            Err(e) => {
                error!("WebSocket upgrade error after handshake: {}", e);
            }
        }
    });

    response
}

async fn get_account_id_from_req(req: &Request<Body>, state: &ServerState) -> Option<i64> {
    let uri = req.uri();
    let query_params: HashMap<_, _> = uri
        .query()
        .map(|q| url::form_urlencoded::parse(q.as_bytes()).into_owned().collect())
        .unwrap_or_default();

    let token = query_params.get("token")?;

    match jwt::decode_jwt(token) {
        Ok(claims) => database::get_account_by_username(&state.db_pool, &claims.sub)
            .await
            .ok()
            .flatten()
            .map(|acc| acc.id),
        Err(_) => None,
    }
}

async fn handle_connection(
    ws_stream: UpgradedWebSocket,
    server_state: ServerState,
    player_id: i64,
) {
    info!("Player {} connected.", player_id);
    let (mut ws_sender, mut ws_receiver) = ws_stream.split();

    if send_message(&mut ws_sender, ServerMessage::PlayerId(player_id))
        .await
        .is_err()
    {
        return;
    }

    loop {
        // on a new connection send the lobby status to the client.
        let lobby_infos: Vec<LobbyInfo> = server_state
            .lobbies
            .lock()
            .await
            .iter()
            .enumerate()
            .map(|(id, lobby)| LobbyInfo {
                id,
                player_count: lobby.players.len(),
            })
            .collect();

        if send_message(&mut ws_sender, ServerMessage::LobbyStatus(lobby_infos))
            .await
            .is_err()
        {
            break;
        }

        let mut lobby_rx = server_state.lobby_tx.subscribe();

        if let Some(lobby_id) = handler::pre_game::pre_game_loop(
            &mut ws_sender,
            &mut ws_receiver,
            &server_state,
            &mut lobby_rx,
            player_id,
        )
        .await
        {
            let result = handler::in_game::in_game_loop(
                &mut ws_sender,
                &mut ws_receiver,
                &server_state,
                lobby_id,
                player_id,
            )
            .await;

            handler::cleanup::cleanup(lobby_id, player_id, &server_state).await;

            match result {
                handler::in_game::InGameLoopResult::PlayerLeft => continue,
                handler::in_game::InGameLoopResult::ClientDisconnected => break,
            }
        } else {
            break;
        }
    }

    info!("Player {} disconnected.", player_id);
}

pub(crate) async fn broadcast_lobby_status(state: &ServerState) {
    let lobbies = state.lobbies.lock().await;
    let lobby_infos: Vec<LobbyInfo> = lobbies
        .iter()
        .enumerate()
        .map(|(id, lobby)| LobbyInfo {
            id,
            player_count: lobby.players.len(),
        })
        .collect();
    let msg = ServerMessage::LobbyStatus(lobby_infos);
    let msg_str = serde_json::to_string(&msg).unwrap();
    let _ = state.lobby_tx.send(msg_str);
}

pub(crate) async fn send_message(
    sender: &mut SplitSink<UpgradedWebSocket, Message>,
    msg: ServerMessage,
) -> Result<(), tokio_tungstenite::tungstenite::Error> {
    let msg_str = serde_json::to_string(&msg).unwrap();
    sender.send(Message::Text(msg_str.into())).await
}
