use std::sync::Arc;
use tokio::time::{self, Duration};
use tracing::info;

use crate::core::dpt::{build_dpt, BuildConfig};
use crate::runtime::http_server::AppState;

pub struct RebuildScheduler {
    state: Arc<AppState>,
    interval_secs: u64,
}

impl RebuildScheduler {
    pub fn new(state: Arc<AppState>, interval_secs: u64) -> Self {
        Self {
            state,
            interval_secs,
        }
    }

    pub async fn run(self) {
        let mut interval = time::interval(Duration::from_secs(self.interval_secs));
        info!("DPT Rebuild Scheduler started with interval {}s", self.interval_secs);

        loop {
            interval.tick().await;
            self.rebuild_dpt().await;
        }
    }

    async fn rebuild_dpt(&self) {
        info!("Starting DPT rebuild...");
        
        // 1. Get current version
        let current_version = {
            let dpt = self.state.dpt.lock().await;
            dpt.version()
        };
        let new_version = current_version + 1;

        // 2. Clone metrics to release lock quickly
        let metrics_snapshot = {
            let metrics = self.state.metrics_store.lock().await;
            metrics.clone()
        };

        // 3. Build new DPT (cpu intensive, but doesn't block other tasks if run in spawn_blocking or just async here)
        // Since build_dpt is synchronous, it blocks the executor thread. 
        // For heavy DPT, we should wrap in spawn_blocking.
        let state_clone = self.state.clone();
        let new_dpt = tokio::task::spawn_blocking(move || {
            build_dpt(
                &metrics_snapshot, 
                state_clone.prefix_config, 
                BuildConfig::default(), 
                new_version
            )
        }).await.unwrap();

        // 4. Update global state
        {
            let mut dpt_guard = self.state.dpt.lock().await;
            *dpt_guard = new_dpt;
        }

        info!("DPT rebuild completed. Version: {}", new_version);
    }
}
