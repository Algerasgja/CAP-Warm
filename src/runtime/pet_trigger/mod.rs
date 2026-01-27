use std::collections::HashSet;

use crate::core::dpt::Dpt;
use crate::core::metrics_store::MetricsStore;
use crate::core::prefix_model::make_prefix_signature;
use crate::runtime::urgency_window::select_prewarm_set;
use crate::types::{
    FuncId, PetRequest, PrefixConfig, PrewarmAction, PrewarmPlan, PrewarmPlanTable, RequestId,
    UrgencyConfig,
};

pub trait PrewarmExecutor {
    fn prewarm(&self, request_id: &RequestId, funcs: &[FuncId]);
}

pub struct PetHandler<'a> {
    pub dpt: &'a Dpt,
    pub metrics: &'a MetricsStore,
    pub prewarm_table: &'a mut PrewarmPlanTable,
    pub executor: &'a dyn PrewarmExecutor,
    pub prefix_config: PrefixConfig,
    pub urgency_config: UrgencyConfig,
}

impl<'a> PetHandler<'a> {
    pub fn handle_pet(&mut self, pet: PetRequest) -> PrewarmPlan {
        let sig = make_prefix_signature(&pet.workflow_id, &pet.prefix, self.prefix_config);
        let predicted = self
            .dpt
            .get(&sig)
            .map(|p| p.funcs.clone())
            .unwrap_or_default();

        let selected = select_prewarm_set(
            &pet,
            &predicted,
            self.metrics,
            self.prefix_config,
            self.prewarm_table,
            self.urgency_config,
        );

        let already: HashSet<FuncId> = self.prewarm_table.get_request_set(&pet.request_id);
        let to_prewarm: Vec<FuncId> = selected
            .iter()
            .cloned()
            .filter(|f| !already.contains(f))
            .collect();

        if !to_prewarm.is_empty() {
            self.executor.prewarm(&pet.request_id, &to_prewarm);
        }

        self.prewarm_table
            .set_window_result(pet.request_id.clone(), selected.clone());

        PrewarmPlan {
            actions: to_prewarm.into_iter().map(PrewarmAction::Inject).collect(),
        }
    }
}

