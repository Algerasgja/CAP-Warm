use crate::core::metrics_store::MetricsStore;
use crate::core::prefix_model::make_prefix_signature;
use crate::types::{EmaParams, Observation, PrefixConfig};

#[derive(Clone, Debug, Default)]
pub struct MockReplay {
    observations: Vec<Observation>,
}

impl MockReplay {
    pub fn new(observations: Vec<Observation>) -> Self {
        Self { observations }
    }

    pub fn feed_into(&self, metrics: &mut MetricsStore, ema: EmaParams, prefix_config: PrefixConfig) {
        for obs in &self.observations {
            metrics.update_exec(
                &obs.curr_func,
                obs.exec_duration,
                obs.is_cold,
                obs.cold_duration_opt,
                ema,
            );

            let sig = make_prefix_signature(&obs.workflow_id, &obs.prefix, prefix_config);
            metrics.update_transition(&sig, &obs.next_func, obs.trans_latency_opt, 1.0, ema);
        }
    }
}
