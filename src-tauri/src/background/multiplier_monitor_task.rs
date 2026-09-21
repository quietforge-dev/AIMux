use std::{sync::Arc, time::Duration};

use tokio::time::{interval, MissedTickBehavior};
use uuid::Uuid;

use crate::{app_state::AppState, service::multiplier_monitor_service};

pub async fn run(state: Arc<AppState>) {
    let worker_id = format!("multiplier-worker-{}", Uuid::new_v4());
    let mut ticker = interval(Duration::from_secs(60));
    ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
    loop {
        ticker.tick().await;
        let settings = state.settings.read().await.clone();
        if let Err(error) =
            multiplier_monitor_service::run_due(&state.pool, &settings, &worker_id).await
        {
            tracing::error!(%error, "倍率监控调度失败");
        }
    }
}
