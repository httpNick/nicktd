use crate::{database, router::router, state::ServerStateData};
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper_util::rt::TokioIo;
use log::{error, info};
use std::net::SocketAddr;
use tokio::net::TcpListener;

pub async fn run() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let db_pool = database::init_db().await.unwrap();
    let server_state = ServerStateData::new(db_pool);

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
