use crate::{error::AppError, model::usage_record::UsageRecord};
use sqlx::SqlitePool;

pub async fn create(pool: &SqlitePool, r: &UsageRecord) -> Result<(), AppError> {
    sqlx::query("INSERT INTO usage_records(id,trace_id,started_at,ended_at,duration_ms,first_token_ms,account_id,account_name,account_type,model,reasoning_effort,endpoint,stream,success,status_code,error_code,error_message,input_tokens,output_tokens,total_tokens,cached_tokens,client_ip,attempts) VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)").bind(&r.id).bind(&r.trace_id).bind(&r.started_at).bind(&r.ended_at).bind(r.duration_ms).bind(r.first_token_ms).bind(&r.account_id).bind(&r.account_name).bind(&r.account_type).bind(&r.model).bind(&r.reasoning_effort).bind(&r.endpoint).bind(r.stream).bind(r.success).bind(r.status_code).bind(&r.error_code).bind(&r.error_message).bind(r.input_tokens).bind(r.output_tokens).bind(r.total_tokens).bind(r.cached_tokens).bind(&r.client_ip).bind(r.attempts).execute(pool).await?;
    Ok(())
}

pub async fn finish_stream(
    pool: &SqlitePool,
    id: &str,
    ended_at: &str,
    duration_ms: i64,
    first_token_ms: Option<i64>,
    success: bool,
    status_code: Option<i64>,
    error_code: Option<&str>,
    error_message: Option<&str>,
    tokens: (Option<i64>, Option<i64>, Option<i64>, Option<i64>),
) -> Result<(), AppError> {
    let (input, output, total, cached) = tokens;
    let result = sqlx::query("UPDATE usage_records SET ended_at=?,duration_ms=?,first_token_ms=?,success=?,status_code=?,error_code=?,error_message=?,input_tokens=?,output_tokens=?,total_tokens=?,cached_tokens=? WHERE id=?")
        .bind(ended_at)
        .bind(duration_ms)
        .bind(first_token_ms)
        .bind(success)
        .bind(status_code)
        .bind(error_code)
        .bind(error_message)
        .bind(input)
        .bind(output)
        .bind(total)
        .bind(cached)
        .bind(id)
        .execute(pool)
        .await?;
    if result.rows_affected() != 1 {
        return Err(AppError::Internal(format!(
            "流式使用记录结束更新异常，期望 1 条，实际 {} 条",
            result.rows_affected()
        )));
    }
    Ok(())
}
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
) -> Result<(Vec<UsageRecord>, i64), AppError> {
    let mut where_sql = String::from(" WHERE 1=1");
    let mut vals: Vec<String> = Vec::new();
    if let Some(v) = account_id {
        where_sql.push_str(" AND account_id=?");
        vals.push(v.into())
    }
    if let Some(v) = model {
        where_sql.push_str(" AND model=?");
        vals.push(v.into())
    }
    if let Some(v) = kind {
        where_sql.push_str(" AND account_type=?");
        vals.push(v.into())
    }
    if let Some(v) = success {
        where_sql.push_str(" AND success=?");
        vals.push((v as i32).to_string())
    }
    if let Some(v) = started_after {
        where_sql.push_str(" AND started_at>=?");
        vals.push(v.into())
    }
    if let Some(v) = started_before {
        where_sql.push_str(" AND started_at<=?");
        vals.push(v.into())
    }
    let q = format!(
        "SELECT * FROM usage_records{} ORDER BY started_at DESC,id DESC LIMIT ? OFFSET ?",
        where_sql
    );
    let mut query = sqlx::query_as::<_, UsageRecord>(&q);
    for v in &vals {
        query = query.bind(v);
    }
    let rows = query.bind(limit).bind(offset).fetch_all(pool).await?;
    let cq = format!("SELECT COUNT(*) FROM usage_records{}", where_sql);
    let mut count = sqlx::query_scalar::<_, i64>(&cq);
    for v in &vals {
        count = count.bind(v);
    }
    Ok((rows, count.fetch_one(pool).await?))
}
pub async fn get(pool: &SqlitePool, id: &str) -> Result<Option<UsageRecord>, AppError> {
    Ok(
        sqlx::query_as::<_, UsageRecord>("SELECT * FROM usage_records WHERE id=?")
            .bind(id)
            .fetch_optional(pool)
            .await?,
    )
}
pub async fn summary(
    pool: &SqlitePool,
    account_id: Option<&str>,
    model: Option<&str>,
    kind: Option<&str>,
    success: Option<bool>,
    started_after: Option<&str>,
    started_before: Option<&str>,
) -> Result<(i64, i64, f64, i64), AppError> {
    let mut where_sql = String::from(" WHERE 1=1");
    let mut vals: Vec<String> = Vec::new();
    if let Some(v) = account_id {
        where_sql.push_str(" AND account_id=?");
        vals.push(v.into())
    }
    if let Some(v) = model {
        where_sql.push_str(" AND model=?");
        vals.push(v.into())
    }
    if let Some(v) = kind {
        where_sql.push_str(" AND account_type=?");
        vals.push(v.into())
    }
    if let Some(v) = success {
        where_sql.push_str(" AND success=?");
        vals.push((v as i32).to_string())
    }
    if let Some(v) = started_after {
        where_sql.push_str(" AND started_at>=?");
        vals.push(v.into())
    }
    if let Some(v) = started_before {
        where_sql.push_str(" AND started_at<=?");
        vals.push(v.into())
    }
    let sql = format!(
        "SELECT COUNT(*),COALESCE(SUM(success),0),COALESCE(AVG(duration_ms),0),COALESCE(SUM(total_tokens),0) FROM usage_records{}",
        where_sql
    );
    let mut query = sqlx::query_as::<_, (i64, i64, f64, i64)>(&sql);
    for value in vals {
        query = query.bind(value);
    }
    Ok(query.fetch_one(pool).await?)
}
pub async fn token_totals(
    pool: &SqlitePool,
    started_after: Option<&str>,
    started_before: Option<&str>,
) -> Result<(i64, i64, i64, i64), AppError> {
    let mut sql = String::from(
        "SELECT COALESCE(SUM(input_tokens),0),COALESCE(SUM(output_tokens),0),COALESCE(SUM(cached_tokens),0),COALESCE(SUM(total_tokens),0) FROM usage_records WHERE 1=1",
    );
    let mut values = Vec::new();
    if let Some(value) = started_after {
        sql.push_str(" AND started_at >= ?");
        values.push(value);
    }
    if let Some(value) = started_before {
        sql.push_str(" AND started_at < ?");
        values.push(value);
    }
    let mut query = sqlx::query_as::<_, (i64, i64, i64, i64)>(&sql);
    for value in values {
        query = query.bind(value);
    }
    Ok(query.fetch_one(pool).await?)
}
pub async fn token_totals_by_account(
    pool: &SqlitePool,
    started_after: &str,
    started_before: &str,
) -> Result<Vec<(String, i64, i64, i64, i64)>, AppError> {
    Ok(sqlx::query_as::<_, (String, i64, i64, i64, i64)>(
        r#"
            SELECT account_id,
                   COALESCE(SUM(input_tokens), 0),
                   COALESCE(SUM(output_tokens), 0),
                   COALESCE(SUM(cached_tokens), 0),
                   COALESCE(SUM(total_tokens), 0)
            FROM usage_records
            WHERE started_at >= ? AND started_at < ? AND account_id IS NOT NULL
            GROUP BY account_id
        "#,
    )
    .bind(started_after)
    .bind(started_before)
    .fetch_all(pool)
    .await?)
}
pub async fn recent_cache_rates_by_account(
    pool: &SqlitePool,
    started_after: &str,
    started_before: &str,
    limit_per_account: i64,
) -> Result<Vec<(String, i64, i64, i64)>, AppError> {
    Ok(sqlx::query_as::<_, (String, i64, i64, i64)>(
        r#"
            WITH ranked AS (
                SELECT account_id,
                       input_tokens,
                       cached_tokens,
                       ROW_NUMBER() OVER (
                           PARTITION BY account_id
                           ORDER BY started_at DESC, id DESC
                       ) AS row_number
                FROM usage_records
                WHERE account_id IS NOT NULL
                  AND started_at >= ?
                  AND started_at < ?
                  AND input_tokens > 0
                  AND cached_tokens IS NOT NULL
            )
            SELECT account_id,
                   COALESCE(SUM(input_tokens), 0),
                   COALESCE(SUM(cached_tokens), 0),
                   COUNT(*)
            FROM ranked
            WHERE row_number <= ?
            GROUP BY account_id
        "#,
    )
    .bind(started_after)
    .bind(started_before)
    .bind(limit_per_account)
    .fetch_all(pool)
    .await?)
}
pub async fn cleanup(pool: &SqlitePool, cutoff: &str) -> Result<i64, AppError> {
    let r = sqlx::query("DELETE FROM usage_records WHERE started_at < ?")
        .bind(cutoff)
        .execute(pool)
        .await?;
    Ok(r.rows_affected() as i64)
}

pub async fn fail_unfinished_streams(pool: &SqlitePool, ended_at: &str) -> Result<i64, AppError> {
    let result = sqlx::query(
        "UPDATE usage_records SET ended_at=?,duration_ms=CAST((julianday(?) - julianday(started_at))*86400000 AS INTEGER),success=0,status_code=504,error_code='stream_interrupted',error_message='流式请求因网关重启中断' WHERE stream=1 AND ended_at IS NULL",
    )
    .bind(ended_at)
    .bind(ended_at)
    .execute(pool)
    .await?;
    Ok(result.rows_affected() as i64)
}
