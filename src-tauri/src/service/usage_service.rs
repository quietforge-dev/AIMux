use crate::{
    dao::usage_dao,
    error::AppError,
    model::usage_record::UsageRecord,
    schema::usage_schema::{UsageResponse, UsageSummary},
    utils::time::utc_days_ago_string,
};
use sqlx::SqlitePool;

pub async fn list(
    pool: &SqlitePool,
    offset: i64,
    limit: i64,
    account_id: Option<&str>,
    model: Option<&str>,
    kind: Option<&str>,
    success: Option<bool>,
    started_after: Option<&str>,
    started_before: Option<&str>,
) -> Result<UsageResponse<UsageRecord>, AppError> {
    let (items, total) = usage_dao::list(
        pool,
        offset.max(0),
        limit.clamp(1, 200),
        account_id,
        model,
        kind,
        success,
        started_after,
        started_before,
    )
    .await?;
    let summary = summary(
        pool,
        account_id,
        model,
        kind,
        success,
        started_after,
        started_before,
    )
    .await?;
    Ok(UsageResponse {
        items,
        total,
        summary,
    })
}
async fn summary(
    pool: &SqlitePool,
    account_id: Option<&str>,
    model: Option<&str>,
    kind: Option<&str>,
    success: Option<bool>,
    started_after: Option<&str>,
    started_before: Option<&str>,
) -> Result<UsageSummary, AppError> {
    let (count, ok, avg, tokens) = usage_dao::summary(
        pool,
        account_id,
        model,
        kind,
        success,
        started_after,
        started_before,
    )
    .await?;
    Ok(UsageSummary {
        request_count: count,
        success_rate: if count == 0 {
            0.0
        } else {
            ok as f64 / count as f64
        },
        average_duration_ms: avg.round() as i64,
        total_tokens: tokens,
    })
}
pub async fn detail(pool: &SqlitePool, id: &str) -> Result<Option<UsageRecord>, AppError> {
    usage_dao::get(pool, id).await
}
pub async fn cleanup(pool: &SqlitePool, days: i64) -> Result<i64, AppError> {
    if !matches!(days, 7 | 30 | 90) {
        return Err(AppError::BadRequest("清理天数只支持 7、30 或 90 天".into()));
    }
    usage_dao::cleanup(pool, &utc_days_ago_string(days)).await
}
