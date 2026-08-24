use crate::{
    dao::{account_dao, usage_dao},
    error::AppError,
    schema::usage_schema::TokenSummary,
};
use chrono::{Duration, Local, TimeZone, Utc};
use sqlx::SqlitePool;
use std::collections::HashMap;

const RECENT_CACHE_RATE_RECORD_LIMIT: i64 = 20;

pub async fn tokens(pool: &SqlitePool) -> Result<serde_json::Value, AppError> {
    let today = Local::now().date_naive();
    let start_today = Local
        .from_local_datetime(&today.and_hms_opt(0, 0, 0).unwrap())
        .single()
        .unwrap()
        .with_timezone(&Utc);
    let start_yesterday = start_today - Duration::days(1);
    let total = range(pool, None, None).await?;
    let yesterday = range(pool, Some(start_yesterday), Some(start_today)).await?;
    let today_summary = range(
        pool,
        Some(start_today),
        Some(start_today + Duration::days(1)),
    )
    .await?;
    let (accounts, _) = account_dao::list(pool, 0, 10000, None, Some("active")).await?;
    let mut account_summaries =
        range_for_accounts(pool, start_today, start_today + Duration::days(1)).await?;
    let mut recent_cache_rates =
        recent_cache_rates_for_accounts(pool, start_today, start_today + Duration::days(1)).await?;
    let mut account_today = Vec::new();
    for account in accounts {
        let s = account_summaries
            .remove(&account.id)
            .unwrap_or_else(|| summary(0, 0, 0, 0));
        let recent = recent_cache_rates.remove(&account.id);
        account_today.push(serde_json::json!({"account_id":account.id,"account_name":account.name,"account_type":account.r#type,"multiplier":account.multiplier,"priority":account.priority,"input_tokens":s.input_tokens,"output_tokens":s.output_tokens,"cached_tokens":s.cached_tokens,"total_tokens":s.total_tokens,"cache_rate":s.cache_rate,"recent_cache_rate":recent.as_ref().and_then(|value| value.cache_rate),"recent_cache_count":recent.map_or(0, |value| value.count)}));
    }
    Ok(
        serde_json::json!({"total":total,"yesterday":yesterday,"today":today_summary,"accounts_today":account_today}),
    )
}
async fn range(
    pool: &SqlitePool,
    start: Option<chrono::DateTime<Utc>>,
    end: Option<chrono::DateTime<Utc>>,
) -> Result<TokenSummary, AppError> {
    let start = start.map(|value| value.format("%Y-%m-%dT%H:%M:%SZ").to_string());
    let end = end.map(|value| value.format("%Y-%m-%dT%H:%M:%SZ").to_string());
    let (a, b, c, d) = usage_dao::token_totals(pool, start.as_deref(), end.as_deref()).await?;
    Ok(summary(a, b, c, d))
}
pub async fn range_for_accounts(
    pool: &SqlitePool,
    start: chrono::DateTime<Utc>,
    end: chrono::DateTime<Utc>,
) -> Result<HashMap<String, TokenSummary>, AppError> {
    let start = start.format("%Y-%m-%dT%H:%M:%SZ").to_string();
    let end = end.format("%Y-%m-%dT%H:%M:%SZ").to_string();
    let rows = usage_dao::token_totals_by_account(pool, &start, &end).await?;
    Ok(rows
        .into_iter()
        .map(|(account_id, input, output, cached, total)| {
            (account_id, summary(input, output, cached, total))
        })
        .collect())
}
struct RecentCacheRate {
    cache_rate: Option<f64>,
    count: i64,
}
async fn recent_cache_rates_for_accounts(
    pool: &SqlitePool,
    start: chrono::DateTime<Utc>,
    end: chrono::DateTime<Utc>,
) -> Result<HashMap<String, RecentCacheRate>, AppError> {
    let start = start.format("%Y-%m-%dT%H:%M:%SZ").to_string();
    let end = end.format("%Y-%m-%dT%H:%M:%SZ").to_string();
    let rows = usage_dao::recent_cache_rates_by_account(
        pool,
        &start,
        &end,
        RECENT_CACHE_RATE_RECORD_LIMIT,
    )
    .await?;
    Ok(rows
        .into_iter()
        .map(|(account_id, input, cached, count)| {
            (
                account_id,
                RecentCacheRate {
                    cache_rate: (input > 0).then(|| cached as f64 / input as f64),
                    count,
                },
            )
        })
        .collect())
}
fn summary(a: i64, b: i64, c: i64, d: i64) -> TokenSummary {
    TokenSummary {
        input_tokens: a,
        output_tokens: b,
        cached_tokens: c,
        total_tokens: d,
        cache_rate: if a == 0 {
            None
        } else {
            Some(c as f64 / a as f64)
        },
    }
}
