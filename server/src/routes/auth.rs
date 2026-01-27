use crate::{
    database,
    model::{account::NewAccount, jwt},
    state::ServerState,
};
use chrono::{Duration, Utc};
use http_body_util::{BodyExt, Full};
use hyper::{
    Response, StatusCode,
    body::{Bytes, Incoming as Body},
    header,
};
use log::error;

pub async fn handle_register(
    req: hyper::Request<Body>,
    state: ServerState,
) -> Response<Full<Bytes>> {
    let body_bytes = match req.collect().await {
        Ok(body) => body.to_bytes(),
        Err(_) => {
            return Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Full::new(Bytes::from(
                    serde_json::to_string(&serde_json::json!({"error": "Invalid request payload"}))
                        .unwrap(),
                )))
                .unwrap();
        }
    };

    match serde_json::from_slice::<NewAccount>(&body_bytes) {
        Ok(payload) => match database::create_account(&state.db_pool, payload).await {
            Ok(account) => Response::builder()
                .status(StatusCode::CREATED)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Full::new(Bytes::from(
                    serde_json::to_string(
                        &serde_json::json!({"id": account.id, "username": account.username}),
                    )
                    .unwrap(),
                )))
                .unwrap(),
            Err(e) => {
                error!("Failed to create account: {}", e);
                Response::builder()
                    .status(StatusCode::BAD_REQUEST)
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Full::new(Bytes::from(
                        serde_json::to_string(
                            &serde_json::json!({"error": "Failed to create account or user already exists"}),
                        )
                        .unwrap(),
                    )))
                    .unwrap()
            }
        },
        Err(_) => Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Full::new(Bytes::from(
                serde_json::to_string(&serde_json::json!({"error": "Invalid request payload"}))
                    .unwrap(),
            )))
            .unwrap(),
    }
}

use uuid::Uuid;

pub async fn handle_login(req: hyper::Request<Body>, state: ServerState) -> Response<Full<Bytes>> {
    let body_bytes = match req.collect().await {
        Ok(body) => body.to_bytes(),
        Err(_) => {
            return Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .body(Full::new(Bytes::from("Invalid request payload")))
                .unwrap();
        }
    };

    match serde_json::from_slice::<NewAccount>(&body_bytes) {
        Ok(payload) => {
            match database::get_account_by_username(&state.db_pool, &payload.username).await {
                Ok(Some(account)) => {
                    if database::verify_password(&payload.password, &account.password_hash).await {
                        let session_id = Uuid::new_v4().to_string();
                        let expires_at = Utc::now() + Duration::hours(24);

                        match database::update_session(
                            &state.db_pool,
                            account.id,
                            &session_id,
                            expires_at,
                        )
                        .await
                        {
                            Ok(_) => match jwt::create_jwt(account.username, session_id, expires_at)
                            {
                                Ok(token) => Response::builder()
                                    .status(StatusCode::OK)
                                    .header(header::CONTENT_TYPE, "application/json")
                                    .body(Full::new(Bytes::from(
                                        serde_json::to_string(
                                            &serde_json::json!({ "token": token }),
                                        )
                                        .unwrap(),
                                    )))
                                    .unwrap(),
                                Err(_) => Response::builder()
                                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                                    .header(header::CONTENT_TYPE, "application/json")
                                    .body(Full::new(Bytes::from(
                                        serde_json::to_string(
                                            &serde_json::json!({"error": "Failed to create token"}),
                                        )
                                        .unwrap(),
                                    )))
                                    .unwrap(),
                            },
                            Err(_) => Response::builder()
                                .status(StatusCode::INTERNAL_SERVER_ERROR)
                                .header(header::CONTENT_TYPE, "application/json")
                                .body(Full::new(Bytes::from(
                                    serde_json::to_string(
                                        &serde_json::json!({"error": "Failed to update session"}),
                                    )
                                    .unwrap(),
                                )))
                                .unwrap(),
                        }
                    } else {
                        Response::builder()
                            .status(StatusCode::UNAUTHORIZED)
                            .header(header::CONTENT_TYPE, "application/json")
                            .body(Full::new(Bytes::from(
                                serde_json::to_string(
                                    &serde_json::json!({"error": "Invalid credentials"}),
                                )
                                .unwrap(),
                            )))
                            .unwrap()
                    }
                }
                _ => Response::builder()
                    .status(StatusCode::UNAUTHORIZED)
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Full::new(Bytes::from(
                        serde_json::to_string(&serde_json::json!({"error": "Invalid credentials"}))
                            .unwrap(),
                    )))
                    .unwrap(),
            }
        }
        Err(_) => Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body(Full::new(Bytes::from("Invalid request payload")))
            .unwrap(),
    }
}

pub async fn handle_logout(req: hyper::Request<Body>, state: ServerState) -> Response<Full<Bytes>> {
    let auth_header = req.headers().get(header::AUTHORIZATION);
    if let Some(auth_header) = auth_header {
        if let Ok(auth_str) = auth_header.to_str() {
            if auth_str.starts_with("Bearer ") {
                let token = &auth_str[7..];
                if let Ok(claims) = jwt::decode_jwt(token) {
                    if let Ok(Some(account)) =
                        database::get_account_by_username(&state.db_pool, &claims.sub).await
                    {
                        // Check if the session ID in the token matches the one in the database
                        if account.session_id.as_deref() == Some(&claims.sid) {
                            if let Err(e) =
                                database::clear_session(&state.db_pool, account.id).await
                            {
                                error!("Failed to clear session: {}", e);
                                return Response::builder()
                                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                                    .header(header::CONTENT_TYPE, "application/json")
                                    .body(Full::new(Bytes::from(
                                        serde_json::to_string(
                                            &serde_json::json!({"error": "Failed to logout"}),
                                        )
                                        .unwrap(),
                                    )))
                                    .unwrap();
                            }
                        }
                    }
                }
            }
        }
    }
    // Always return OK, even if the token is invalid or the session doesn't exist.
    // This prevents leaking information about session validity.
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Full::new(Bytes::from(
            serde_json::to_string(&serde_json::json!({"message": "Logged out successfully"}))
                .unwrap(),
        )))
        .unwrap()
}
