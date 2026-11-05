// Monitor request orchestration lives in background::monitor_task.
use crate::database::UserDb;
use crate::{dao::usage_dao, error::AppError};
use std::collections::HashMap;

pub async fn recent_average_throughputs(
    pool: &UserDb,
    account_ids: &[String],
) -> Result<HashMap<String, f64>, AppError> {
    Ok(usage_dao::recent_average_throughputs(pool, account_ids, 20)
        .await?
        .into_iter()
        .filter_map(|(id, average)| average.filter(|value| value.is_finite()).map(|v| (id, v)))
        .collect())
}
