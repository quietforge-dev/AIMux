use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Account {
    pub id: String,
    pub name: String,
    pub r#type: String,
    pub base_url: String,
    pub api_key_encrypted: String,
    pub status: String,
    pub disabled_until: Option<i64>,
    pub priority: i64,
    pub multiplier: f64,
    pub supported_models: Option<String>,
    pub tags: Option<String>,
    pub notes: Option<String>,
    pub last_error_code: Option<String>,
    pub last_error_message: Option<String>,
    pub last_successful_test_model: Option<String>,
    pub last_used_at: Option<String>,
    pub total_requests: i64,
    pub total_tokens: i64,
    pub monitor_average_duration_ms: Option<i64>,
    pub created_at: String,
    pub updated_at: String,
    pub test_default_model: Option<String>,
    pub model_mappings: Option<String>,
}
