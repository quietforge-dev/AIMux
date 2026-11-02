use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct MultiplierMonitorConfig {
    pub id: String,
    pub account_id: String,
    pub platform_type: String,
    pub new_api_group: Option<String>,
    pub sync_upstream_multiplier: bool,
    pub multiplier_divisor: i64,
    pub enabled: bool,
    pub last_started_at: Option<String>,
    pub last_finished_at: Option<String>,
    pub lease_owner: Option<String>,
    pub lease_until: Option<String>,
    pub last_success_at: Option<String>,
    pub last_error: Option<String>,
    pub last_remote_multiplier: Option<f64>,
    pub last_result: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct MultiplierMonitorEntry {
    pub account_id: String,
    pub account_name: String,
    pub account_type: String,
    pub base_url: String,
    pub api_key: String,
    pub account_status: String,
    pub account_multiplier: f64,
    pub config_id: Option<String>,
    pub platform_type: Option<String>,
    pub new_api_group: Option<String>,
    pub sync_upstream_multiplier: Option<bool>,
    pub multiplier_divisor: Option<i64>,
    pub enabled: Option<bool>,
    pub last_started_at: Option<String>,
    pub last_finished_at: Option<String>,
    pub lease_owner: Option<String>,
    pub lease_until: Option<String>,
    pub last_success_at: Option<String>,
    pub last_error: Option<String>,
    pub last_remote_multiplier: Option<f64>,
    pub last_result: Option<String>,
    pub config_created_at: Option<String>,
    pub config_updated_at: Option<String>,
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
