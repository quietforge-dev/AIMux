use crate::{
    app_state::AppState,
    dao::model_dao,
    error::AppError,
    schema::model_schema::{ModelCreate, ModelUpdate},
};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, post, put},
    Json, Router,
};
use serde::Deserialize;
use std::sync::Arc;
#[derive(Deserialize, Default)]
struct Q {
    #[serde(rename = "type")]
    kind: Option<String>,
}
pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/models", get(list).post(create))
        .route("/api/models/{id}", put(update).delete(remove))
        .route("/api/models/{id}/set-default", post(default_model))
}
async fn list(
    State(s): State<Arc<AppState>>,
    Query(q): Query<Q>,
) -> Result<Json<serde_json::Value>, AppError> {
    Ok(Json(
        serde_json::json!({"items":model_dao::list(&s.pool,q.kind.as_deref()).await?.into_iter().map(model_dao::to_view).collect::<Vec<_>>()}),
    ))
}
async fn create(
    State(s): State<Arc<AppState>>,
    Json(p): Json<ModelCreate>,
) -> Result<Json<serde_json::Value>, AppError> {
    Ok(Json(
        serde_json::to_value(model_dao::to_view(model_dao::create(&s.pool, p).await?)).unwrap(),
    ))
}
async fn update(
    State(s): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(p): Json<ModelUpdate>,
) -> Result<Json<serde_json::Value>, AppError> {
    let m = model_dao::get(&s.pool, &id)
        .await?
        .ok_or_else(|| AppError::NotFound("模型不存在".into()))?;
    Ok(Json(
        serde_json::to_value(model_dao::to_view(model_dao::update(&s.pool, m, p).await?)).unwrap(),
    ))
}
async fn remove(
    State(s): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<StatusCode, AppError> {
    model_dao::delete(&s.pool, &id).await?;
    Ok(StatusCode::NO_CONTENT)
}
async fn default_model(
    State(s): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    let m = model_dao::get(&s.pool, &id)
        .await?
        .ok_or_else(|| AppError::NotFound("模型不存在".into()))?;
    Ok(Json(
        serde_json::to_value(model_dao::to_view(
            model_dao::set_default(&s.pool, m).await?,
        ))
        .unwrap(),
    ))
}
