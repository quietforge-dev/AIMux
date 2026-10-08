use std::{sync::Arc, time::Duration};

use tokio::time::{interval, MissedTickBehavior};

use crate::{app_state::AppState, dao::account_dao};

pub async fn run(state: Arc<AppState>) {
    let mut ticker = interval(Duration::from_secs(60));
    ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
    loop {
        ticker.tick().await;
        match account_dao::expire_disabled(&state.pool).await {
            Ok(count) if count > 0 => tracing::info!(count, "已自动启用到期账号"),
            Ok(_) => {}
            Err(error) => tracing::error!(%error, "自动恢复到期账号失败"),
        }
    }
}
