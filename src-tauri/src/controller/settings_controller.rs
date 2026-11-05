use crate::{
    app_state::{AppState, ServerState},
    error::AppError,
    schema::settings_schema::MonitoringSettingsUpdate,
    service::settings_service,
};
use axum::{extract::Extension, routing::get, Json, Router};
use std::sync::Arc;
pub fn routes() -> Router<Arc<ServerState>> {
    Router::new()
        .route("/api/settings", get(get_settings).put(update_settings))
        .route(
            "/api/settings/monitoring",
            axum::routing::put(update_monitoring),
        )
}
async fn get_settings(Extension(s): Extension<Arc<AppState>>) -> Json<crate::config::Settings> {
    Json(s.settings.read().await.clone())
}
async fn update_settings(
    Extension(s): Extension<Arc<AppState>>,
    Json(value): Json<crate::config::Settings>,
) -> Result<Json<crate::config::Settings>, AppError> {
    let mut tracker = s.cache_priority.lock().await;
    let saved = settings_service::update(&s.pool, s.desktop, &s.settings, value).await?;
    tracker.configure(&saved);
    Ok(Json(saved))
}
async fn update_monitoring(
    Extension(s): Extension<Arc<AppState>>,
    Json(value): Json<MonitoringSettingsUpdate>,
) -> Result<Json<serde_json::Value>, AppError> {
    let saved = settings_service::update_monitoring(
        &s.pool,
        &s.settings,
        value.monitoring_enabled,
        value.monitoring_start_time,
        value.monitoring_end_time,
        value.monitoring_weekdays,
        value.monitoring_interval_minutes,
        value.monitoring_recent_count,
    )
    .await?;
    Ok(Json(serde_json::json!({
        "monitoring_enabled": saved.monitoring_enabled,
        "monitoring_start_time": saved.monitoring_start_time,
        "monitoring_end_time": saved.monitoring_end_time,
        "monitoring_weekdays": saved.monitoring_weekdays,
        "monitoring_interval_minutes": saved.monitoring_interval_minutes,
        "monitoring_recent_count": saved.monitoring_recent_count
    })))
}
