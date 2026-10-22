use crate::{app_state::AppState, error::AppError, service::gateway_service};
use axum::{
    extract::State,
    http::HeaderMap,
    response::Response,
    routing::{get, post},
    Json, Router,
};
use std::sync::Arc;
pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/v1/messages", post(messages))
        .route("/v1/messages/count_tokens", post(count_tokens))
        .route("/v1/messages/batches", post(batches))
        .route("/v1/complete", post(complete))
        .route("/v1/anthropic/models", get(models))
}
async fn forward(
    State(s): State<Arc<AppState>>,
    headers: HeaderMap,
    endpoint: &str,
    body: serde_json::Value,
) -> Result<Response, AppError> {
    let settings = s.settings.read().await.clone();
    gateway_service::forward(
        &s.pool,
        &settings,
        &s.cache_priority,
        body,
        endpoint,
        "anthropic",
        headers,
    )
    .await
}
macro_rules! endpoint {
    ($name:ident,$path:literal) => {
        async fn $name(
            State(s): State<Arc<AppState>>,
            headers: HeaderMap,
            body: axum::extract::Json<serde_json::Value>,
        ) -> Result<Response, AppError> {
            forward(State(s), headers, $path, body.0).await
        }
    };
}
endpoint!(messages, "/v1/messages");
endpoint!(count_tokens, "/v1/messages/count_tokens");
endpoint!(batches, "/v1/messages/batches");
endpoint!(complete, "/v1/complete");
async fn models(State(s): State<Arc<AppState>>) -> Result<Json<serde_json::Value>, AppError> {
    let all = gateway_service::models(&s.pool, "anthropic").await?;
    Ok(Json(serde_json::json!({"data":all["data"]})))
}
