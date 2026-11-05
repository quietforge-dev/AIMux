use crate::{
    app_state::{AppState, ServerState},
    error::AppError,
    service::statistics_service,
};
use axum::{extract::Extension, routing::get, Json, Router};
use std::sync::Arc;
pub fn routes() -> Router<Arc<ServerState>> {
    Router::new().route("/api/usage/statistics", get(statistics))
}
async fn statistics(
    Extension(s): Extension<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, AppError> {
    Ok(Json(statistics_service::tokens(&s.pool).await?))
}
