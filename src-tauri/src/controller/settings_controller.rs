use crate::{
    app_state::AppState, error::AppError, schema::settings_schema::MonitoringSettingsUpdate,
    service::settings_service,
};
use axum::{extract::State, routing::get, Json, Router};
use std::sync::Arc;
pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/settings", get(get_settings).put(update_settings))
        .route(
            "/api/settings/monitoring",
            axum::routing::put(update_monitoring),
        )
}
async fn get_settings(State(s): State<Arc<AppState>>) -> Json<crate::config::Settings> {
    Json(s.settings.read().await.clone())
}
async fn update_settings(
    State(s): State<Arc<AppState>>,
    Json(value): Json<crate::config::Settings>,
) -> Result<Json<crate::config::Settings>, AppError> {
    Ok(Json(settings_service::update(&s.settings, value).await?))
}
async fn update_monitoring(
    State(s): State<Arc<AppState>>,
    Json(value): Json<MonitoringSettingsUpdate>,
) -> Result<Json<serde_json::Value>, AppError> {
    let (monitoring_enabled, monitoring_start_time, monitoring_end_time) =
        settings_service::update_monitoring(
            &s.settings,
            value.monitoring_enabled,
            value.monitoring_start_time,
            value.monitoring_end_time,
        )
        .await?;
    Ok(Json(serde_json::json!({
        "monitoring_enabled": monitoring_enabled,
        "monitoring_start_time": monitoring_start_time,
        "monitoring_end_time": monitoring_end_time
    })))
}
