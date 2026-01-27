use crate::model::lobby::Lobby;
use hyper::upgrade::Upgraded;
use hyper_util::rt::TokioIo;
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, broadcast, mpsc};
use tokio_tungstenite::WebSocketStream;

pub struct ServerStateData {
    pub lobbies: Mutex<Vec<Lobby>>,
    pub db_pool: SqlitePool,
    pub lobby_tx: broadcast::Sender<String>,
    pub active_connections: Mutex<HashMap<i64, mpsc::Sender<()>>>,
}
impl ServerStateData {
    pub fn new(db_pool: SqlitePool) -> Arc<Self> {
        let lobbies = (0..5).map(|_| Lobby::new()).collect();
        let (lobby_tx, _) = broadcast::channel(100);
        Arc::new(Self {
            lobbies: Mutex::new(lobbies),
            db_pool,
            lobby_tx,
            active_connections: Mutex::new(HashMap::new()),
        })
    }
}

pub type ServerState = Arc<ServerStateData>;

pub type UpgradedWebSocket = WebSocketStream<TokioIo<Upgraded>>;
