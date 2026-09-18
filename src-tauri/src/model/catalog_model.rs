use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct CatalogModel {
    pub id: String,
    pub name: String,
    pub r#type: String,
    pub provider: String,
    pub is_default: i64,
    pub created_at: String,
    pub updated_at: String,
}
