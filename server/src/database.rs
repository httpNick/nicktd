use sqlx::{sqlite::SqlitePoolOptions, SqlitePool};
use std::fs;
use std::path::Path;

pub async fn init_db() -> Result<SqlitePool, sqlx::Error> {
    let data_dir = Path::new("./data");
    if !data_dir.exists() {
        fs::create_dir(data_dir).expect("Failed to create data directory");
    }
    let db_url = "sqlite:data/nicktd.db";
    if !Path::new("data/nicktd.db").exists() {
        fs::File::create("data/nicktd.db").expect("Failed to create database file");
    }
    SqlitePoolOptions::new()
        .max_connections(5)
        .connect(db_url)
        .await
}