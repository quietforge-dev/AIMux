use axum::{extract::State, routing::post, Json, Router};
use serde::{Deserialize, Serialize};
use std::{env, sync::Arc};
use uuid::Uuid;

use crate::{app_state::AppState, error::AppError};

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    username: String,
    password: String,
}

#[derive(Debug, Serialize)]
pub struct LoginResponse {
    token: String,
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new().route("/api/auth/login", post(login))
}

async fn login(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, AppError> {
    let username = env::var("AIMUX_ADMIN_USERNAME")
        .map_err(|_| AppError::BadRequest("服务器未配置网页登录账号".into()))?;
    let password = env::var("AIMUX_ADMIN_PASSWORD")
        .map_err(|_| AppError::BadRequest("服务器未配置网页登录密码".into()))?;
    if payload.username != username || payload.password != password {
        return Err(AppError::BadRequest("账号或密码错误".into()));
    }
    let token = Uuid::new_v4().to_string();
    state.web_sessions.write().await.insert(token.clone());
    Ok(Json(LoginResponse { token }))
}
