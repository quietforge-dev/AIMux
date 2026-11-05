use crate::database::UserDb;
use crate::{error::AppError, model::usage_record::UsageRecord};
use sqlx::{QueryBuilder, Sqlite};

pub async fn recent_average_throughputs(
    pool: &UserDb,
    account_ids: &[String],
    limit_per_account: i64,
) -> Result<Vec<(String, Option<f64>)>, AppError> {
    let mut rows = Vec::new();
    // 每个账号利用现有账号/时间索引取最近样本，避免扫描所有历史请求。
    for ids in account_ids.chunks(400) {
        let mut query = QueryBuilder::<Sqlite>::new(
            r#"SELECT a.id, (
                SELECT AVG(throughput) FROM (
                    SELECT output_tokens * 1000.0 / (
                        CASE WHEN stream = 1 AND first_token_ms IS NOT NULL
                                  AND duration_ms > first_token_ms
                             THEN duration_ms - first_token_ms
                             ELSE duration_ms END
                    ) AS throughput
                    FROM usage_records
                    WHERE user_id = a.user_id AND account_id = a.id
                      AND success = 1 AND ended_at IS NOT NULL AND ended_at <> ''
                      AND output_tokens > 0 AND duration_ms > 0
                    ORDER BY started_at DESC, id DESC
                    LIMIT "#,
        );
        query.push_bind(limit_per_account.max(1));
        query.push(") ) AS average_throughput FROM accounts a WHERE a.id IN (");
        let mut separated = query.separated(",");
        for id in ids {
            separated.push_bind(id);
        }
        separated.push_unseparated(")");
        query.push(" AND a.user_id=").push_bind(pool.user_id());
        rows.extend(
            query
                .build_query_as::<(String, Option<f64>)>()
                .fetch_all(pool.raw())
                .await?,
        );
    }
    Ok(rows)
}

struct UsageFilter<'a> {
    user_id: &'a str,
    account_id: Option<&'a str>,
    model: Option<&'a str>,
    kind: Option<&'a str>,
    success: Option<bool>,
    started_after: Option<&'a str>,
    started_before: Option<&'a str>,
    reasoning_effort: Option<&'a str>,
}

impl<'a> UsageFilter<'a> {
    fn where_clause(&self) -> (String, Vec<String>) {
        let mut where_sql = String::from(" WHERE user_id=?");
        let mut values = vec![self.user_id.to_owned()];
        if let Some(value) = self.account_id {
            where_sql.push_str(" AND account_id=?");
            values.push(value.into());
        }
        if let Some(value) = self.model {
            where_sql.push_str(" AND model=?");
            values.push(value.into());
        }
        if let Some(value) = self.reasoning_effort {
            if value == "__nonempty__" {
                where_sql.push_str(" AND reasoning_effort IS NOT NULL AND TRIM(reasoning_effort, char(9) || char(10) || char(13) || ' ') <> ''");
            } else {
                where_sql.push_str(" AND reasoning_effort=?");
                values.push(value.into());
            }
        }
        if let Some(value) = self.kind {
            where_sql.push_str(" AND account_type=?");
            values.push(value.into());
        }
        if let Some(value) = self.success {
            where_sql.push_str(" AND success=?");
            values.push((value as i32).to_string());
        }
        if let Some(value) = self.started_after {
            where_sql.push_str(" AND started_at>=?");
            values.push(value.into());
        }
        if let Some(value) = self.started_before {
            where_sql.push_str(" AND started_at<=?");
            values.push(value.into());
        }
        (where_sql, values)
    }
}

pub async fn create(pool: &UserDb, r: &UsageRecord) -> Result<(), AppError> {
    sqlx::query("INSERT INTO usage_records(user_id,id,trace_id,started_at,ended_at,duration_ms,first_token_ms,account_id,account_name,account_type,model,reasoning_effort,endpoint,stream,success,status_code,error_code,error_message,input_tokens,output_tokens,total_tokens,cached_tokens,client_ip,attempts) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21,?22,?23,?24)").bind(pool.user_id()).bind(&r.id).bind(&r.trace_id).bind(&r.started_at).bind(&r.ended_at).bind(r.duration_ms).bind(r.first_token_ms).bind(&r.account_id).bind(&r.account_name).bind(&r.account_type).bind(&r.model).bind(&r.reasoning_effort).bind(&r.endpoint).bind(r.stream).bind(r.success).bind(r.status_code).bind(&r.error_code).bind(&r.error_message).bind(r.input_tokens).bind(r.output_tokens).bind(r.total_tokens).bind(r.cached_tokens).bind(&r.client_ip).bind(r.attempts).execute(pool.raw()).await?;
    Ok(())
}

pub async fn finish_stream(
    pool: &UserDb,
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
    let result = sqlx::query("UPDATE usage_records SET ended_at=?2,duration_ms=?3,first_token_ms=?4,success=?5,status_code=?6,error_code=?7,error_message=?8,input_tokens=?9,output_tokens=?10,total_tokens=?11,cached_tokens=?12 WHERE user_id=?1 AND id=?13").bind(pool.user_id())
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
        .execute(pool.raw())
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
    pool: &UserDb,
    offset: i64,
    limit: i64,
    account_id: Option<&str>,
    model: Option<&str>,
    kind: Option<&str>,
    success: Option<bool>,
    started_after: Option<&str>,
    started_before: Option<&str>,
    reasoning_effort: Option<&str>,
) -> Result<(Vec<UsageRecord>, i64), AppError> {
    let filter = UsageFilter {
        user_id: pool.user_id(),
        account_id,
        model,
        kind,
        success,
        started_after,
        started_before,
        reasoning_effort,
    };
    let (where_sql, values) = filter.where_clause();
    let q = format!(
        "SELECT * FROM usage_records{} ORDER BY started_at DESC,id DESC LIMIT ? OFFSET ?",
        where_sql
    );
    let mut query = sqlx::query_as::<_, UsageRecord>(&q);
    for value in &values {
        query = query.bind(value);
    }
    let rows = query.bind(limit).bind(offset).fetch_all(pool.raw()).await?;
    let cq = format!("SELECT COUNT(*) FROM usage_records{}", where_sql);
    let mut count = sqlx::query_scalar::<_, i64>(&cq);
    for value in &values {
        count = count.bind(value);
    }
    Ok((rows, count.fetch_one(pool.raw()).await?))
}
pub async fn get(pool: &UserDb, id: &str) -> Result<Option<UsageRecord>, AppError> {
    Ok(
        sqlx::query_as::<_, UsageRecord>("SELECT * FROM usage_records WHERE user_id=?1 AND id=?2")
            .bind(pool.user_id())
            .bind(id)
            .fetch_optional(pool.raw())
            .await?,
    )
}
pub async fn summary(
    pool: &UserDb,
    account_id: Option<&str>,
    model: Option<&str>,
    kind: Option<&str>,
    success: Option<bool>,
    started_after: Option<&str>,
    started_before: Option<&str>,
    reasoning_effort: Option<&str>,
) -> Result<(i64, i64, f64, i64), AppError> {
    let filter = UsageFilter {
        user_id: pool.user_id(),
        account_id,
        model,
        kind,
        success,
        started_after,
        started_before,
        reasoning_effort,
    };
    let (where_sql, values) = filter.where_clause();
    let sql = format!(
        "SELECT COUNT(*),COALESCE(SUM(success),0),COALESCE(AVG(duration_ms),0.0),COALESCE(SUM(total_tokens),0) FROM usage_records{}",
        where_sql
    );
    let mut query = sqlx::query_as::<_, (i64, i64, f64, i64)>(&sql);
    for value in values {
        query = query.bind(value);
    }
    Ok(query.fetch_one(pool.raw()).await?)
}
pub async fn token_totals(
    pool: &UserDb,
    started_after: Option<&str>,
    started_before: Option<&str>,
) -> Result<(i64, i64, i64, i64), AppError> {
    let mut sql = String::from(
        "SELECT COALESCE(SUM(input_tokens),0),COALESCE(SUM(output_tokens),0),COALESCE(SUM(cached_tokens),0),COALESCE(SUM(total_tokens),0) FROM usage_records WHERE user_id=?",
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
    let mut query = sqlx::query_as::<_, (i64, i64, i64, i64)>(&sql).bind(pool.user_id());
    for value in values {
        query = query.bind(value);
    }
    Ok(query.fetch_one(pool.raw()).await?)
}
pub async fn token_totals_by_account(
    pool: &UserDb,
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
            WHERE user_id=?1 AND started_at >= ?2 AND started_at < ?3 AND account_id IS NOT NULL
            GROUP BY account_id
        "#,
    )
    .bind(pool.user_id())
    .bind(started_after)
    .bind(started_before)
    .fetch_all(pool.raw())
    .await?)
}
pub async fn recent_cache_rates_by_account(
    pool: &UserDb,
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
                WHERE user_id=?1 AND account_id IS NOT NULL
                  AND started_at >= ?2
                  AND started_at < ?3
                  AND input_tokens > 0
                  AND cached_tokens IS NOT NULL
            )
            SELECT account_id,
                   COALESCE(SUM(input_tokens), 0),
                   COALESCE(SUM(cached_tokens), 0),
                   COUNT(*)
            FROM ranked
            WHERE row_number <= ?4
            GROUP BY account_id
        "#,
    )
    .bind(pool.user_id())
    .bind(started_after)
    .bind(started_before)
    .bind(limit_per_account)
    .fetch_all(pool.raw())
    .await?)
}
pub async fn cleanup(pool: &UserDb, cutoff: &str) -> Result<i64, AppError> {
    let r = sqlx::query("DELETE FROM usage_records WHERE user_id=?1 AND started_at < ?2 AND NOT(stream=1 AND ended_at IS NULL) AND EXISTS (SELECT 1 FROM daily_usage_statistics d WHERE d.user_id=usage_records.user_id AND usage_records.started_at>=d.started_at AND usage_records.started_at<d.ended_at)").bind(pool.user_id())
        .bind(cutoff)
        .execute(pool.raw())
        .await?;
    Ok(r.rows_affected() as i64)
}

pub async fn fail_unfinished_streams(pool: &UserDb, ended_at: &str) -> Result<i64, AppError> {
    let result = sqlx::query(
        "UPDATE usage_records SET ended_at=?2,duration_ms=CAST((julianday(?3) - julianday(started_at))*86400000 AS INTEGER),success=0,status_code=504,error_code='stream_interrupted',error_message='流式请求因网关重启中断' WHERE user_id=?1 AND stream=1 AND ended_at IS NULL",
    ).bind(pool.user_id())
    .bind(ended_at)
    .bind(ended_at)
    .execute(pool.raw())
    .await?;
    Ok(result.rows_affected() as i64)
}

#[cfg(test)]
mod throughput_tests {
    use super::*;

    async fn pool() -> UserDb {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        let pool = UserDb::new(pool, "local");
        sqlx::migrate!("./migrations")
            .run(pool.raw())
            .await
            .unwrap();
        for id in ["a", "b"] {
            sqlx::query("INSERT INTO accounts(user_id,id,name,type,base_url,api_key_encrypted,status,priority,multiplier,total_requests,total_tokens,created_at,updated_at) VALUES('local',?,?,'openai','https://example.com','','active',5,0.1,0,0,'2026-11-02','2026-11-02')")
                .bind(id).bind(id).execute(pool.raw()).await.unwrap();
        }
        pool
    }

    async fn insert(
        pool: &UserDb,
        id: &str,
        output: Option<i64>,
        duration: Option<i64>,
        first: Option<i64>,
        stream: bool,
        success: bool,
        ended: Option<&str>,
    ) {
        sqlx::query("INSERT INTO usage_records(user_id,id,trace_id,account_id,started_at,ended_at,stream,success,attempts,output_tokens,duration_ms,first_token_ms) VALUES('local',?,?,'a','2026-11-02T01:00:00Z',?,?,?,1,?,?,?)")
            .bind(id).bind(id).bind(ended).bind(stream).bind(success)
            .bind(output).bind(duration).bind(first).execute(pool.raw()).await.unwrap();
    }

    #[tokio::test]
    async fn averages_individual_rates_and_matches_usage_fallbacks() {
        let pool = pool().await;
        let ended = Some("2026-11-02T02:00:00Z");
        // 50 与 10 的算术平均是 30，而非合并 token/耗时后的 16.7。
        insert(
            &pool,
            "1",
            Some(100),
            Some(4000),
            Some(2000),
            true,
            true,
            ended,
        )
        .await;
        insert(
            &pool,
            "2",
            Some(100),
            Some(11000),
            Some(1000),
            true,
            true,
            ended,
        )
        .await;
        let ids = vec!["a".into(), "b".into()];
        let rows = recent_average_throughputs(&pool, &ids, 20).await.unwrap();
        assert_eq!(rows.iter().find(|r| r.0 == "a").unwrap().1, Some(30.0));
        assert_eq!(rows.iter().find(|r| r.0 == "b").unwrap().1, None);
        // 非流式、缺首字、净耗时为零时均按使用记录的总耗时回退规则。
        insert(
            &pool,
            "3",
            Some(200),
            Some(4000),
            Some(2000),
            false,
            true,
            ended,
        )
        .await;
        insert(&pool, "4", Some(100), Some(2000), None, true, true, ended).await;
        insert(
            &pool,
            "5",
            Some(100),
            Some(2000),
            Some(2000),
            true,
            true,
            ended,
        )
        .await;
        let rows = recent_average_throughputs(&pool, &ids, 20).await.unwrap();
        assert_eq!(rows.iter().find(|r| r.0 == "a").unwrap().1, Some(42.0));
        assert!(recent_average_throughputs(&pool, &[], 20)
            .await
            .unwrap()
            .is_empty());
    }

    #[tokio::test]
    async fn takes_twenty_valid_samples_with_stable_order_and_excludes_invalid_rows() {
        let pool = pool().await;
        let ended = Some("2026-11-02T02:00:00Z");
        for n in 1..=21 {
            insert(
                &pool,
                &format!("{n:03}"),
                Some(if n == 1 { 1000 } else { 100 }),
                Some(3000),
                Some(1000),
                true,
                true,
                ended,
            )
            .await;
        }
        insert(
            &pool,
            "z-failed",
            Some(999),
            Some(3000),
            Some(1000),
            true,
            false,
            ended,
        )
        .await;
        insert(
            &pool,
            "z-pending",
            Some(999),
            Some(3000),
            Some(1000),
            true,
            true,
            None,
        )
        .await;
        insert(
            &pool,
            "z-empty",
            Some(999),
            Some(3000),
            Some(1000),
            true,
            true,
            Some(""),
        )
        .await;
        insert(
            &pool,
            "z-zero-output",
            Some(0),
            Some(3000),
            Some(1000),
            true,
            true,
            ended,
        )
        .await;
        insert(
            &pool,
            "z-no-usage",
            None,
            Some(3000),
            Some(1000),
            true,
            true,
            ended,
        )
        .await;
        insert(
            &pool,
            "z-zero-duration",
            Some(999),
            Some(0),
            None,
            true,
            true,
            ended,
        )
        .await;
        insert(
            &pool,
            "z-no-duration",
            Some(999),
            None,
            None,
            true,
            true,
            ended,
        )
        .await;
        let rows = recent_average_throughputs(&pool, &["a".into()], 20)
            .await
            .unwrap();
        assert_eq!(rows, vec![("a".into(), Some(50.0))]);
    }
}
