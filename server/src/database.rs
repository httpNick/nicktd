use crate::model::account::{Account, NewAccount};
use bcrypt::{hash, verify, DEFAULT_COST};
use chrono::{DateTime, Utc};
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
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(db_url)
        .await?;

    sqlx::migrate!("./migrations")
        .run(&pool)
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
        r#"
        INSERT INTO accounts (username, password_hash) 
        VALUES (?, ?) 
        RETURNING id, username, password_hash, session_id, session_expires_at
        "#,
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
        r#"
        SELECT id, username, password_hash, session_id, session_expires_at
        FROM accounts
        WHERE username = ?
        "#,
        username
    )
    .fetch_optional(pool)
    .await?;

    Ok(account)
}

pub async fn update_session(
    pool: &SqlitePool,
    account_id: i64,
    session_id: &str,
    expires_at: DateTime<Utc>,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "UPDATE accounts SET session_id = ?, session_expires_at = ? WHERE id = ?",
        session_id,
        expires_at,
        account_id
    )
    .execute(pool)
    .await?;
    Ok(())
}


pub async fn clear_session(pool: &SqlitePool, account_id: i64) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "UPDATE accounts SET session_id = NULL, session_expires_at = NULL WHERE id = ?",
        account_id
    )
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn verify_password(password: &str, hashed_password: &str) -> bool {
    verify(password, hashed_password).unwrap_or(false) // TODO: Handle error properly
}