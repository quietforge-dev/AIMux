use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    routing::{get, post},
    Json, Router,
};

use crate::{
    app_state::AppState,
    dao::multiplier_monitor_dao,
    error::AppError,
    schema::multiplier_monitor_schema::{
        MultiplierMonitorCreate, MultiplierMonitorLogQuery, MultiplierMonitorUpdate,
    },
    service::multiplier_monitor_service,
};

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/multiplier-monitors", get(list).post(create))
        .route(
            "/api/multiplier-monitors/account-options",
            get(account_options),
        )
        .route("/api/multiplier-monitors/{id}", get(get_one).put(update))
        .route("/api/multiplier-monitors/{id}/toggle", post(toggle))
        .route("/api/multiplier-monitors/{id}/check", post(check))
        .route("/api/multiplier-monitors/{id}/logs", get(logs))
}

async fn list(State(state): State<Arc<AppState>>) -> Result<Json<serde_json::Value>, AppError> {
    Ok(Json(serde_json::json!({
        "items": multiplier_monitor_service::list(&state.pool).await?
    })))
}

async fn get_one(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    Ok(Json(
        serde_json::to_value(multiplier_monitor_service::get_view(&state.pool, &id).await?)
            .map_err(|error| AppError::Internal(error.to_string()))?,
    ))
}

async fn create(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<MultiplierMonitorCreate>,
) -> Result<Json<serde_json::Value>, AppError> {
    Ok(Json(
        serde_json::to_value(multiplier_monitor_service::create(&state.pool, payload).await?)
            .map_err(|error| AppError::Internal(error.to_string()))?,
    ))
}

async fn update(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(payload): Json<MultiplierMonitorUpdate>,
) -> Result<Json<serde_json::Value>, AppError> {
    Ok(Json(
        serde_json::to_value(multiplier_monitor_service::update(&state.pool, &id, payload).await?)
            .map_err(|error| AppError::Internal(error.to_string()))?,
    ))
}

async fn toggle(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    Ok(Json(
        serde_json::to_value(multiplier_monitor_service::toggle(&state.pool, &id).await?)
            .map_err(|error| AppError::Internal(error.to_string()))?,
    ))
}

async fn check(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    let settings = state.settings.read().await.clone();
    Ok(Json(
        serde_json::to_value(
            multiplier_monitor_service::check_now(&state.pool, &settings, &id).await?,
        )
        .map_err(|error| AppError::Internal(error.to_string()))?,
    ))
}

async fn account_options(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, AppError> {
    Ok(Json(serde_json::json!({
        "items": multiplier_monitor_dao::list_account_options(&state.pool).await?
    })))
}

async fn logs(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(query): Query<MultiplierMonitorLogQuery>,
) -> Result<Json<serde_json::Value>, AppError> {
    if multiplier_monitor_dao::get_config(&state.pool, &id)
        .await?
        .is_none()
    {
        return Err(AppError::NotFound("倍率监控配置不存在".into()));
    }
    let (items, total) = multiplier_monitor_dao::list_logs(
        &state.pool,
        &id,
        query.offset.unwrap_or(0).max(0),
        query.limit.unwrap_or(50).clamp(1, 200),
        query.result.as_deref(),
    )
    .await?;
    Ok(Json(serde_json::json!({ "items": items, "total": total })))
}
