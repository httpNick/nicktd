use crate::model::lobby::Lobby;
use hyper::upgrade::Upgraded;
use hyper_util::rt::TokioIo;
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc, Mutex};
use tokio_tungstenite::WebSocketStream;

pub struct ServerStateData {
    pub lobbies: Mutex<Vec<Lobby>>,
    pub db_pool: SqlitePool,
    pub lobby_tx: broadcast::Sender<String>,
    pub active_connections: Mutex<HashMap<i64, mpsc::Sender<()>>>,
}

pub type ServerState = Arc<ServerStateData>;

pub type UpgradedWebSocket = WebSocketStream<TokioIo<Upgraded>>;
