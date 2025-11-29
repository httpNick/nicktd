use crate::{
    database,
    model::{account::NewAccount, jwt},
    state::ServerState,
};
use http_body_util::{BodyExt, Full};
use hyper::{
    body::{Bytes, Incoming as Body},
    header, Response, StatusCode,
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

pub async fn handle_login(
    req: hyper::Request<Body>,
    state: ServerState,
) -> Response<Full<Bytes>> {
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
                        match jwt::create_jwt(account.username) {
                            Ok(token) => Response::builder()
                                .status(StatusCode::OK)
                                .header(header::CONTENT_TYPE, "application/json")
                                .body(Full::new(Bytes::from(
                                    serde_json::to_string(&serde_json::json!({ "token": token }))
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
