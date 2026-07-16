use crate::model::lobby::Lobby;
use hyper::upgrade::Upgraded;
use hyper_util::rt::TokioIo;
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use tokio::sync::{Mutex, RwLock, mpsc, oneshot};
use tokio_tungstenite::WebSocketStream;

/// A player waiting in the matchmaking queue.
pub struct QueuedPlayer {
    pub account_id: i64,
    pub username: String,
    /// Fires with the match_id when this player gets paired.
    pub match_tx: oneshot::Sender<u64>,
}

pub struct ServerStateData {
    /// match_id -> lobby. LOCK RULE: held only to insert/remove/get+clone the
    /// Arc — never while holding a lobby guard, never across a network await.
    pub matches: RwLock<HashMap<u64, Arc<Mutex<Lobby>>>>,
    pub next_match_id: AtomicU64,
    /// At most one waiter (1v1: the second joiner always pairs immediately).
    /// LOCK RULE: never overlaps a lobby guard or a network await.
    pub queue: Mutex<Option<QueuedPlayer>>,
    pub db_pool: SqlitePool,
    pub active_connections: Mutex<HashMap<i64, mpsc::Sender<()>>>,
}
impl ServerStateData {
    pub fn new(db_pool: SqlitePool) -> Arc<Self> {
        Arc::new(Self {
            matches: RwLock::new(HashMap::new()),
            next_match_id: AtomicU64::new(0),
            queue: Mutex::new(None),
            db_pool,
            active_connections: Mutex::new(HashMap::new()),
        })
    }
}

pub type ServerState = Arc<ServerStateData>;

pub type UpgradedWebSocket = WebSocketStream<TokioIo<Upgraded>>;

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;
    use std::sync::atomic::Ordering;

    #[tokio::test]
    async fn new_state_has_empty_matches_and_queue() {
        let db_pool = SqlitePoolOptions::new()
            .connect("sqlite::memory:")
            .await
            .unwrap();
        let state = ServerStateData::new(db_pool);
        assert!(state.matches.read().await.is_empty());
        assert!(state.queue.lock().await.is_none());
        assert_eq!(state.next_match_id.load(Ordering::Relaxed), 0);
    }
}
