/// sessionStorage key for the JWT session token.
/// Uses sessionStorage (per-tab) so each browser tab maintains an independent
/// session — duplicating a tab copies the session, opening a new tab starts fresh.
pub const TOKEN_KEY: &str = "jwt";

pub fn get_token() -> Option<String> {
    #[cfg(target_arch = "wasm32")]
    {
        use gloo_storage::{SessionStorage, Storage};
        return SessionStorage::get(TOKEN_KEY).ok();
    }
    #[allow(unreachable_code)]
    None
}

pub fn set_token(token: &str) {
    #[cfg(target_arch = "wasm32")]
    {
        use gloo_storage::{SessionStorage, Storage};
        let _ = SessionStorage::set(TOKEN_KEY, token);
    }
    let _ = token;
}

pub fn clear_token() {
    #[cfg(target_arch = "wasm32")]
    {
        use gloo_storage::{SessionStorage, Storage};
        SessionStorage::delete(TOKEN_KEY);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // RED → GREEN: token key is stable and namespaced
    #[test]
    fn token_key_matches_ts_client() {
        assert_eq!(TOKEN_KEY, "jwt");
    }

    // RED → GREEN: no browser storage available on native test runner
    #[test]
    fn get_token_returns_none_on_native() {
        assert_eq!(get_token(), None);
    }
}
