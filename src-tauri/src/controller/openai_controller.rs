use crate::{app_state::AppState, error::AppError, service::gateway_service};
use axum::{
    extract::{Path, State},
    http::HeaderMap,
    response::Response,
    routing::{get, post},
    Json, Router,
};
use std::sync::Arc;
pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/v1/chat/completions", post(chat))
        .route("/v1/completions", post(completions))
        .route("/v1/responses", post(responses))
        .route("/v1/responses/{id}/cancel", post(responses))
        .route("/v1/responses/{id}/compact", post(responses))
        .route("/v1/embeddings", post(embeddings))
        .route("/v1/moderations", post(moderations))
        .route("/v1/images/generations", post(images))
        .route("/v1/audio/speech", post(speech))
        .route("/v1/rerank", post(rerank))
        .route("/v1/models", get(models))
        .route("/v1/models/{id}", get(model))
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
        "openai",
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
endpoint!(chat, "/v1/chat/completions");
endpoint!(completions, "/v1/completions");
endpoint!(responses, "/v1/responses");
endpoint!(embeddings, "/v1/embeddings");
endpoint!(moderations, "/v1/moderations");
endpoint!(images, "/v1/images/generations");
endpoint!(speech, "/v1/audio/speech");
endpoint!(rerank, "/v1/rerank");
async fn models(State(s): State<Arc<AppState>>) -> Result<Json<serde_json::Value>, AppError> {
    Ok(Json(gateway_service::models(&s.pool, "openai").await?))
}
async fn model(
    State(s): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    let all = gateway_service::models(&s.pool, "openai").await?;
    if all["data"]
        .as_array()
        .is_some_and(|items| items.iter().any(|v| v["id"] == id))
    {
        Ok(Json(
            serde_json::json!({"id":id,"object":"model","owned_by":"aimux"}),
        ))
    } else {
        Err(AppError::NotFound("模型不存在".into()))
    }
}
