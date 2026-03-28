use leptos::prelude::*;
use serde::{Deserialize, Serialize};

pub const LOGIN_PATH: &str = "/api/auth/login";
pub const REGISTER_PATH: &str = "/api/auth/register";

/// Credentials posted to `/api/auth/login` and `/api/auth/register`.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Credentials {
    pub username: String,
    pub password: String,
}

/// Successful login response — server returns only a `token` field.
#[derive(Deserialize, Debug)]
pub struct LoginResponse {
    pub token: String,
}

/// POST credentials and return the JWT on success.
/// Only compiled for WASM (uses reqwest's browser Fetch backend).
#[cfg(target_arch = "wasm32")]
async fn post_for_token(api_path: &str, creds: &Credentials) -> Result<String, String> {
    let url = crate::ws::build_api_url(api_path);
    let resp = reqwest::Client::new()
        .post(&url)
        .json(creds)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if resp.status().is_success() {
        let body: LoginResponse = resp.json().await.map_err(|e| e.to_string())?;
        Ok(body.token)
    } else {
        let body: serde_json::Value = resp.json().await.unwrap_or_default();
        Err(body["error"]
            .as_str()
            .unwrap_or("Unknown error")
            .to_string())
    }
}

/// POST to register. Server returns `{id, username}` on success (no token).
#[cfg(target_arch = "wasm32")]
async fn post_register(creds: &Credentials) -> Result<(), String> {
    let url = crate::ws::build_api_url(REGISTER_PATH);
    let resp = reqwest::Client::new()
        .post(&url)
        .json(creds)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if resp.status().is_success() {
        Ok(())
    } else {
        let body: serde_json::Value = resp.json().await.unwrap_or_default();
        Err(body["error"]
            .as_str()
            .unwrap_or("Registration failed")
            .to_string())
    }
}

// ── Login View ────────────────────────────────────────────────────────────────

#[component]
pub fn LoginView() -> impl IntoView {
    let username = RwSignal::new(String::new());
    let password = RwSignal::new(String::new());
    let error = RwSignal::new(Option::<String>::None);
    let loading = RwSignal::new(false);

    // navigate and token_ctx are only needed on WASM; define them inside the
    // cfg block so they don't generate "unused variable" warnings on native.
    let on_submit = {
        #[cfg(target_arch = "wasm32")]
        let navigate = leptos_router::hooks::use_navigate();
        #[cfg(target_arch = "wasm32")]
        let token_ctx = expect_context::<RwSignal<Option<String>>>();
        #[cfg(target_arch = "wasm32")]
        let app_state = expect_context::<crate::app_state::AppState>();

        move |ev: leptos::ev::SubmitEvent| {
            ev.prevent_default();
            let user = username.get();
            let pass = password.get();

            if user.is_empty() || pass.is_empty() {
                error.set(Some("Username and password are required.".to_string()));
                return;
            }

            loading.set(true);
            error.set(None);

            #[cfg(target_arch = "wasm32")]
            {
                let nav = navigate.clone();
                let state = app_state.clone();
                leptos::task::spawn_local(async move {
                    let creds = Credentials { username: user, password: pass };
                    match post_for_token(LOGIN_PATH, &creds).await {
                        Ok(token) => {
                            crate::storage::set_token(&token);
                            crate::ws::connect_ws(&token, state);
                            token_ctx.set(Some(token));
                            nav("/lobby", Default::default());
                        }
                        Err(e) => {
                            error.set(Some(e));
                            loading.set(false);
                        }
                    }
                });
            }
        }
    };

    view! {
        <div class="auth-view">
            <h2>"Login"</h2>
            <form on:submit=on_submit>
                <div>
                    <label for="login-username">"Username"</label>
                    <input
                        id="login-username"
                        type="text"
                        prop:value=username
                        on:input=move |ev| username.set(event_target_value(&ev))
                        autocomplete="username"
                        disabled=loading
                    />
                </div>
                <div>
                    <label for="login-password">"Password"</label>
                    <input
                        id="login-password"
                        type="password"
                        prop:value=password
                        on:input=move |ev| password.set(event_target_value(&ev))
                        autocomplete="current-password"
                        disabled=loading
                    />
                </div>
                {move || error.get().map(|e| view! { <p class="error">{e}</p> })}
                <button type="submit" disabled=loading>"Login"</button>
            </form>
            <p>"Don't have an account? " <a href="/register">"Register"</a></p>
        </div>
    }
}

// ── Register View ─────────────────────────────────────────────────────────────

#[component]
pub fn RegisterView() -> impl IntoView {
    let username = RwSignal::new(String::new());
    let password = RwSignal::new(String::new());
    let error = RwSignal::new(Option::<String>::None);
    let loading = RwSignal::new(false);

    let on_submit = {
        #[cfg(target_arch = "wasm32")]
        let navigate = leptos_router::hooks::use_navigate();
        #[cfg(target_arch = "wasm32")]
        let token_ctx = expect_context::<RwSignal<Option<String>>>();
        #[cfg(target_arch = "wasm32")]
        let app_state = expect_context::<crate::app_state::AppState>();

        move |ev: leptos::ev::SubmitEvent| {
            ev.prevent_default();
            let user = username.get();
            let pass = password.get();

            if user.is_empty() || pass.is_empty() {
                error.set(Some("Username and password are required.".to_string()));
                return;
            }

            loading.set(true);
            error.set(None);

            #[cfg(target_arch = "wasm32")]
            {
                let nav = navigate.clone();
                let state = app_state.clone();
                leptos::task::spawn_local(async move {
                    let creds = Credentials { username: user, password: pass };
                    match post_register(&creds).await {
                        Ok(_) => {
                            // Auto-login with the same credentials (req 3.6).
                            match post_for_token(LOGIN_PATH, &creds).await {
                                Ok(token) => {
                                    crate::storage::set_token(&token);
                                    crate::ws::connect_ws(&token, state);
                                    token_ctx.set(Some(token));
                                    nav("/lobby", Default::default());
                                }
                                Err(e) => {
                                    error.set(Some(format!(
                                        "Registered, but auto-login failed: {e}"
                                    )));
                                    loading.set(false);
                                }
                            }
                        }
                        Err(e) => {
                            error.set(Some(e));
                            loading.set(false);
                        }
                    }
                });
            }
        }
    };

    view! {
        <div class="auth-view">
            <h2>"Register"</h2>
            <form on:submit=on_submit>
                <div>
                    <label for="reg-username">"Username"</label>
                    <input
                        id="reg-username"
                        type="text"
                        prop:value=username
                        on:input=move |ev| username.set(event_target_value(&ev))
                        autocomplete="username"
                        disabled=loading
                    />
                </div>
                <div>
                    <label for="reg-password">"Password"</label>
                    <input
                        id="reg-password"
                        type="password"
                        prop:value=password
                        on:input=move |ev| password.set(event_target_value(&ev))
                        autocomplete="new-password"
                        disabled=loading
                    />
                </div>
                {move || error.get().map(|e| view! { <p class="error">{e}</p> })}
                <button type="submit" disabled=loading>"Create Account"</button>
            </form>
            <p>"Already have an account? " <a href="/login">"Login"</a></p>
        </div>
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // RED → GREEN: API paths match the server router exactly
    #[test]
    fn login_path_matches_server_route() {
        assert_eq!(LOGIN_PATH, "/api/auth/login");
    }

    #[test]
    fn register_path_matches_server_route() {
        assert_eq!(REGISTER_PATH, "/api/auth/register");
    }

    // RED → GREEN: credentials serialise to the shape the server expects
    #[test]
    fn credentials_serialize_to_expected_json() {
        let creds = Credentials {
            username: "alice".to_string(),
            password: "secret".to_string(),
        };
        let json = serde_json::to_string(&creds).unwrap();
        assert!(json.contains("\"username\":\"alice\""));
        assert!(json.contains("\"password\":\"secret\""));
    }

    // RED → GREEN: login response deserialises the token field
    #[test]
    fn login_response_extracts_token() {
        let json = r#"{"token":"header.payload.sig"}"#;
        let resp: LoginResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.token, "header.payload.sig");
    }

    // RED → GREEN: credentials round-trip through JSON
    #[test]
    fn credentials_round_trip() {
        let original = Credentials {
            username: "bob".to_string(),
            password: "hunter2".to_string(),
        };
        let json = serde_json::to_string(&original).unwrap();
        let restored: Credentials = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.username, original.username);
        assert_eq!(restored.password, original.password);
    }
}
