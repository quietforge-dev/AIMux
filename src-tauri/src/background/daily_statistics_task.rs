use crate::{app_state::AppState, service::daily_statistics_service};
use std::{sync::Arc, time::Duration};

pub async fn run(state: Arc<AppState>) {
    let mut timer = tokio::time::interval(Duration::from_secs(60));
    timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        timer.tick().await;
        match daily_statistics_service::catch_up(&state.pool, chrono::Local::now()).await {
            Ok(count) if count > 0 => tracing::info!(count, "每日统计历史补齐完成"),
            Ok(_) => {}
            Err(error) => tracing::error!(%error, "每日统计汇总失败，将自动重试"),
        }
    }
}
