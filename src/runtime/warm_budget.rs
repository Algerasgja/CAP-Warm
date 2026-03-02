use std::collections::HashMap;
use std::sync::Mutex;
use crate::types::FuncId;

pub struct PrewarmBudget {
    global_inflight: usize,
    global_limit: usize,
    func_inflight: HashMap<FuncId, usize>,
    func_limit: usize,
}

impl Default for PrewarmBudget {
    fn default() -> Self {
        Self {
            global_inflight: 0,
            global_limit: 100, // Default global limit
            func_inflight: HashMap::new(),
            func_limit: 10,    // Default per-function limit
        }
    }
}

impl PrewarmBudget {
    pub fn new(global_limit: usize, func_limit: usize) -> Self {
        Self {
            global_inflight: 0,
            global_limit,
            func_inflight: HashMap::new(),
            func_limit,
        }
    }

    pub fn can_prewarm(&self, func: &FuncId) -> bool {
        if self.global_inflight >= self.global_limit {
            return false;
        }
        if let Some(&count) = self.func_inflight.get(func) {
            if count >= self.func_limit {
                return false;
            }
        }
        true
    }

    pub fn acquire(&mut self, func: FuncId) {
        self.global_inflight += 1;
        *self.func_inflight.entry(func).or_insert(0) += 1;
    }

    pub fn release(&mut self, func: &FuncId) {
        if self.global_inflight > 0 {
            self.global_inflight -= 1;
        }
        if let Some(count) = self.func_inflight.get_mut(func) {
            if *count > 0 {
                *count -= 1;
            }
        }
    }
}
