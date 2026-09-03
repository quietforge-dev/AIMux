use crate::{
    app_state::AppState,
    dao::account_dao,
    error::AppError,
    schema::account_schema::{
        AccountCreate, AccountUpdate, DiscoverModelsRequest, DiscoverModelsResponse, TestRequest,
    },
    service::{account_model_discovery_service, account_probe_service, account_service},
    upstream::error::response_body,
};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use std::sync::Arc;

#[derive(Deserialize, Default)]
pub struct AccountQuery {
    pub offset: Option<i64>,
    pub limit: Option<i64>,
    #[serde(rename = "type")]
    pub account_type: Option<String>,
    pub status: Option<String>,
    pub name: Option<String>,
}
pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/accounts", get(list).post(create))
        .route(
            "/api/accounts/{id}",
            get(get_one).put(update).delete(remove),
        )
        .route("/api/accounts/{id}/toggle-status", post(toggle))
        .route("/api/accounts/{id}/adjust-priority", post(adjust))
        .route("/api/accounts/{id}/test", post(test))
        .route("/api/accounts/discover-models", post(discover_models))
}
async fn list(
    State(s): State<Arc<AppState>>,
    Query(q): Query<AccountQuery>,
) -> Result<Json<serde_json::Value>, AppError> {
    let (items, total) = account_dao::list(
        &s.pool,
        q.offset.unwrap_or(0).max(0),
        q.limit.unwrap_or(50).clamp(1, 200),
        q.account_type.as_deref(),
        q.status.as_deref(),
        q.name.as_deref(),
    )
    .await?;
    Ok(Json(
        serde_json::json!({"items":items.into_iter().map(account_dao::to_view).collect::<Vec<_>>(),"total":total}),
    ))
}
async fn get_one(
    State(s): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    Ok(Json(
        serde_json::to_value(account_service::to_view(&s.pool, &id).await?).unwrap(),
    ))
}
async fn create(
    State(s): State<Arc<AppState>>,
    Json(p): Json<AccountCreate>,
) -> Result<Json<serde_json::Value>, AppError> {
    Ok(Json(
        serde_json::to_value(account_dao::to_view(account_dao::create(&s.pool, p).await?)).unwrap(),
    ))
}
async fn update(
    State(s): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(p): Json<AccountUpdate>,
) -> Result<Json<serde_json::Value>, AppError> {
    let current = account_dao::get(&s.pool, &id)
        .await?
        .ok_or_else(|| AppError::NotFound("账号不存在".into()))?;
    Ok(Json(
        serde_json::to_value(account_dao::to_view(
            account_dao::update(&s.pool, current, p).await?,
        ))
        .unwrap(),
    ))
}
async fn remove(
    State(s): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<StatusCode, AppError> {
    account_dao::delete(&s.pool, &id).await?;
    Ok(StatusCode::NO_CONTENT)
}
async fn discover_models(
    State(s): State<Arc<AppState>>,
    Json(p): Json<DiscoverModelsRequest>,
) -> Result<Json<DiscoverModelsResponse>, AppError> {
    let settings = s.settings.read().await.clone();
    let models = account_model_discovery_service::discover(
        &p.account_type,
        &p.base_url,
        &p.api_key,
        &settings,
    )
    .await?;
    Ok(Json(DiscoverModelsResponse { models }))
}
async fn toggle(
    State(s): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    let a = account_dao::toggle_status(&s.pool, &id)
        .await?
        .ok_or_else(|| AppError::NotFound("账号不存在".into()))?;
    Ok(Json(serde_json::to_value(account_dao::to_view(a)).unwrap()))
}
async fn adjust(
    State(s): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(q): Query<AdjustQuery>,
) -> Result<Json<serde_json::Value>, AppError> {
    account_dao::save_priority(&s.pool, &id, q.priority.unwrap_or(5)).await?;
    Ok(Json(
        serde_json::to_value(account_service::to_view(&s.pool, &id).await?).unwrap(),
    ))
}
#[derive(Deserialize)]
struct AdjustQuery {
    priority: Option<i64>,
}
async fn test(
    State(s): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(p): Json<TestRequest>,
) -> Result<impl IntoResponse, AppError> {
    let account = account_dao::get(&s.pool, &id)
        .await?
        .ok_or_else(|| AppError::NotFound("账号不存在".into()))?;
    let Some(probe) = account_probe_service::prepare(&s.pool, &account, p.model.as_deref()).await?
    else {
        return Err(AppError::BadRequest("没有可用测试模型".into()));
    };
    let model = probe.model.clone();
    let settings = s.settings.read().await.clone();
    let response = account_probe_service::send(&account, &probe, &settings, None).await;
    match response {
        Ok(r) => {
            let status = r.status();
            let bytes = r.bytes().await.unwrap_or_default();
            let ok = status.is_success();
            let response_body = response_body(&bytes);
            if ok {
                tracing::info!(
                    account_id = %account.id,
                    account_name = %account.name,
                    status_code = status.as_u16(),
                    "账号测试成功"
                );
            } else {
                tracing::error!(
                    account_id = %account.id,
                    account_name = %account.name,
                    status_code = status.as_u16(),
                    "账号测试失败"
                );
            }
            account_dao::adjust(
                &s.pool,
                &id,
                ok,
                "request",
                if ok { None } else { Some("test_failed") },
                if ok { None } else { Some("测试失败") },
            )
            .await?;
            Ok(Json(
                serde_json::json!({"account_id":id,"success":ok,"status_code":status.as_u16(),"response_body":response_body,"model":model}),
            ))
        }
        Err(e) => {
            tracing::error!(account_id = %account.id, account_name = %account.name, error = %e, "账号测试连接失败");
            account_dao::adjust(
                &s.pool,
                &id,
                false,
                "request",
                Some("test_connection_error"),
                Some(&e.to_string()),
            )
            .await?;
            Ok(Json(
                serde_json::json!({"account_id":id,"success":false,"error_code":"test_connection_error","error_message":e.to_string(),"model":model}),
            ))
        }
    }
}
