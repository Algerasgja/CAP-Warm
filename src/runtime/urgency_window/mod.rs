use crate::core::metrics_store::MetricsStore;
use crate::core::prefix_model::make_prefix_signature;
use crate::types::{
    ColdBufferMode, FuncId, PetRequest, Prefix, PrefixConfig, PrewarmPlanTable, UrgencyConfig,
};

pub fn select_prewarm_set(
    pet: &PetRequest,
    predicted_funcs: &[FuncId],
    metrics: &MetricsStore,
    prefix_config: PrefixConfig,
    prewarm_table: &PrewarmPlanTable,
    config: UrgencyConfig,
) -> Vec<FuncId> {
    let mut selected: Vec<FuncId> = Vec::new();
    let mut t: i64 = pet.timestamp as i64;

    let mut current_prefix = Prefix {
        funcs: pet.prefix.funcs.clone(),
    };
    let mut prev_pred: Option<FuncId> = None;

    for (i, next) in predicted_funcs.iter().cloned().enumerate() {
        if selected.len() >= config.max_window_len {
            break;
        }

        if let Some(prev) = prev_pred.as_ref() {
            t += avg_exec(metrics, prev, config) as i64;
        }

        let sig = make_prefix_signature(&pet.workflow_id, &current_prefix, prefix_config);
        t += avg_trans(metrics, &sig, &next, config) as i64;

        let arrival = t;
        let cold = avg_cold(metrics, &next, config) as i64;
        let margin = arrival - cold;

        let delta = compute_delta_buffer(
            metrics,
            prewarm_table.is_prewarmed(&pet.request_id, &next),
            &next,
            predicted_funcs.get(i + 1),
            config,
        ) as i64;

        if margin < delta {
            selected.push(next.clone());
            current_prefix.funcs.push(next.clone());
            prev_pred = Some(next);
        } else {
            break;
        }
    }

    selected
}

fn avg_exec(metrics: &MetricsStore, func: &FuncId, config: UrgencyConfig) -> u64 {
    metrics
        .get_exec_stats(func)
        .map(|s| s.avg_exec)
        .unwrap_or(config.default_exec)
}

fn avg_cold(metrics: &MetricsStore, func: &FuncId, config: UrgencyConfig) -> u64 {
    metrics
        .get_exec_stats(func)
        .map(|s| s.avg_cold)
        .unwrap_or(config.default_cold)
}

fn avg_trans(
    metrics: &MetricsStore,
    prefix_signature: &crate::types::PrefixSignature,
    next: &FuncId,
    config: UrgencyConfig,
) -> u64 {
    metrics
        .get_transition_dist(prefix_signature)
        .into_iter()
        .find(|(f, _)| f == next)
        .map(|(_, stats)| stats.avg_latency)
        .unwrap_or(config.default_trans)
}

fn compute_delta_buffer(
    metrics: &MetricsStore,
    is_hot: bool,
    func: &FuncId,
    next_func: Option<&FuncId>,
    config: UrgencyConfig,
) -> u64 {
    if is_hot {
        return avg_exec(metrics, func, config);
    }

    let cold = avg_cold(metrics, func, config);
    match config.cold_buffer_mode {
        ColdBufferMode::ExecPlusCold => avg_exec(metrics, func, config).saturating_add(cold),
        ColdBufferMode::NextExecPlusCold => {
            let next_exec = next_func
                .map(|f| avg_exec(metrics, f, config))
                .unwrap_or_else(|| avg_exec(metrics, func, config));
            next_exec.saturating_add(cold)
        }
    }
}

