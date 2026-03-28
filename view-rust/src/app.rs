use crate::{
    app_state::AppState,
    auth::{LoginView, RegisterView},
    game_view::GameView,
    lobby::LobbyView,
    storage,
};
use leptos::prelude::*;
use leptos_router::{
    components::{Route, Router, Routes},
    path,
};

/// Root application component.
///
/// Creates the global `AppState`, provides it as context, opens the WebSocket
/// (if a stored session token exists), and sets up client-side routing.
#[component]
pub fn App() -> impl IntoView {
    let token: RwSignal<Option<String>> = RwSignal::new(storage::get_token());
    let state = AppState::new();

    provide_context(token);
    provide_context(state.clone());

    // If we already have a token (page refresh), reconnect immediately.
    #[cfg(target_arch = "wasm32")]
    {
        if let Some(t) = token.get_untracked() {
            crate::ws::connect_ws(&t, state);
        }
    }

    view! {
        <Router>
            <Routes fallback=|| view! { <p>"Page not found."</p> }>
                <Route path=path!("/") view=move || {
                    let navigate = leptos_router::hooks::use_navigate();
                    if token.get_untracked().is_some() {
                        navigate("/lobby", Default::default());
                    } else {
                        navigate("/login", Default::default());
                    }
                    view! { <></> }
                }/>
                <Route path=path!("/login") view=LoginView />
                <Route path=path!("/register") view=RegisterView />
                <Route path=path!("/lobby") view=move || {
                    let navigate = leptos_router::hooks::use_navigate();
                    if token.get_untracked().is_none() {
                        navigate("/login", Default::default());
                        return view! { <></> }.into_any();
                    }
                    view! { <LobbyView /> }.into_any()
                }/>
                <Route path=path!("/game") view=GameView />
            </Routes>
        </Router>
    }
}
