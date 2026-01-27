use std::collections::{HashMap, HashSet};

use crate::core::metrics_store::MetricsStore;
use crate::core::prefix_model::{make_prefix_signature, parse_prefix_signature};
use crate::types::{FuncId, PredictedPath, Prefix, PrefixConfig, PrefixSignature, WorkflowId};

#[derive(Clone, Debug, Default)]
pub struct Dpt {
    version: u64,
    table: HashMap<PrefixSignature, PredictedPath>,
}

impl Dpt {
    pub fn version(&self) -> u64 {
        self.version
    }

    pub fn get(&self, prefix_signature: &PrefixSignature) -> Option<&PredictedPath> {
        self.table.get(prefix_signature)
    }

    pub fn replace(&mut self, version: u64, table: HashMap<PrefixSignature, PredictedPath>) {
        self.version = version;
        self.table = table;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BuildConfig {
    pub max_path_len: usize,
}

impl Default for BuildConfig {
    fn default() -> Self {
        Self { max_path_len: 64 }
    }
}

pub fn build_dpt(
    metrics: &MetricsStore,
    prefix_config: PrefixConfig,
    build_config: BuildConfig,
    version: u64,
) -> Dpt {
    let mut table: HashMap<PrefixSignature, PredictedPath> = HashMap::new();

    for prefix_sig in metrics.all_prefixes() {
        let Some((workflow_id, funcs)) = parse_prefix_signature(&prefix_sig) else {
            continue;
        };

        let prefix = Prefix { funcs };
        let path = build_mlp(metrics, &workflow_id, &prefix, prefix_config, build_config);
        table.insert(prefix_sig, path);
    }

    Dpt { version, table }
}

fn build_mlp(
    metrics: &MetricsStore,
    workflow_id: &WorkflowId,
    prefix: &Prefix,
    prefix_config: PrefixConfig,
    build_config: BuildConfig,
) -> PredictedPath {
    let mut seen: HashSet<FuncId> = HashSet::new();
    let mut out: Vec<FuncId> = Vec::new();

    let mut current_prefix = prefix.clone();
    for _ in 0..build_config.max_path_len {
        let sig = make_prefix_signature(workflow_id, &current_prefix, prefix_config);
        let mut dist = metrics.get_next_prob(&sig);
        if dist.is_empty() {
            break;
        }
        dist.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        let next = dist[0].0.clone();
        if !seen.insert(next.clone()) {
            break;
        }
        out.push(next.clone());
        current_prefix.funcs.push(next);
    }

    PredictedPath { funcs: out }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{EmaParams, PrefixConfig};

    #[test]
    fn builds_mlp_from_weighted_counts() {
        let mut metrics = MetricsStore::default();
        let ema = EmaParams {
            alpha_exec: 0.2,
            alpha_cold: 0.2,
            alpha_trans: 0.2,
            alpha_count: 0.0,
        };

        let w = WorkflowId::from("w");
        let prefix = Prefix::new(vec![FuncId::from("A")]);
        let sig = make_prefix_signature(&w, &prefix, PrefixConfig { lmax: 8 });

        metrics.update_transition(&sig, &FuncId::from("B"), None, 1.0, ema);
        metrics.update_transition(&sig, &FuncId::from("B"), None, 1.0, ema);
        metrics.update_transition(&sig, &FuncId::from("C"), None, 1.0, ema);

        let dpt = build_dpt(&metrics, PrefixConfig { lmax: 8 }, BuildConfig { max_path_len: 8 }, 1);
        let path = dpt.get(&sig).unwrap();
        assert_eq!(path.funcs.first().unwrap(), &FuncId::from("B"));
    }
}
