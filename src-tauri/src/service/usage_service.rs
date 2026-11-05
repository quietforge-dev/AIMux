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
    reasoning_effort: Option<&str>,
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
        reasoning_effort,
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
        reasoning_effort,
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
    reasoning_effort: Option<&str>,
) -> Result<UsageSummary, AppError> {
    let (count, ok, avg, tokens) = usage_dao::summary(
        pool,
        account_id,
        model,
        kind,
        success,
        started_after,
        started_before,
        reasoning_effort,
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
    super::daily_statistics_service::catch_up(pool, chrono::Local::now()).await?;
    usage_dao::cleanup(pool, &utc_days_ago_string(days)).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn reasoning_filters_keep_rows_counts_summary_and_pagination_consistent() {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::raw_sql(include_str!("../../migrations/0001_baseline.sql"))
            .execute(&pool)
            .await
            .unwrap();
        for (id, effort, success) in [
            ("1", None, true),
            ("2", Some(""), true),
            ("3", Some(" \t\r\n "), true),
            ("4", Some("low"), true),
            ("5", Some("high"), false),
            ("6", Some("none"), true),
            ("7", Some("low"), false),
        ] {
            sqlx::query("INSERT INTO usage_records(id,trace_id,started_at,account_type,model,reasoning_effort,stream,success,attempts,total_tokens,duration_ms) VALUES(?,?,'2026-11-05T08:00:00Z','openai','test-model',?,0,?,1,10,100)")
                .bind(id).bind(id).bind(effort).bind(success).execute(&pool).await.unwrap();
        }
        for (effort, expected, ok) in [
            (None, 7, 5),
            (Some("__nonempty__"), 4, 2),
            (Some("low"), 2, 1),
            (Some("none"), 1, 1),
            (Some("xhigh"), 0, 0),
            (Some("low' OR 1=1 --"), 0, 0),
        ] {
            let result = list(&pool, 0, 200, None, None, None, None, None, None, effort)
                .await
                .unwrap();
            assert_eq!(result.total, expected);
            assert_eq!(result.items.len() as i64, expected);
            assert_eq!(result.summary.request_count, expected);
            assert_eq!(result.summary.total_tokens, expected * 10);
            assert_eq!(
                result.summary.success_rate,
                if expected == 0 {
                    0.0
                } else {
                    ok as f64 / expected as f64
                }
            );
        }
        let result = list(
            &pool,
            1,
            1,
            None,
            Some("test-model"),
            Some("openai"),
            Some(true),
            Some("2026-11-05T00:00:00Z"),
            Some("2026-11-05T23:59:59Z"),
            Some("__nonempty__"),
        )
        .await
        .unwrap();
        assert_eq!(result.total, 2);
        assert_eq!(result.items.len(), 1);
        assert_eq!(result.items[0].id, "4");
        assert_eq!(result.summary.request_count, 2);
        assert_eq!(result.summary.total_tokens, 20);
    }
}
