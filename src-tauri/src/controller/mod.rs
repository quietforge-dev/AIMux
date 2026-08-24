pub mod account_controller;
pub mod anthropic_controller;
pub mod middleware;
pub mod model_controller;
pub mod monitor_controller;
pub mod openai_controller;
pub mod settings_controller;
pub mod statistics_controller;
pub mod usage_controller;

use std::{
    net::{AddrParseError, SocketAddr},
    sync::Arc,
};

use axum::{extract::DefaultBodyLimit, middleware::from_fn_with_state, routing::get, Router};
use tokio::net::TcpListener;
use tower_http::cors::CorsLayer;

use crate::{app_state::AppState, error::AppError};

const MAX_REQUEST_BODY_BYTES: usize = 512 * 1024 * 1024;

pub async fn serve(state: Arc<AppState>) -> Result<(), AppError> {
    let settings = state.settings.read().await.clone();
    let app: Router<Arc<AppState>> = Router::new()
        .merge(openai_controller::routes())
        .merge(anthropic_controller::routes())
        .merge(account_controller::routes())
        .merge(model_controller::routes())
        .merge(usage_controller::routes())
        .merge(statistics_controller::routes())
        .merge(monitor_controller::routes())
        .merge(settings_controller::routes())
        .route(
            "/health",
            get(|| async { axum::Json(serde_json::json!({"status":"ok"})) }),
        )
        .layer(DefaultBodyLimit::max(MAX_REQUEST_BODY_BYTES))
        .layer(from_fn_with_state(Arc::clone(&state), middleware::auth))
        .layer(CorsLayer::permissive());
    let addr: SocketAddr = format!("{}:{}", settings.host, settings.port)
        .parse::<SocketAddr>()
        .map_err(|error: AddrParseError| AppError::Internal(error.to_string()))?;
    let listener = TcpListener::bind(addr)
        .await
        .map_err(|error| AppError::Internal(error.to_string()))?;
    tracing::info!(%addr, "AIMux HTTP 服务已启动");
    axum::serve(listener, app.with_state(state))
        .await
        .map_err(|error| AppError::Internal(error.to_string()))
}
