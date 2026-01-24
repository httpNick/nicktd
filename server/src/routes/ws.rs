use crate::{
    database, handler,
    model::{
        account::Account,
        jwt,
        messages::{LobbyInfo, ServerMessage},
    },
    state::{ServerState, UpgradedWebSocket},
};
use chrono::Utc;
use futures_util::stream::SplitSink;
use futures_util::{SinkExt, StreamExt};
use http_body_util::Full;
use hyper::body::{Bytes, Incoming as Body};
use hyper::{Request, Response, StatusCode};
use log::error;
use std::collections::HashMap;
use tokio_tungstenite::tungstenite::protocol::Message;

pub async fn handle_ws_upgrade(
    req: &mut Request<Body>,
    state: ServerState,
) -> Response<Full<Bytes>> {
    let authenticated_account = match authenticate_websocket_request(&req, &state).await {
        Ok(account) => account,
        Err(e) => {
            return Response::builder()
                .status(StatusCode::UNAUTHORIZED)
                .body(Full::new(Bytes::from(format!(
                    "Unauthorized: {}",
                    e
                ))))
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
                handle_connection(ws_stream, state, authenticated_account.id, authenticated_account.username).await;
            }
            Err(e) => {
                error!("WebSocket upgrade error after handshake: {}", e);
            }
        }
    });

    response
}

async fn authenticate_websocket_request(
    req: &Request<Body>,
    state: &ServerState,
) -> Result<Account, String> {
    let uri = req.uri();
    let query_params: HashMap<_, _> = uri
        .query()
        .map(|q| url::form_urlencoded::parse(q.as_bytes()).into_owned().collect())
        .unwrap_or_default();

    let token = query_params
        .get("token")
        .ok_or_else(|| "Missing token".to_string())?;

    let claims = jwt::decode_jwt(token).map_err(|e| format!("Invalid token: {}", e))?;

    let account = database::get_account_by_username(&state.db_pool, &claims.sub)
        .await
        .map_err(|e| format!("Database error: {}", e))?
        .ok_or_else(|| "Account not found".to_string())?;

    // CRITICAL CHECK 1: Session ID Match
    if account.session_id.as_deref() != Some(&claims.sid) {
        return Err("Session ID mismatch or session invalidated".to_string());
    }

    // CRITICAL CHECK 2: Session Expiration
    if let Some(expires_at) = account.session_expires_at {
        if Utc::now() > expires_at.and_utc() {
            return Err("Session expired".to_string());
        }
    } else {
        // If session_id is present but session_expires_at is null, it's an invalid state
        return Err("Invalid session state: expiration missing".to_string());
    }

    Ok(account)
}

async fn handle_connection(
    ws_stream: UpgradedWebSocket,
    server_state: ServerState,
    account_id: i64,
    username: String,
) {
    let (mut ws_sender, mut ws_receiver) = ws_stream.split();

    if send_message(&mut ws_sender, ServerMessage::PlayerId(account_id))
        .await
        .is_err()
    {
        return;
    }

    let mut final_lobby_id: Option<usize> = None; // To store lobby_id for cleanup outside the loop

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
            account_id,
            username.clone(),
        )
        .await
        {
            final_lobby_id = Some(lobby_id); // Store lobby_id here
            let result = handler::in_game::in_game_loop(
                &mut ws_sender,
                &mut ws_receiver,
                &server_state,
                lobby_id,
                account_id,
            )
            .await;

            match result {
                handler::in_game::InGameLoopResult::PlayerLeft => continue,
                handler::in_game::InGameLoopResult::ClientDisconnected => break,
            }
        } else {
            break;
        }
    }

    // Call cleanup here, after the loop breaks
    if let Some(lobby_id) = final_lobby_id {
        handler::cleanup::cleanup(lobby_id, account_id, &server_state).await;
    } else {
        // If final_lobby_id is None, it means the player disconnected before joining any lobby via pre_game_loop
        // We still need to clear their session in case it was created via initial login but never entered a game phase.
        if let Err(e) = database::clear_session(&server_state.db_pool, account_id).await {
            log::error!("Failed to clear session for player {}: {}", account_id, e);
        }
    }
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
