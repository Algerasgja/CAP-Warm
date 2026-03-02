use axum::{
    routing::{get, post},
    Json, Router, http::StatusCode,
};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{info, warn, error};

use crate::core::dpt::Dpt;
use crate::core::metrics_store::MetricsStore;
use crate::runtime::pet_trigger::{PetHandler, PrewarmExecutor};
use crate::runtime::warm_budget::PrewarmBudget;
use crate::types::{
    ActivationCompleted, EmaParams, PetEvent, PrefixConfig,
    PrewarmPlanTable, RunStarted, RunSummary, UrgencyConfig,
};
use crate::runtime::phase_manager::PhaseManager;

// Global state for the application
pub struct AppState {
    pub metrics_store: Mutex<MetricsStore>,
    pub dpt: Mutex<Dpt>,
    pub prewarm_table: Mutex<PrewarmPlanTable>,
    pub budget: Mutex<PrewarmBudget>,
    pub phase_manager: Arc<PhaseManager>,
    pub executor: Arc<dyn PrewarmExecutor + Send + Sync>,
    pub prefix_config: PrefixConfig,
    pub urgency_config: UrgencyConfig,
    // ema_params is now managed by PhaseManager, but we keep initial/default config here
    // or we could remove it if it's dynamic. For now, let's keep it as initial config.
    pub default_ema_params: EmaParams, 
}

pub async fn start_server(state: Arc<AppState>, port: u16) {
    let app = Router::new()
        .route("/healthz", get(health_check))
        .route("/run/start", post(handle_run_start))
        .route("/pet", post(handle_pet))
        .route("/activation/complete", post(handle_activation_complete))
        .route("/run/summary", post(handle_run_summary))
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    info!("Listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn health_check() -> &'static str {
    "OK"
}

async fn handle_run_start(
    axum::extract::State(_state): axum::extract::State<Arc<AppState>>,
    Json(event): Json<RunStarted>,
) -> StatusCode {
    info!(
        "Run Started: request_id={}, workflow_id={}",
        event.request_id, event.workflow_id
    );
    // TODO: Create run context if needed
    StatusCode::OK
}

async fn handle_pet(
    axum::extract::State(state): axum::extract::State<Arc<AppState>>,
    Json(event): Json<PetEvent>,
) -> StatusCode {
    info!(
        "PET Triggered: request_id={}, func={}",
        event.request_id, event.curr_func
    );

    let dpt = state.dpt.lock().await;
    let metrics = state.metrics_store.lock().await;
    let mut prewarm_table = state.prewarm_table.lock().await;
    
    // We check if DPT's predicted next function matches event.next_func
    // 1. Construct prefix signature
    use crate::core::prefix_model::make_prefix_signature;
    let prefix = crate::types::Prefix::new(event.prefix.clone());
    let sig = make_prefix_signature(&event.workflow_id, &prefix, state.prefix_config);

    // 2. Query DPT for prediction
    // DPT returns a PredictedPath, which is a sequence of future functions. 
    // The first one is the immediate next function.
    let predicted_next = dpt.get_prediction(&sig)
        .and_then(|path| path.funcs.first().cloned());
    
    // 3. Compare with actual next_func from event
    let is_hit = if let Some(pred) = predicted_next {
        pred == event.next_func
    } else {
        false // No prediction means miss
    };
    
    {
        // We need to create a temporary handler because PetHandler holds references
        // In a real app we might restructure PetHandler to own data or use Arc
        let mut handler = PetHandler {
            dpt: &dpt,
            metrics: &metrics,
            prewarm_table: &mut prewarm_table,
            executor: state.executor.as_ref(),
            prefix_config: state.prefix_config,
            urgency_config: state.urgency_config,
        };

        let plan = handler.handle_pet(event.clone().into());
        info!(
            "PET Decision: request_id={}, plan_actions={}",
            event.request_id,
            plan.actions.len()
        );
    } // handler dropped here

    // Drop locks before async calls
    drop(dpt);
    drop(metrics);
    drop(prewarm_table);

    // 4. Update Phase Manager
    state.phase_manager.record_observation(is_hit).await;
    
    // 5. Check trigger rebuild
    state.phase_manager.check_and_trigger_rebuild(&state).await;

    // In future, we could return the plan as JSON
    StatusCode::OK
}

async fn handle_activation_complete(
    axum::extract::State(state): axum::extract::State<Arc<AppState>>,
    Json(event): Json<ActivationCompleted>,
) -> StatusCode {
    info!(
        "Activation Completed: request_id={}, func={}, duration={}ms, cold={:?}",
        event.request_id, event.func, event.exec_duration, event.cold_start_duration
    );

    // Old logic: Update Phase Manager here (REMOVED)
    // let is_cold = event.cold_start_duration.is_some();
    // state.phase_manager.record_observation(is_cold).await;
    // state.phase_manager.check_and_trigger_rebuild(&state).await;

    let mut metrics = state.metrics_store.lock().await;
    // Get dynamic EMA params from Phase Manager
    let ema_params = state.phase_manager.get_current_ema_params().await;
    
    metrics.ingest_openwhisk_activation(&event.into(), ema_params, state.prefix_config);
    
    StatusCode::OK
}

async fn handle_run_summary(
    axum::extract::State(_state): axum::extract::State<Arc<AppState>>,
    Json(event): Json<RunSummary>,
) -> StatusCode {
    info!(
        "Run Summary: request_id={}, reason={}, total_exec_duration={}ms",
        event.request_id, event.termination_reason, event.total_exec_duration
    );
    // TODO: Update phase model stats
    StatusCode::OK
}
