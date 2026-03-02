use serde::{Deserialize, Serialize};
use std::fmt;
use std::collections::{HashMap, HashSet};

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FuncId(pub String);

impl FuncId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
}

impl From<&str> for FuncId {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

impl AsRef<str> for FuncId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for FuncId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct WorkflowId(pub String);

impl WorkflowId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
}

impl From<&str> for WorkflowId {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

impl AsRef<str> for WorkflowId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for WorkflowId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RunId(pub String);

impl RunId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
}

impl From<&str> for RunId {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

impl AsRef<str> for RunId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for RunId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PrefixSignature(pub String);

impl PrefixSignature {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
}

impl From<&str> for PrefixSignature {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

impl AsRef<str> for PrefixSignature {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for PrefixSignature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Prefix {
    pub funcs: Vec<FuncId>,
}

impl Prefix {
    pub fn new(funcs: Vec<FuncId>) -> Self {
        Self { funcs }
    }

    pub fn len(&self) -> usize {
        self.funcs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.funcs.is_empty()
    }

    pub fn push(&mut self, func_id: FuncId) {
        self.funcs.push(func_id);
    }

    pub fn last(&self) -> Option<&FuncId> {
        self.funcs.last()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PredictedPath {
    pub funcs: Vec<FuncId>,
}

impl PredictedPath {
    pub fn new(funcs: Vec<FuncId>) -> Self {
        Self { funcs }
    }

    pub fn len(&self) -> usize {
        self.funcs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.funcs.is_empty()
    }

    pub fn push(&mut self, func_id: FuncId) {
        self.funcs.push(func_id);
    }

    pub fn last(&self) -> Option<&FuncId> {
        self.funcs.last()
    }
}

pub type Timestamp = u64;
pub type Duration = u64;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EmaParams {
    pub alpha_exec: f64,
    pub alpha_cold: f64,
    pub alpha_trans: f64,
    pub alpha_count: f64,
}

impl Default for EmaParams {
    fn default() -> Self {
        Self {
            alpha_exec: 0.2,
            alpha_cold: 0.2,
            alpha_trans: 0.2,
            alpha_count: 0.2,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PrefixConfig {
    pub lmax: usize,
}

impl Default for PrefixConfig {
    fn default() -> Self {
        Self { lmax: 8 }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ExecStats {
    pub avg_exec: Duration,
    pub avg_cold: Duration,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TransitionStats {
    pub weighted_count: f64,
    pub avg_latency: Duration,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Urgency {
    pub margin: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PrewarmAction {
    Keep(FuncId),
    Inject(FuncId),
    Discard(FuncId),
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PrewarmPlan {
    pub actions: Vec<PrewarmAction>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExecutionEvent {
    PostExecution {
        workflow_id: WorkflowId,
        prefix: Prefix,
        actual_next: FuncId,
        timestamp: Timestamp,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Explore,
    Stable,
    Drift,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SystemPhase {
    pub phase: Phase,
    pub sample_size: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Observation {
    pub workflow_id: WorkflowId,
    pub prefix: Prefix,
    pub curr_func: FuncId,
    pub next_func: FuncId,
    pub exec_duration: Duration,
    pub is_cold: bool,
    pub cold_duration_opt: Option<Duration>,
    pub trans_latency_opt: Option<Duration>,
    pub timestamp: Timestamp,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OpenWhiskActivationObservation {
    pub workflow_id: WorkflowId,
    pub prefix: Prefix,
    pub func: FuncId,
    pub exec_duration: Duration,
    pub cold_start_duration: Option<Duration>,
    pub trans_latency: Option<Duration>,
    pub weight: f64,
    pub timestamp: Timestamp,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RequestId(pub String);

impl RequestId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
}

impl From<&str> for RequestId {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

impl AsRef<str> for RequestId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for RequestId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PetRequest {
    pub request_id: RequestId,
    pub workflow_id: WorkflowId,
    pub prefix: Prefix,
    pub curr_func: FuncId,
    pub timestamp: Timestamp,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColdBufferMode {
    ExecPlusCold,
    NextExecPlusCold,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UrgencyConfig {
    pub max_window_len: usize,
    pub default_exec: Duration,
    pub default_cold: Duration,
    pub default_trans: Duration,
    pub cold_buffer_mode: ColdBufferMode,
}

impl Default for UrgencyConfig {
    fn default() -> Self {
        Self {
            max_window_len: 16,
            default_exec: 1,
            default_cold: 0,
            default_trans: 0,
            cold_buffer_mode: ColdBufferMode::ExecPlusCold,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct PrewarmPlanTable {
    pub by_request: HashMap<RequestId, HashSet<FuncId>>,
}

impl PrewarmPlanTable {
    pub fn is_prewarmed(&self, request_id: &RequestId, func_id: &FuncId) -> bool {
        self.by_request
            .get(request_id)
            .map(|set| set.contains(func_id))
            .unwrap_or(false)
    }

    pub fn set_window_result(&mut self, request_id: RequestId, funcs: Vec<FuncId>) {
        let set: HashSet<FuncId> = funcs.into_iter().collect();
        self.by_request.insert(request_id, set);
    }

    pub fn get_request_set(&self, request_id: &RequestId) -> HashSet<FuncId> {
        self.by_request
            .get(request_id)
            .cloned()
            .unwrap_or_default()
    }
}

// LoadGen Events
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunStarted {
    pub workflow_id: WorkflowId,
    pub run_id: RunId,
    pub request_id: RequestId,
    pub timestamp: Timestamp,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PetEvent {
    pub workflow_id: WorkflowId,
    pub run_id: RunId,
    pub request_id: RequestId,
    pub prefix: Vec<FuncId>,
    pub curr_func: FuncId,
    pub next_func: FuncId,
    pub timestamp: Timestamp,
}

impl From<PetEvent> for PetRequest {
    fn from(e: PetEvent) -> Self {
        Self {
            request_id: e.request_id,
            workflow_id: e.workflow_id,
            prefix: Prefix::new(e.prefix),
            curr_func: e.curr_func,
            timestamp: e.timestamp,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActivationCompleted {
    pub workflow_id: WorkflowId,
    pub run_id: RunId,
    pub request_id: RequestId,
    pub prefix: Vec<FuncId>,
    pub func: FuncId,
    pub activation_id: String,
    pub start_ts: Timestamp,
    pub end_ts: Timestamp,
    pub exec_duration: Duration,
    pub cold_start_duration: Option<Duration>,
    pub transition_time: Duration,
    pub timestamp: Timestamp,
}

impl From<ActivationCompleted> for OpenWhiskActivationObservation {
    fn from(e: ActivationCompleted) -> Self {
        Self {
            workflow_id: e.workflow_id,
            prefix: Prefix::new(e.prefix),
            func: e.func,
            exec_duration: e.exec_duration,
            cold_start_duration: e.cold_start_duration,
            trans_latency: Some(e.transition_time), // Assuming transition_time is trans_latency
            weight: 1.0, // Default weight
            timestamp: e.timestamp,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunSummary {
    pub workflow_id: WorkflowId,
    pub run_id: RunId,
    pub request_id: RequestId,
    pub start_time: Timestamp,
    pub end_time: Timestamp,
    pub total_hops: usize,
    pub total_exec_duration: Duration,
    pub cold_start_count: usize,
    pub termination_reason: String,
}
