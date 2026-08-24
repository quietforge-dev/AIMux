use crate::{
    app_state::AppState,
    dao::{account_dao, monitor_dao},
    error::AppError,
    utils::time::utc_hours_ago_string,
};
use axum::{
    extract::{Query, State},
    routing::get,
    Json, Router,
};
use serde::Deserialize;
use std::{collections::HashMap, sync::Arc};
#[derive(Deserialize, Default)]
struct Q {
    limit: Option<i64>,
}
pub fn routes() -> Router<Arc<AppState>> {
    Router::new().route("/api/monitor/records", get(records))
}
async fn records(
    State(s): State<Arc<AppState>>,
    Query(q): Query<Q>,
) -> Result<Json<serde_json::Value>, AppError> {
    let (accounts, _) = account_dao::list(&s.pool, 0, 10000, None, Some("active")).await?;
    let ids = accounts.iter().map(|a| a.id.clone()).collect::<Vec<_>>();
    let since = utc_hours_ago_string(2);
    let rows = monitor_dao::list_grouped(&s.pool, &ids, q.limit.unwrap_or(30).clamp(1, 30), &since)
        .await?;
    let mut grouped: HashMap<String, Vec<_>> = HashMap::new();
    for r in rows {
        grouped.entry(r.account_id.clone()).or_default().push(serde_json::json!({"checked_at":r.checked_at,"model":r.model,"success":r.success,"duration_ms":r.duration_ms,"status_code":r.status_code,"error_code":r.error_code,"error_message":r.error_message}));
    }
    let items=accounts.into_iter().map(|a|{let mut rs=grouped.remove(&a.id).unwrap_or_default();rs.reverse();serde_json::json!({"account_id":a.id,"account_name":a.name,"account_type":a.r#type,"multiplier":a.multiplier,"priority":a.priority,"model":a.test_default_model,"monitor_average_duration_ms":a.monitor_average_duration_ms,"records":rs})}).collect::<Vec<_>>();
    let enabled = s.settings.read().await.monitoring_enabled;
    Ok(Json(
        serde_json::json!({"items":items,"monitoring_enabled":enabled}),
    ))
}
