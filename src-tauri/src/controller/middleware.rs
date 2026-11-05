use crate::app_state::AppState;
use axum::{
    extract::State,
    http::{Request, StatusCode},
    middleware::Next,
    response::Response,
};
use std::{env, sync::Arc};

pub async fn auth(
    State(state): State<Arc<AppState>>,
    request: Request<axum::body::Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    let path = request.uri().path();
    if path == "/health" || path == "/api/auth/login" {
        return Ok(next.run(request).await);
    }
    if !path.starts_with("/api/") && !path.starts_with("/v1/") {
        return Ok(next.run(request).await);
    }
    let token = state.settings.read().await.local_token.clone();
    let supplied = request
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .or_else(|| {
            request
                .headers()
                .get("x-aimux-token")
                .and_then(|v| v.to_str().ok())
        });
    let session_valid = if let Some(value) = supplied {
        state.web_sessions.read().await.contains(value)
    } else {
        false
    };
    let admin_login_enabled =
        env::var("AIMUX_ADMIN_USERNAME").is_ok() || env::var("AIMUX_ADMIN_PASSWORD").is_ok();
    if session_valid
        || supplied == Some(token.as_str())
        || (!admin_login_enabled && token.is_empty())
    {
        Ok(next.run(request).await)
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}
