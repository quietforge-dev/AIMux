use crate::{
    app_state::{AppState, ServerState},
    error::AppError,
    service::usage_service,
};
use axum::{
    extract::{Extension, Path, Query},
    routing::{delete, get},
    Json, Router,
};
use serde::Deserialize;
use std::sync::Arc;
#[derive(Deserialize, Default)]
struct Q {
    pub offset: Option<i64>,
    pub limit: Option<i64>,
    pub account_id: Option<String>,
    pub model: Option<String>,
    pub reasoning_effort: Option<String>,
    #[serde(rename = "type")]
    pub kind: Option<String>,
    pub success: Option<bool>,
    pub started_after: Option<String>,
    pub started_before: Option<String>,
}
#[derive(Deserialize, Default)]
struct CleanupQuery {
    days: Option<i64>,
}
pub fn routes() -> Router<Arc<ServerState>> {
    Router::new()
        .route("/api/usage/records", get(list))
        .route("/api/usage/records/{id}", get(detail))
        .route("/api/usage/records/expired", delete(cleanup))
}
async fn list(
    Extension(s): Extension<Arc<AppState>>,
    Query(q): Query<Q>,
) -> Result<Json<serde_json::Value>, AppError> {
    Ok(Json(
        serde_json::to_value(
            usage_service::list(
                &s.pool,
                q.offset.unwrap_or(0),
                q.limit.unwrap_or(20),
                q.account_id.as_deref(),
                q.model.as_deref(),
                q.kind.as_deref(),
                q.success,
                q.started_after.as_deref(),
                q.started_before.as_deref(),
                q.reasoning_effort.as_deref(),
            )
            .await?,
        )
        .unwrap(),
    ))
}
async fn detail(
    Extension(s): Extension<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    let row = usage_service::detail(&s.pool, &id)
        .await?
        .ok_or_else(|| AppError::NotFound("使用记录不存在".into()))?;
    Ok(Json(serde_json::to_value(row).unwrap()))
}
async fn cleanup(
    Extension(s): Extension<Arc<AppState>>,
    Query(q): Query<CleanupQuery>,
) -> Result<Json<serde_json::Value>, AppError> {
    Ok(Json(
        serde_json::json!({"deleted":usage_service::cleanup(&s.pool, q.days.unwrap_or(7)).await?}),
    ))
}
