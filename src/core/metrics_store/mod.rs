use std::collections::HashMap;

use crate::core::prefix_model::make_prefix_signature;
use crate::types::{
    Duration, EmaParams, ExecStats, FuncId, OpenWhiskActivationObservation, PrefixConfig,
    PrefixSignature, TransitionStats,
};

#[derive(Clone, Debug, Default)]
pub struct MetricsStore {
    node_stats: HashMap<FuncId, ExecStats>,
    transition_stats: HashMap<PrefixSignature, HashMap<FuncId, TransitionStats>>,
}

impl MetricsStore {
    pub fn all_prefixes(&self) -> Vec<PrefixSignature> {
        self.transition_stats.keys().cloned().collect()
    }

    pub fn decay(&mut self, gamma: f64) {
        for inner_map in self.transition_stats.values_mut() {
            for stats in inner_map.values_mut() {
                stats.weighted_count *= gamma;
            }
        }
    }

    pub fn ingest_openwhisk_activation(
        &mut self,
        obs: &OpenWhiskActivationObservation,
        ema: EmaParams,
        prefix_config: PrefixConfig,
    ) {
        self.update_exec(
            &obs.func,
            obs.exec_duration,
            obs.cold_start_duration.is_some(),
            obs.cold_start_duration,
            ema,
        );

        let sig = make_prefix_signature(&obs.workflow_id, &obs.prefix, prefix_config);
        self.update_transition(&sig, &obs.func, obs.trans_latency, obs.weight, ema);
    }

    pub fn update_exec(
        &mut self,
        func_id: &FuncId,
        exec_duration: Duration,
        is_cold: bool,
        cold_duration_opt: Option<Duration>,
        ema: EmaParams,
    ) {
        let entry = self.node_stats.entry(func_id.clone()).or_default();
        entry.avg_exec = ema_duration(entry.avg_exec, exec_duration, ema.alpha_exec);

        if is_cold {
            if let Some(cold_duration) = cold_duration_opt {
                entry.avg_cold = ema_duration(entry.avg_cold, cold_duration, ema.alpha_cold);
            }
        }
    }

    pub fn update_transition(
        &mut self,
        prefix_signature: &PrefixSignature,
        next_func: &FuncId,
        trans_latency_opt: Option<Duration>,
        weight: f64,
        ema: EmaParams,
    ) {
        let by_next = self
            .transition_stats
            .entry(prefix_signature.clone())
            .or_default();
        let entry = by_next.entry(next_func.clone()).or_default();

        entry.weighted_count = ema_count(entry.weighted_count, weight, ema.alpha_count);
        if let Some(trans_latency) = trans_latency_opt {
            entry.avg_latency = ema_duration(entry.avg_latency, trans_latency, ema.alpha_trans);
        }
    }

    pub fn get_exec_stats(&self, func_id: &FuncId) -> Option<&ExecStats> {
        self.node_stats.get(func_id)
    }

    pub fn get_transition_dist(
        &self,
        prefix_signature: &PrefixSignature,
    ) -> Vec<(FuncId, TransitionStats)> {
        self.transition_stats
            .get(prefix_signature)
            .map(|m| m.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
            .unwrap_or_default()
    }

    pub fn get_next_prob(&self, prefix_signature: &PrefixSignature) -> Vec<(FuncId, f64)> {
        let dist = match self.transition_stats.get(prefix_signature) {
            Some(v) => v,
            None => return Vec::new(),
        };

        let sum: f64 = dist.values().map(|s| s.weighted_count).sum();
        if sum <= 0.0 {
            return Vec::new();
        }

        dist.iter()
            .map(|(k, v)| (k.clone(), v.weighted_count / sum))
            .collect()
    }
}

fn ema_duration(old: Duration, new: Duration, alpha: f64) -> Duration {
    if old == 0 {
        return new;
    }
    ((old as f64) * (1.0 - alpha) + (new as f64) * alpha) as Duration
}

fn ema_count(old: f64, new: f64, alpha: f64) -> f64 {
    old * (1.0 - alpha) + new * alpha
}
