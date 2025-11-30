use crate::{
    routes::{auth, ws},
    state::ServerState,
};
use http_body_util::Full;
use hyper::{
    body::{Bytes, Incoming as Body},
    header, Method, Request, Response, StatusCode,
};

pub async fn router(
    mut req: Request<Body>,
    state: ServerState,
) -> Result<Response<Full<Bytes>>, hyper::Error> {
    let mut response = Response::new(Full::new(Bytes::new()));

    if req.method() == Method::OPTIONS {
        *response.status_mut() = StatusCode::OK;
    } else {
        match (req.method(), req.uri().path()) {
            (&Method::POST, "/api/auth/register") => {
                response = auth::handle_register(req, state).await;
            }
            (&Method::POST, "/api/auth/login") => {
                response = auth::handle_login(req, state).await;
            }
            (&Method::POST, "/api/auth/logout") => {
                response = auth::handle_logout(req, state).await;
            }
            (&Method::GET, "/ws") => {
                response = ws::handle_ws_upgrade(&mut req, state).await;
            }
            _ => {
                *response.status_mut() = StatusCode::NOT_FOUND;
            }
        }
    }

    // Add CORS headers to all responses
    let headers = response.headers_mut();
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_ORIGIN,
        "*".parse().unwrap(),
    );
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_METHODS,
        "GET, POST, OPTIONS".parse().unwrap(),
    );
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_HEADERS,
        "Content-Type, Authorization".parse().unwrap(),
    );

    Ok(response)
}
