use std::sync::Arc;

use sqlx::SqlitePool;

use crate::{
    config::{Settings, SharedSettings},
    dao::usage_dao,
    database,
    error::AppError,
    utils::time::utc_now_string,
};

pub struct AppState {
    pub pool: SqlitePool,
    pub settings: SharedSettings,
}

impl AppState {
    pub async fn initialize(settings: Settings) -> Result<Self, AppError> {
        let pool = database::connect(&settings.database_path()).await?;
        crate::service::model_service::seed(&pool).await?;
        let now = utc_now_string();
        let interrupted = usage_dao::fail_unfinished_streams(&pool, &now).await?;
        if interrupted > 0 {
            tracing::warn!(
                count = interrupted,
                "已将上次遗留的未结束流式记录标记为失败"
            );
        }
        Ok(Self {
            pool,
            settings: Arc::new(tokio::sync::RwLock::new(settings)),
        })
    }
}
