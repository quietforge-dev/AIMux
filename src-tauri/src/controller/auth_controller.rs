use crate::{
    app_state::ServerState, controller::middleware::Identity, error::AppError,
    model::user::UserView, service::auth_service,
};
use axum::{
    extract::State,
    routing::{get, post},
    Extension, Json, Router,
};
use serde::Deserialize;
use std::sync::Arc;

#[derive(Deserialize)]
struct Login {
    username: String,
    password: String,
}
#[derive(Deserialize)]
struct ChangePassword {
    current_password: String,
    new_password: String,
}
pub fn routes() -> Router<Arc<ServerState>> {
    Router::new()
        .route("/api/auth/login", post(login))
        .route("/api/auth/me", get(me))
        .route("/api/auth/logout", post(logout))
        .route("/api/auth/password", post(password))
        .route("/api/auth/token/reset", post(token))
}
async fn login(
    State(s): State<Arc<ServerState>>,
    Json(p): Json<Login>,
) -> Result<Json<serde_json::Value>, AppError> {
    if s.desktop {
        return Err(AppError::Forbidden);
    }
    let key = p.username.trim().to_lowercase();
    if key.len() > 256 || p.password.len() > 256 {
        return Err(AppError::Unauthorized);
    }
    {
        let mut attempts = s.login_attempts.lock().await;
        attempts.retain(|_, (_, at)| at.elapsed() < std::time::Duration::from_secs(300));
        if attempts.len() >= 1024 && !attempts.contains_key(&key) {
            return Err(AppError::TooManyRequests);
        }
        let entry = attempts
            .entry(key.clone())
            .or_insert((0, std::time::Instant::now()));
        entry.0 += 1;
        if entry.0 > 10 {
            return Err(AppError::TooManyRequests);
        }
    }
    let _slot = s
        .login_slots
        .try_acquire()
        .map_err(|_| AppError::TooManyRequests)?;
    let (token, user) = auth_service::login(&s.pool, p.username.trim(), &p.password).await?;
    s.login_attempts.lock().await.remove(&key);
    Ok(Json(
        serde_json::json!({"token":token,"user":UserView::from(user),"desktop":false}),
    ))
}
async fn me(
    State(s): State<Arc<ServerState>>,
    Extension(i): Extension<Identity>,
) -> Json<serde_json::Value> {
    Json(serde_json::json!({"user":UserView::from(i.user),"desktop":s.desktop}))
}
async fn logout(
    State(s): State<Arc<ServerState>>,
    Extension(i): Extension<Identity>,
) -> Result<Json<serde_json::Value>, AppError> {
    if let Some(hash) = i.session_hash {
        auth_service::logout(&s.pool, &hash).await?;
    }
    Ok(Json(serde_json::json!({"ok":true})))
}
async fn password(
    State(s): State<Arc<ServerState>>,
    Extension(i): Extension<Identity>,
    Json(p): Json<ChangePassword>,
) -> Result<Json<serde_json::Value>, AppError> {
    if s.desktop {
        return Err(AppError::Forbidden);
    }
    auth_service::change_password(&s.pool, &i.user.id, &p.current_password, &p.new_password)
        .await?;
    Ok(Json(serde_json::json!({"ok":true})))
}
async fn token(
    State(s): State<Arc<ServerState>>,
    Extension(i): Extension<Identity>,
) -> Result<Json<serde_json::Value>, AppError> {
    let token = auth_service::reset_token(&s.pool, &i.user.id).await?;
    Ok(Json(serde_json::json!({"token":token})))
}
