use common::messages::{CombatEvent, LobbyInfo, SerializableGameState};
use leptos::prelude::*;

/// Global reactive state provided at the `App` component level and consumed
/// by all child views via `expect_context::<AppState>()`.
///
/// All fields are `RwSignal<T>` which are `Send + Sync`, so this struct and
/// any closure capturing it are also `Send + Sync` — safe for Leptos view
/// closures and thread-local message callbacks alike.
#[derive(Clone)]
pub struct AppState {
    /// Server-assigned player ID (received as the first message on connect).
    pub player_id: RwSignal<Option<i64>>,
    /// Latest lobby list received from `ServerMessage::LobbyStatus`.
    pub lobby_status: RwSignal<Vec<LobbyInfo>>,
    /// Latest game-state snapshot (`None` while not in a game).
    pub game_state: RwSignal<Option<SerializableGameState>>,
    /// Most recent `CombatEvent` batch (replaced on each new batch).
    pub combat_events: RwSignal<Vec<CombatEvent>>,
    /// Non-fatal server error text (e.g. force-disconnected by another login).
    pub ws_error: RwSignal<Option<String>>,
    /// Set to `true` when the WebSocket drops unexpectedly.
    pub disconnected: RwSignal<bool>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            player_id: RwSignal::new(None),
            lobby_status: RwSignal::new(Vec::new()),
            game_state: RwSignal::new(None),
            combat_events: RwSignal::new(Vec::new()),
            ws_error: RwSignal::new(None),
            disconnected: RwSignal::new(false),
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
