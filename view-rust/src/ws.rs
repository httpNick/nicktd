use common::messages::ClientMessage;
#[cfg(target_arch = "wasm32")]
use leptos::prelude::*;

pub const SERVER_HOST: &str = "127.0.0.1:9001";
pub const API_BASE: &str = "http://127.0.0.1:9001";

/// Build a WebSocket URL with the JWT as a query parameter.
/// The server authenticates via `?token=` at upgrade time.
pub fn build_ws_url(server_host: &str, token: &str) -> String {
    format!("ws://{}/ws?token={}", server_host, token)
}

/// Build an absolute HTTP API URL.
pub fn build_api_url(path: &str) -> String {
    format!("{}{}", API_BASE, path)
}

// ── Thread-local sender registry ──────────────────────────────────────────────
//
// Storing the send function in a thread_local avoids Send+Sync requirements on
// closures that use it — they call `send_ws_message()` (a plain fn) rather than
// directly capturing a !Send value like Rc or futures::mpsc::Sender.
// In WASM, all JS callbacks run on the same thread, so this is safe.

thread_local! {
    static WS_SENDER: std::cell::RefCell<Option<Box<dyn Fn(ClientMessage)>>> =
        std::cell::RefCell::new(None);
}

/// Send a `ClientMessage` over the active WebSocket connection.
/// No-op if no connection is currently open.
pub fn send_ws_message(msg: ClientMessage) {
    WS_SENDER.with(|cell| {
        if let Some(f) = cell.borrow().as_ref() {
            f(msg);
        }
    });
}

/// Register the active send function (called by the WS client setup).
#[cfg(target_arch = "wasm32")]
pub(crate) fn register_ws_sender(f: Box<dyn Fn(ClientMessage)>) {
    WS_SENDER.with(|cell| *cell.borrow_mut() = Some(f));
}

/// Clear the sender when the connection drops.
#[cfg(target_arch = "wasm32")]
pub(crate) fn deregister_ws_sender() {
    WS_SENDER.with(|cell| *cell.borrow_mut() = None);
}

// ── Full connection helper (task 4.1) ─────────────────────────────────────────

/// Open an authenticated WebSocket and wire it to `AppState`.
///
/// Dispatches all `ServerMessage` variants to the correct signal.
/// Sets `state.disconnected = true` on unexpected close so the UI can
/// offer a reconnect prompt.
#[cfg(target_arch = "wasm32")]
pub fn connect_ws(token: &str, state: crate::app_state::AppState) {
    let url = build_ws_url(SERVER_HOST, token);
    let state_msg = state.clone();
    let state_close = state.clone();

    match client::WsClient::open(
        &url,
        move |msg| dispatch_to_state(&state_msg, msg),
        move || {
            state_close.disconnected.set(true);
        },
    ) {
        Ok(_) => {
            state.disconnected.set(false);
            state.ws_error.set(None);
        }
        Err(e) => {
            leptos::logging::error!("WS connect failed: {e}");
        }
    }
}

/// Route a `ServerMessage` to the right signal in `AppState`.
#[cfg(target_arch = "wasm32")]
fn dispatch_to_state(state: &crate::app_state::AppState, msg: common::messages::ServerMessage) {
    use common::messages::ServerMessage;
    match msg {
        ServerMessage::PlayerId(id) => state.player_id.set(Some(id)),
        ServerMessage::LobbyStatus(lobbies) => state.lobby_status.set(lobbies),
        ServerMessage::GameState(gs) => {
            // Forward to Bevy bridge buffer as well.
            crate::bevy_app::push_game_state(gs.clone());
            state.game_state.set(Some(gs));
        }
        ServerMessage::CombatEvents(events) => {
            crate::bevy_app::push_combat_events(events.clone());
            state.combat_events.set(events);
        }
        ServerMessage::Error(e) => state.ws_error.set(Some(e)),
        ServerMessage::UnitInfo(_) => {} // handled by game HUD in task 6
    }
}

// ── WASM WebSocket client ──────────────────────────────────────────────────────

#[cfg(target_arch = "wasm32")]
pub mod client {
    use common::messages::{ClientMessage, ServerMessage};
    use futures::channel::mpsc;
    use futures::{SinkExt, StreamExt};
    use gloo_net::websocket::futures::WebSocket;
    use gloo_net::websocket::Message;

    pub struct WsClient;

    impl WsClient {
        /// Open a connection to `url`, register the global sender, and start
        /// background read/write tasks.
        pub fn open(
            url: &str,
            on_message: impl Fn(ServerMessage) + 'static,
            on_close: impl Fn() + 'static,
        ) -> Result<Self, String> {
            let ws = WebSocket::open(url).map_err(|e| format!("{e:?}"))?;
            let (mut write, mut read) = ws.split();
            let (tx, mut rx) = mpsc::channel::<ClientMessage>(32);

            // Read task: deserialise incoming frames and dispatch to callback.
            leptos::task::spawn_local(async move {
                while let Some(Ok(msg)) = read.next().await {
                    if let Message::Text(text) = msg {
                        if let Ok(server_msg) = serde_json::from_str::<ServerMessage>(&text) {
                            on_message(server_msg);
                        }
                    }
                }
                super::deregister_ws_sender();
                on_close();
            });

            // Write task: forward ClientMessages from the channel to the socket.
            leptos::task::spawn_local(async move {
                while let Some(client_msg) = rx.next().await {
                    if let Ok(json) = serde_json::to_string(&client_msg) {
                        if write.send(Message::Text(json)).await.is_err() {
                            break;
                        }
                    }
                }
            });

            // Register the send function globally via thread_local.
            super::register_ws_sender(Box::new(move |msg: ClientMessage| {
                let mut s = tx.clone();
                leptos::task::spawn_local(async move {
                    let _ = s.send(msg).await;
                });
            }));

            Ok(WsClient)
        }
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // RED → GREEN: WS URL includes path and token query param
    #[test]
    fn build_ws_url_formats_correctly() {
        let url = build_ws_url("127.0.0.1:9001", "abc.def.ghi");
        assert_eq!(url, "ws://127.0.0.1:9001/ws?token=abc.def.ghi");
    }

    // RED → GREEN: WS URL always uses ws:// scheme
    #[test]
    fn build_ws_url_uses_ws_scheme() {
        let url = build_ws_url("localhost:9001", "tok");
        assert!(url.starts_with("ws://"));
    }

    // RED → GREEN: API URL is correctly formed
    #[test]
    fn build_api_url_prepends_base() {
        let url = build_api_url("/api/auth/login");
        assert_eq!(url, "http://127.0.0.1:9001/api/auth/login");
    }

    // RED → GREEN: SERVER_HOST constant points to the known server address
    #[test]
    fn server_host_constant_is_correct() {
        assert_eq!(SERVER_HOST, "127.0.0.1:9001");
    }

    // RED → GREEN: send_ws_message is a no-op when no connection is registered
    #[test]
    fn send_ws_message_noop_when_no_sender() {
        use common::messages::ClientMessage;
        // Must not panic when no sender is registered.
        send_ws_message(ClientMessage::LeaveLobby);
    }
}
