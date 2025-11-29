use crate::{
    database,
    model::lobby::Lobby,
    router::router,
    state::ServerStateData,
};
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper_util::rt::TokioIo;
use log::{error, info};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::{broadcast, Mutex};

const NUM_LOBBIES: usize = 5;

pub async fn run() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let db_pool = database::init_db().await.unwrap();
    let (lobby_tx, _) = broadcast::channel(16);

    let server_state = Arc::new(ServerStateData {
        lobbies: Mutex::new((0..NUM_LOBBIES).map(|_| Lobby::new()).collect()),
        db_pool,
        lobby_tx,
    });

    let addr = SocketAddr::from(([0, 0, 0, 0], 9001));
    let listener = TcpListener::bind(addr).await?;
    info!("Server listening on {}", addr);

    loop {
        let (stream, _) = listener.accept().await?;
        let io = TokioIo::new(stream);
        let state_clone = server_state.clone();

        tokio::task::spawn(async move {
            if let Err(err) = http1::Builder::new()
                .serve_connection(io, service_fn(move |req| router(req, state_clone.clone())))
                .with_upgrades()
                .await
            {
                error!("server error: {}", err);
            }
        });
    }
}
