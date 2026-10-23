use crate::error::AppError;
use sqlx::SqlitePool;

pub type TokenTotals = (i64, i64, i64, i64);

pub async fn first_usage_at(pool: &SqlitePool) -> Result<Option<String>, AppError> {
    Ok(
        sqlx::query_scalar("SELECT MIN(started_at) FROM usage_records")
            .fetch_one(pool)
            .await?,
    )
}

pub async fn dates(pool: &SqlitePool) -> Result<Vec<String>, AppError> {
    Ok(
        sqlx::query_scalar("SELECT stat_date FROM daily_usage_statistics ORDER BY stat_date")
            .fetch_all(pool)
            .await?,
    )
}

/// A single write statement is atomic across multiple app instances. Never overwrite
/// an archived row using raw data that the user may already have cleaned up.
pub async fn archive_day(
    pool: &SqlitePool,
    date: &str,
    start: &str,
    end: &str,
    now: &str,
) -> Result<bool, AppError> {
    let result = sqlx::query(r#"
        INSERT INTO daily_usage_statistics(stat_date,started_at,ended_at,input_tokens,output_tokens,cached_tokens,total_tokens,updated_at)
        SELECT ?,?,?,COALESCE(SUM(input_tokens),0),COALESCE(SUM(output_tokens),0),COALESCE(SUM(cached_tokens),0),COALESCE(SUM(total_tokens),0),?
        FROM usage_records WHERE started_at>=? AND started_at<?
        HAVING NOT EXISTS (SELECT 1 FROM usage_records WHERE started_at>=? AND started_at<? AND stream=1 AND ended_at IS NULL)
           AND NOT EXISTS (SELECT 1 FROM daily_usage_statistics WHERE started_at < ? AND ended_at > ?)
        ON CONFLICT(stat_date) DO NOTHING
    "#).bind(date).bind(start).bind(end).bind(now).bind(start).bind(end).bind(start).bind(end).bind(end).bind(start)
        .execute(pool).await?;
    Ok(result.rows_affected() == 1)
}

/// Snapshot rows and uncovered raw records are read within one SQLite statement.
pub async fn total(pool: &SqlitePool) -> Result<TokenTotals, AppError> {
    Ok(sqlx::query_as(r#"
        WITH ordered AS (
            SELECT ended_at AS range_start,
                   LEAD(started_at, 1, '9999-12-31T23:59:59Z') OVER (ORDER BY started_at) AS range_end
            FROM daily_usage_statistics
        ), uncovered AS (
            SELECT '' AS range_start, COALESCE(MIN(started_at), '9999-12-31T23:59:59Z') AS range_end
            FROM daily_usage_statistics
            UNION ALL
            SELECT range_start, range_end FROM ordered WHERE range_start < range_end
        )
        SELECT COALESCE(SUM(input_tokens),0),COALESCE(SUM(output_tokens),0),COALESCE(SUM(cached_tokens),0),COALESCE(SUM(total_tokens),0)
        FROM (
            SELECT input_tokens,output_tokens,cached_tokens,total_tokens FROM daily_usage_statistics
            UNION ALL
            SELECT u.input_tokens,u.output_tokens,u.cached_tokens,u.total_tokens
            FROM uncovered r JOIN usage_records u ON u.started_at>=r.range_start AND u.started_at<r.range_end
        )
    "#).fetch_one(pool).await?)
}

pub async fn day(
    pool: &SqlitePool,
    date: &str,
    start: &str,
    end: &str,
) -> Result<TokenTotals, AppError> {
    Ok(sqlx::query_as(r#"
        SELECT COALESCE(SUM(input_tokens),0),COALESCE(SUM(output_tokens),0),COALESCE(SUM(cached_tokens),0),COALESCE(SUM(total_tokens),0)
        FROM (
            SELECT input_tokens,output_tokens,cached_tokens,total_tokens FROM daily_usage_statistics WHERE stat_date=?
            UNION ALL
            SELECT input_tokens,output_tokens,cached_tokens,total_tokens FROM usage_records
            WHERE started_at>=? AND started_at<? AND NOT EXISTS(SELECT 1 FROM daily_usage_statistics WHERE stat_date=?)
        )
    "#).bind(date).bind(start).bind(end).bind(date).fetch_one(pool).await?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn pool() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        // Simulate an existing installation before upgrading, preserving its raw rows.
        sqlx::raw_sql(include_str!("../../migrations/0001_baseline.sql"))
            .execute(&pool)
            .await
            .unwrap();
        insert(
            &pool,
            "old",
            "2026-10-20T12:00:00Z",
            Some("2026-10-20T12:01:00Z"),
            100,
        )
        .await;
        for sql in [
            include_str!("../../migrations/0002_placeholder.sql"),
            include_str!("../../migrations/0003_models_one_default.sql"),
            include_str!("../../migrations/0004_accounts_monitor_average_duration.sql"),
            include_str!("../../migrations/0005_remove_accounts_multiplier_constraint.sql"),
            include_str!("../../migrations/0006_models_provider.sql"),
            include_str!("../../migrations/0007_multiplier_monitor.sql"),
            include_str!("../../migrations/0008_multiplier_monitor_divisor.sql"),
            include_str!("../../migrations/0009_multiplier_monitor_refresh_token.sql"),
            include_str!("../../migrations/0010_accounts_timed_disable.sql"),
            include_str!("../../migrations/0011_daily_usage_statistics.sql"),
        ] {
            sqlx::raw_sql(sql).execute(&pool).await.unwrap();
        }
        pool
    }

    async fn insert(pool: &SqlitePool, id: &str, start: &str, end: Option<&str>, input: i64) {
        sqlx::query("INSERT INTO usage_records(id,trace_id,started_at,ended_at,stream,success,attempts,input_tokens,output_tokens,cached_tokens,total_tokens) VALUES(?,?,?, ?,1,1,1,?,10,50,?)")
            .bind(id).bind(id).bind(start).bind(end).bind(input).bind(input+10).execute(pool).await.unwrap();
    }

    #[tokio::test]
    async fn upgrades_preserve_data_and_archiving_is_idempotent_and_survives_cleanup() {
        let pool = pool().await;
        let start = "2026-10-20T00:00:00Z";
        let end = "2026-10-21T00:00:00Z";
        assert_eq!(total(&pool).await.unwrap(), (100, 10, 50, 110));
        assert!(archive_day(&pool, "2026-10-20", start, end, end)
            .await
            .unwrap());
        assert!(!archive_day(&pool, "2026-10-20", start, end, end)
            .await
            .unwrap());
        insert(&pool, "late", "2026-10-20T13:00:00Z", Some(end), 200).await;
        sqlx::query("UPDATE usage_records SET input_tokens=300,total_tokens=310 WHERE id='late'")
            .execute(&pool)
            .await
            .unwrap();
        insert(&pool, "today", end, Some(end), 400).await;
        assert_eq!(total(&pool).await.unwrap(), (800, 30, 150, 830));
        assert_eq!(
            day(&pool, "2026-10-20", start, end).await.unwrap(),
            (400, 20, 100, 420)
        );
        assert_eq!(crate::dao::usage_dao::cleanup(&pool, end).await.unwrap(), 2);
        assert_eq!(total(&pool).await.unwrap(), (800, 30, 150, 830));
        assert!(!archive_day(&pool, "2026-10-20", start, end, end)
            .await
            .unwrap());
        assert_eq!(
            day(&pool, "2026-10-20", start, end).await.unwrap(),
            (400, 20, 100, 420)
        );
    }

    #[tokio::test]
    async fn pending_streams_defer_day_and_missing_zero_days_are_backfilled() {
        let pool = pool().await;
        let now = chrono::DateTime::parse_from_rfc3339("2026-10-24T12:00:00+08:00")
            .unwrap()
            .with_timezone(&chrono::Local);
        insert(&pool, "pending", "2026-10-21T12:00:00Z", None, 0).await;
        crate::service::daily_statistics_service::catch_up(&pool, now)
            .await
            .unwrap();
        assert!(!dates(&pool).await.unwrap().contains(&"2026-10-21".into()));
        sqlx::query("UPDATE usage_records SET ended_at='2026-10-22T01:00:00Z',input_tokens=500,total_tokens=510 WHERE id='pending'").execute(&pool).await.unwrap();
        crate::service::daily_statistics_service::catch_up(&pool, now)
            .await
            .unwrap();
        let dates = dates(&pool).await.unwrap();
        assert!(dates.contains(&"2026-10-21".into()));
        assert!(dates.contains(&"2026-10-23".into()));
        assert_eq!(total(&pool).await.unwrap(), (600, 20, 100, 620));
    }

    #[tokio::test]
    async fn concurrent_archives_and_unarchived_fallback_do_not_double_count() {
        let pool = pool().await;
        let start = "2026-10-20T00:00:00Z";
        let end = "2026-10-21T00:00:00Z";
        assert_eq!(
            day(&pool, "2026-10-20", start, end).await.unwrap(),
            (100, 10, 50, 110)
        );
        let (first, second) = tokio::join!(
            archive_day(&pool, "2026-10-20", start, end, end),
            archive_day(&pool, "2026-10-20", start, end, end)
        );
        assert_ne!(first.unwrap(), second.unwrap());
        assert_eq!(total(&pool).await.unwrap(), (100, 10, 50, 110));
        insert(&pool, "unarchived", end, Some(end), 200).await;
        assert_eq!(
            crate::dao::usage_dao::cleanup(&pool, "2026-10-25T00:00:00Z")
                .await
                .unwrap(),
            1
        );
        assert_eq!(total(&pool).await.unwrap(), (300, 20, 100, 320));
    }

    #[tokio::test]
    async fn upgrades_legacy_file_and_reopens_with_sqlx_metadata() {
        let folder =
            std::env::temp_dir().join(format!("aimux-daily-test-{}", uuid::Uuid::new_v4()));
        tokio::fs::create_dir(&folder).await.unwrap();
        let path = folder.join("legacy-copy.db");
        let options = sqlx::sqlite::SqliteConnectOptions::new()
            .filename(&path)
            .create_if_missing(true);
        let legacy = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .unwrap();
        sqlx::raw_sql(include_str!("../../migrations/0001_baseline.sql"))
            .execute(&legacy)
            .await
            .unwrap();
        insert(
            &legacy,
            "legacy",
            "2026-10-20T12:00:00Z",
            Some("2026-10-20T12:01:00Z"),
            100,
        )
        .await;
        legacy.close().await;
        let upgraded = crate::database::connect(&path).await.unwrap();
        assert_eq!(total(&upgraded).await.unwrap(), (100, 10, 50, 110));
        assert!(archive_day(
            &upgraded,
            "2026-10-20",
            "2026-10-20T00:00:00Z",
            "2026-10-21T00:00:00Z",
            "2026-10-21T08:00:00Z"
        )
        .await
        .unwrap());
        upgraded.close().await;
        let reopened = crate::database::connect(&path).await.unwrap();
        assert_eq!(total(&reopened).await.unwrap(), (100, 10, 50, 110));
        reopened.close().await;
        tokio::fs::remove_dir_all(&folder).await.unwrap();
    }
}
