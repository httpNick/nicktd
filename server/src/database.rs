use sqlx::{sqlite::SqlitePoolOptions, SqlitePool};
use std::fs;
use std::path::Path;
use bcrypt::{hash, verify, DEFAULT_COST};

use crate::model::account::{Account, NewAccount};

pub async fn init_db() -> Result<SqlitePool, sqlx::Error> {
    let data_dir = Path::new("./data");
    if !data_dir.exists() {
        fs::create_dir(data_dir).expect("Failed to create data directory");
    }
    let db_url = "sqlite:data/nicktd.db";
    if !Path::new("data/nicktd.db").exists() {
        fs::File::create("data/nicktd.db").expect("Failed to create database file");
    }
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(db_url)
        .await?;

    // Create players table if it doesn't exist
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS players (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            username TEXT NOT NULL UNIQUE,
            password_hash TEXT NOT NULL
        );"
    )
    .execute(&pool)
    .await?;

    Ok(pool)
}

pub async fn create_account(
    pool: &SqlitePool,
    new_account: NewAccount,
) -> Result<Account, sqlx::Error> {
    let hashed_password = hash(new_account.password, DEFAULT_COST).unwrap(); // Handle error properly in production
    let account = sqlx::query_as!(
        Account,
        r#"INSERT INTO players (username, password_hash) VALUES (?, ?) RETURNING id AS "id!", username AS "username!", password_hash AS "password_hash!""#,
        new_account.username,
        hashed_password
    )
    .fetch_one(pool)
    .await?;

    Ok(account)
}

pub async fn get_account_by_username(
    pool: &SqlitePool,
    username: &str,
) -> Result<Option<Account>, sqlx::Error> {
    let account = sqlx::query_as!(
        Account,
        r#"SELECT id AS "id!", username AS "username!", password_hash AS "password_hash!" FROM players WHERE username = ?"#,
        username
    )
    .fetch_optional(pool)
    .await?;

    Ok(account)
}

pub async fn verify_password(password: &str, hashed_password: &str) -> bool {
    verify(password, hashed_password).unwrap_or(false) // Handle error properly in production
}