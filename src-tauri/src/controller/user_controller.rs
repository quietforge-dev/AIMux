use crate::{
    app_state::ServerState, controller::middleware::Identity, error::AppError,
    model::user::UserView, service::auth_service,
};
use axum::{
    extract::{Path, Query, State},
    routing::{get, post},
    Extension, Json, Router,
};
use serde::Deserialize;
use std::sync::Arc;
#[derive(Deserialize, Default)]
struct Page {
    offset: Option<i64>,
    limit: Option<i64>,
}
#[derive(Deserialize)]
struct Create {
    username: String,
    password: String,
}
#[derive(Deserialize)]
struct Enabled {
    enabled: bool,
}
#[derive(Deserialize)]
struct Password {
    password: String,
}
pub fn routes() -> Router<Arc<ServerState>> {
    Router::new()
        .route("/api/users", get(list).post(create))
        .route("/api/users/{id}/enabled", post(enabled))
        .route("/api/users/{id}/password", post(password))
}
fn require_admin(s: &ServerState, i: &Identity) -> Result<(), AppError> {
    if s.desktop {
        return Err(AppError::Forbidden);
    }
    i.admin()
}
async fn list(
    State(s): State<Arc<ServerState>>,
    Extension(i): Extension<Identity>,
    Query(p): Query<Page>,
) -> Result<Json<serde_json::Value>, AppError> {
    require_admin(&s, &i)?;
    let (users, total) =
        auth_service::list_users(&s.pool, p.offset.unwrap_or(0), p.limit.unwrap_or(20)).await?;
    Ok(Json(
        serde_json::json!({"items":users.into_iter().map(UserView::from).collect::<Vec<_>>(),"total":total}),
    ))
}
async fn create(
    State(s): State<Arc<ServerState>>,
    Extension(i): Extension<Identity>,
    Json(p): Json<Create>,
) -> Result<Json<serde_json::Value>, AppError> {
    require_admin(&s, &i)?;
    let (user, token) = auth_service::create_user(&s.pool, &p.username, &p.password).await?;
    s.user_state(&user).await?;
    Ok(Json(
        serde_json::json!({"user":UserView::from(user),"token":token}),
    ))
}
async fn enabled(
    State(s): State<Arc<ServerState>>,
    Extension(i): Extension<Identity>,
    Path(id): Path<String>,
    Json(p): Json<Enabled>,
) -> Result<Json<serde_json::Value>, AppError> {
    require_admin(&s, &i)?;
    auth_service::set_user_enabled(&s.pool, &id, p.enabled).await?;
    if !p.enabled {
        s.stop_user(&id).await;
    }
    Ok(Json(serde_json::json!({"ok":true})))
}
async fn password(
    State(s): State<Arc<ServerState>>,
    Extension(i): Extension<Identity>,
    Path(id): Path<String>,
    Json(p): Json<Password>,
) -> Result<Json<serde_json::Value>, AppError> {
    require_admin(&s, &i)?;
    auth_service::reset_password(&s.pool, &id, &p.password).await?;
    Ok(Json(serde_json::json!({"ok":true})))
}
