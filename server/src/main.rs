mod database;
mod handler;
mod model;
mod router;
mod routes;
mod server;
mod state;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    env_logger::init();
    server::run().await
}