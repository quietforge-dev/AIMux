use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct MultiplierMonitorConfig {
    pub id: String,
    pub name: String,
    pub url: String,
    pub token: String,
    pub account_ids: String,
    pub multiplier_divisor: i64,
    pub enabled: bool,
    pub last_started_at: Option<String>,
    pub last_finished_at: Option<String>,
    pub lease_owner: Option<String>,
    pub lease_until: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct MultiplierMonitorLog {
    pub id: String,
    pub run_id: String,
    pub config_id: String,
    pub config_name: String,
    pub account_id: Option<String>,
    pub account_name: Option<String>,
    pub started_at: String,
    pub finished_at: String,
    pub duration_ms: Option<i64>,
    pub result: String,
    pub old_multiplier: Option<f64>,
    pub remote_multiplier: Option<f64>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub http_status: Option<i64>,
}

#[derive(Debug, Clone, FromRow)]
pub struct LatestRunStats {
    pub run_id: String,
    pub started_at: String,
    pub finished_at: String,
    pub total: i64,
    pub updated: i64,
    pub unchanged: i64,
    pub issues: i64,
    pub failed: i64,
}
