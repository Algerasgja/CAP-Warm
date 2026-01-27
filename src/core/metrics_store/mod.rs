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
            .map(|(func, stats)| (func.clone(), stats.weighted_count / sum))
            .collect()
    }

    pub fn all_prefixes(&self) -> Vec<PrefixSignature> {
        self.transition_stats.keys().cloned().collect()
    }
}

fn ema_duration(old: Duration, observed: Duration, alpha: f64) -> Duration {
    if old == 0 {
        return observed;
    }
    let alpha = alpha.clamp(0.0, 1.0);
    let new_val = (alpha * observed as f64) + ((1.0 - alpha) * old as f64);
    new_val.round().max(0.0) as Duration
}

fn ema_count(old: f64, weight: f64, alpha: f64) -> f64 {
    let alpha = alpha.clamp(0.0, 1.0);
    let weight = if weight.is_finite() && weight > 0.0 { weight } else { 1.0 };
    (1.0 - alpha) * old + weight
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::PrefixSignature;
    use crate::types::{ColdBufferMode, PetRequest, PrewarmPlanTable, RequestId, UrgencyConfig};
    use crate::{core::dpt::Dpt, runtime::pet_trigger::PetHandler};
    use crate::{runtime::pet_trigger::PrewarmExecutor, types::Prefix};

    #[test]
    fn updates_exec_stats_with_ema() {
        let mut store = MetricsStore::default();
        let ema = EmaParams {
            alpha_exec: 0.5,
            alpha_cold: 0.5,
            alpha_trans: 0.5,
            alpha_count: 0.5,
        };
        let f = FuncId::from("A");

        store.update_exec(&f, 100, false, None, ema);
        assert_eq!(store.get_exec_stats(&f).unwrap().avg_exec, 100);

        store.update_exec(&f, 200, false, None, ema);
        assert_eq!(store.get_exec_stats(&f).unwrap().avg_exec, 150);
    }

    #[test]
    fn updates_transition_count_and_latency() {
        let mut store = MetricsStore::default();
        let ema = EmaParams::default();
        let sig = PrefixSignature::from("w\0A");
        let next = FuncId::from("B");

        store.update_transition(&sig, &next, Some(10), 1.0, ema);
        let dist = store.get_transition_dist(&sig);
        assert_eq!(dist.len(), 1);
        assert_eq!(dist[0].0, next);
        assert!(dist[0].1.weighted_count > 0.0);
        assert_eq!(dist[0].1.avg_latency, 10);
    }

    #[test]
    fn ingests_openwhisk_activation_into_metrics_store() {
        let mut store = MetricsStore::default();
        let ema = EmaParams {
            alpha_exec: 1.0,
            alpha_cold: 1.0,
            alpha_trans: 1.0,
            alpha_count: 0.0,
        };

        let obs = OpenWhiskActivationObservation {
            workflow_id: crate::types::WorkflowId::from("w"),
            prefix: crate::types::Prefix::new(vec![FuncId::from("A")]),
            func: FuncId::from("B"),
            exec_duration: 120,
            cold_start_duration: Some(30),
            trans_latency: Some(7),
            weight: 2.0,
            timestamp: 1,
        };

        store.ingest_openwhisk_activation(&obs, ema, PrefixConfig { lmax: 8 });

        let exec = store.get_exec_stats(&FuncId::from("B")).unwrap();
        assert_eq!(exec.avg_exec, 120);
        assert_eq!(exec.avg_cold, 30);

        let sig = PrefixSignature::from("w\0A");
        let dist = store.get_transition_dist(&sig);
        assert_eq!(dist.len(), 1);
        assert_eq!(dist[0].0, FuncId::from("B"));
        assert_eq!(dist[0].1.weighted_count, 2.0);
        assert_eq!(dist[0].1.avg_latency, 7);
    }

    #[derive(Default)]
    struct RecordingExecutor {
        calls: std::cell::RefCell<Vec<(RequestId, Vec<FuncId>)>>,
    }

    impl PrewarmExecutor for RecordingExecutor {
        fn prewarm(&self, request_id: &RequestId, funcs: &[FuncId]) {
            self.calls
                .borrow_mut()
                .push((request_id.clone(), funcs.to_vec()));
        }
    }

    #[test]
    fn handles_pet_and_updates_prewarm_table_per_request() {
        let mut metrics = MetricsStore::default();
        let ema = EmaParams {
            alpha_exec: 1.0,
            alpha_cold: 1.0,
            alpha_trans: 1.0,
            alpha_count: 0.0,
        };

        let w = crate::types::WorkflowId::from("w");
        let prefix_a = Prefix::new(vec![FuncId::from("A")]);
        let sig_a = crate::core::prefix_model::make_prefix_signature(&w, &prefix_a, PrefixConfig { lmax: 8 });
        metrics.update_transition(&sig_a, &FuncId::from("B"), Some(1), 1.0, ema);

        let prefix_ab = Prefix::new(vec![FuncId::from("A"), FuncId::from("B")]);
        let sig_ab = crate::core::prefix_model::make_prefix_signature(&w, &prefix_ab, PrefixConfig { lmax: 8 });
        metrics.update_transition(&sig_ab, &FuncId::from("C"), Some(1), 1.0, ema);

        let prefix_abc = Prefix::new(vec![FuncId::from("A"), FuncId::from("B"), FuncId::from("C")]);
        let sig_abc = crate::core::prefix_model::make_prefix_signature(&w, &prefix_abc, PrefixConfig { lmax: 8 });
        metrics.update_transition(&sig_abc, &FuncId::from("D"), Some(1), 1.0, ema);

        metrics.update_exec(&FuncId::from("B"), 5, true, Some(10), ema);
        metrics.update_exec(&FuncId::from("C"), 5, true, Some(2), ema);
        metrics.update_exec(&FuncId::from("D"), 5, false, None, ema);

        let mut dpt = Dpt::default();
        dpt.replace(
            1,
            std::collections::HashMap::from([(
                sig_a.clone(),
                crate::types::PredictedPath {
                    funcs: vec![FuncId::from("B"), FuncId::from("C"), FuncId::from("D")],
                },
            )]),
        );

        let mut prewarm_table = PrewarmPlanTable::default();
        let req = RequestId::from("r1");
        prewarm_table.set_window_result(req.clone(), vec![FuncId::from("B")]);

        let executor = RecordingExecutor::default();
        let mut handler = PetHandler {
            dpt: &dpt,
            metrics: &metrics,
            prewarm_table: &mut prewarm_table,
            executor: &executor,
            prefix_config: PrefixConfig { lmax: 8 },
            urgency_config: UrgencyConfig {
                max_window_len: 16,
                default_exec: 1,
                default_cold: 0,
                default_trans: 0,
                cold_buffer_mode: ColdBufferMode::ExecPlusCold,
            },
        };

        let pet = PetRequest {
            request_id: req.clone(),
            workflow_id: w,
            prefix: prefix_a,
            curr_func: FuncId::from("A"),
            timestamp: 0,
        };

        let plan = handler.handle_pet(pet);
        assert_eq!(plan.actions.len(), 1);

        let calls = executor.calls.borrow();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, req);
        assert_eq!(calls[0].1, vec![FuncId::from("C")]);

        let updated = handler.prewarm_table.get_request_set(&RequestId::from("r1"));
        assert!(updated.contains(&FuncId::from("B")));
        assert!(updated.contains(&FuncId::from("C")));
        assert!(!updated.contains(&FuncId::from("D")));
    }
}
