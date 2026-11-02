use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct MultiplierMonitorUpdate {
    pub platform_type: Option<String>,
    pub new_api_group: Option<String>,
    pub sync_upstream_multiplier: Option<bool>,
    pub multiplier_divisor: Option<i64>,
    pub enabled: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MultiplierMonitorConfigView {
    pub id: String,
    pub account_id: String,
    pub account_name: String,
    #[serde(rename = "account_type")]
    pub account_type: String,
    pub account_status: String,
    pub account_multiplier: f64,
    pub configured: bool,
    pub platform_type: String,
    pub new_api_group: Option<String>,
    pub sync_upstream_multiplier: bool,
    pub multiplier_divisor: i64,
    pub enabled: bool,
    pub last_started_at: Option<String>,
    pub last_finished_at: Option<String>,
    pub last_success_at: Option<String>,
    pub last_error: Option<String>,
    pub last_remote_multiplier: Option<f64>,
    pub last_result: Option<String>,
    pub running: bool,
    pub last_run: Option<MultiplierMonitorRunSummary>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct MultiplierMonitorRunSummary {
    pub run_id: String,
    pub started_at: String,
    pub finished_at: String,
    pub total: i64,
    pub updated: i64,
    pub unchanged: i64,
    pub issues: i64,
    pub failed: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct MultiplierMonitorCheckResult {
    pub run_id: String,
    pub total: i64,
    pub updated: i64,
    pub unchanged: i64,
    pub issues: i64,
    pub failed: i64,
}

#[derive(Debug, Deserialize, Default)]
pub struct MultiplierMonitorLogQuery {
    pub offset: Option<i64>,
    pub limit: Option<i64>,
    pub result: Option<String>,
}
