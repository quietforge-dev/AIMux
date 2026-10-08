use crate::{error::AppError, model::monitor_record::MonitorRecord};
use sqlx::SqlitePool;

pub async fn list_grouped(
    pool: &SqlitePool,
    ids: &[String],
    limit: i64,
) -> Result<Vec<MonitorRecord>, AppError> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let placeholders = std::iter::repeat("?")
        .take(ids.len())
        .collect::<Vec<_>>()
        .join(",");
    let q = format!(
        r#"
            WITH ranked AS (
                SELECT id,account_id,account_name,account_type,model,checked_at,duration_ms,success,status_code,error_code,error_message,
                       ROW_NUMBER() OVER (PARTITION BY account_id ORDER BY checked_at DESC,id DESC) AS row_number
                FROM monitor_records
                WHERE account_id IN ({})
            )
            SELECT id,account_id,account_name,account_type,model,checked_at,duration_ms,success,status_code,error_code,error_message
            FROM ranked
            WHERE row_number <= ?
            ORDER BY account_id,checked_at DESC,id DESC
        "#,
        placeholders
    );
    let mut query = sqlx::query_as::<_, MonitorRecord>(&q);
    for id in ids {
        query = query.bind(id);
    }
    Ok(query.bind(limit.max(1)).fetch_all(pool).await?)
}
pub async fn create_and_refresh_account_average(
    pool: &SqlitePool,
    record: &MonitorRecord,
) -> Result<(), AppError> {
    let mut transaction = pool.begin().await?;
    sqlx::query("INSERT INTO monitor_records(id,account_id,account_name,account_type,model,checked_at,duration_ms,success,status_code,error_code,error_message) VALUES(?,?,?,?,?,?,?,?,?,?,?)")
        .bind(&record.id)
        .bind(&record.account_id)
        .bind(&record.account_name)
        .bind(&record.account_type)
        .bind(&record.model)
        .bind(&record.checked_at)
        .bind(record.duration_ms)
        .bind(record.success)
        .bind(record.status_code)
        .bind(&record.error_code)
        .bind(&record.error_message)
        .execute(&mut *transaction)
        .await?;
    sqlx::query(
        r#"
            UPDATE accounts
            SET monitor_average_duration_ms = (
                SELECT CAST(ROUND(AVG(duration_ms)) AS INTEGER)
                FROM (
                    SELECT duration_ms
                    FROM monitor_records
                    WHERE account_id = ? AND duration_ms IS NOT NULL
                    ORDER BY checked_at DESC, id DESC
                    LIMIT 30
                )
            )
            WHERE id = ?
        "#,
    )
    .bind(&record.account_id)
    .bind(&record.account_id)
    .execute(&mut *transaction)
    .await?;
    transaction.commit().await?;
    Ok(())
}
