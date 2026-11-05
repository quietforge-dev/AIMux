use crate::{
    app_state::{AppState, ServerState},
    dao::{account_dao, monitor_dao},
    error::AppError,
    service::monitor_service,
};
use axum::{
    extract::{Extension, Query},
    routing::get,
    Json, Router,
};
use serde::Deserialize;
use std::{collections::HashMap, sync::Arc};
#[derive(Deserialize, Default)]
struct Q {
    limit: Option<i64>,
}
pub fn routes() -> Router<Arc<ServerState>> {
    Router::new().route("/api/monitor/records", get(records))
}
async fn records(
    Extension(s): Extension<Arc<AppState>>,
    Query(q): Query<Q>,
) -> Result<Json<serde_json::Value>, AppError> {
    let (accounts, _) = account_dao::list(&s.pool, 0, 10000, None, Some("active"), None).await?;
    let ids = accounts.iter().map(|a| a.id.clone()).collect::<Vec<_>>();
    let throughputs = monitor_service::recent_average_throughputs(&s.pool, &ids).await?;
    let settings = s.settings.read().await.clone();
    let rows = monitor_dao::list_grouped(
        &s.pool,
        &ids,
        q.limit
            .unwrap_or(settings.monitoring_recent_count)
            .clamp(10, 40),
    )
    .await?;
    let mut grouped: HashMap<String, Vec<_>> = HashMap::new();
    for r in rows {
        grouped.entry(r.account_id.clone()).or_default().push(serde_json::json!({"checked_at":r.checked_at,"model":r.model,"success":r.success,"duration_ms":r.duration_ms,"status_code":r.status_code,"error_code":r.error_code,"error_message":r.error_message}));
    }
    let items = accounts
        .into_iter()
        .map(|a| {
            let mut records = grouped.remove(&a.id).unwrap_or_default();
            records.reverse();
            serde_json::json!({
                "account_id": a.id,
                "account_name": a.name,
                "account_type": a.r#type,
                "provider": a.provider,
                "super_priority": a.super_priority,
                "account_status": a.status,
                "multiplier": a.multiplier,
                "priority": a.priority,
                "model": a.test_default_model,
                "monitor_average_duration_ms": a.monitor_average_duration_ms,
                "recent_average_throughput": throughputs.get(&a.id),
                "records": records
            })
        })
        .collect::<Vec<_>>();
    Ok(Json(serde_json::json!({
        "items":items,
        "monitoring_enabled":settings.monitoring_enabled,
        "monitoring_start_time":settings.monitoring_start_time,
        "monitoring_end_time":settings.monitoring_end_time,
        "monitoring_weekdays":settings.monitoring_weekdays,
        "monitoring_interval_minutes":settings.monitoring_interval_minutes,
        "monitoring_recent_count":settings.monitoring_recent_count
    })))
}
