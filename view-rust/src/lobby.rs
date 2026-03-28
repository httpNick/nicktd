use crate::app_state::AppState;
use common::messages::{ClientMessage, LobbyInfo};
use leptos::prelude::*;

// ── Lobby View ────────────────────────────────────────────────────────────────

/// Pre-game lobby screen.
///
/// Reads lobby list and connection status from the global `AppState` (populated
/// by `ws::connect_ws`).  Shows a reconnection overlay if the WebSocket drops.
#[component]
pub fn LobbyView() -> impl IntoView {
    let state = expect_context::<AppState>();
    let token_ctx = expect_context::<RwSignal<Option<String>>>();

    let on_join = move |lobby_id: usize| {
        crate::ws::send_ws_message(ClientMessage::JoinLobby(lobby_id));
    };

    let navigate_login = leptos_router::hooks::use_navigate();
    let on_logout = {
        let state = state.clone();
        move |_| {
            crate::storage::clear_token();
            token_ctx.set(None);
            state.game_state.set(None);
            state.lobby_status.set(Vec::new());
            navigate_login("/login", Default::default());
        }
    };

    // Navigate to /game when the server transitions us into a game.
    let navigate_game = leptos_router::hooks::use_navigate();
    let state_nav = state.clone();
    Effect::new(move |_| {
        if state_nav.game_state.get().is_some() {
            navigate_game("/game", Default::default());
        }
    });

    let state_overlay = state.clone();
    let state_error = state.clone();
    let state_list = state;

    view! {
        <div class="lobby-view">
            <header>
                <h1>"Nick's Tower Defense"</h1>
                <button on:click=on_logout class="logout-btn">"Logout"</button>
            </header>

            // ── Reconnection overlay (req 4.5) ─────────────────────────────
            {move || {
                if state_overlay.disconnected.get() {
                    #[cfg(target_arch = "wasm32")]
                    let token = token_ctx.get_untracked().unwrap_or_default();
                    #[cfg(target_arch = "wasm32")]
                    let state_rc = state_overlay.clone();
                    view! {
                        <div class="disconnect-overlay">
                            <p>"Connection lost."</p>
                            <button on:click=move |_| {
                                #[cfg(target_arch = "wasm32")]
                                crate::ws::connect_ws(&token, state_rc.clone());
                            }>"Reconnect"</button>
                        </div>
                    }.into_any()
                } else {
                    view! { <></> }.into_any()
                }
            }}

            // ── WS error banner ────────────────────────────────────────────
            {move || state_error.ws_error.get().map(|e| view! {
                <p class="error">"Server: " {e}</p>
            })}

            // ── Lobby list ─────────────────────────────────────────────────
            {move || {
                let lobbies = state_list.lobby_status.get();
                let join = on_join.clone();
                if lobbies.is_empty() {
                    view! {
                        <div class="status"><p>"Connecting to server…"</p></div>
                    }.into_any()
                } else {
                    view! { <LobbyList lobbies=lobbies on_join=join /> }.into_any()
                }
            }}
        </div>
    }
}

// ── Lobby List ────────────────────────────────────────────────────────────────

#[component]
fn LobbyList(
    lobbies: Vec<LobbyInfo>,
    on_join: impl Fn(usize) + Clone + Send + Sync + 'static,
) -> impl IntoView {
    let rows: Vec<_> = lobbies
        .into_iter()
        .map(|info| {
            let id = info.id;
            let join = on_join.clone();
            let available = info.player_count < 2;
            view! {
                <li class="lobby-item">
                    <span>"Lobby " {id + 1}</span>
                    <span>{info.player_count} " / 2 players"</span>
                    <button disabled=!available on:click=move |_| join(id)>
                        {if available { "Join" } else { "Full" }}
                    </button>
                </li>
            }
        })
        .collect();

    view! {
        <div class="lobby-list">
            <h2>"Available Lobbies"</h2>
            <ul>{rows}</ul>
        </div>
    }
}

// ── Waiting screen shown after joining a lobby ────────────────────────────────

#[component]
pub fn WaitingView(lobby_id: usize) -> impl IntoView {
    view! {
        <div class="waiting-view">
            <h2>"Waiting for Opponent"</h2>
            <p>"Lobby " {lobby_id + 1}</p>
            <p class="hint">"Share this lobby number with a friend to start."</p>
            <div class="spinner" aria-label="Loading" />
        </div>
    }
}
