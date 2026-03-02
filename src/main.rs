use cap_warm::config::load_config;
use cap_warm::core::dpt::Dpt;
use cap_warm::core::metrics_store::MetricsStore;
use cap_warm::runtime::http_server::{start_server, AppState};
use cap_warm::runtime::openwhisk::OpenWhiskClient;
use cap_warm::runtime::phase_manager::PhaseManager;
use cap_warm::runtime::warm_budget::PrewarmBudget;
use cap_warm::types::PrewarmPlanTable;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::info;

#[tokio::main]
async fn main() {
    // Initialize logging
    tracing_subscriber::fmt::init();

    // Load config
    let config = load_config().expect("Failed to load configuration");
    info!("Configuration loaded: {:?}", config);

    // Initialize components
    let metrics_store = Mutex::new(MetricsStore::default());
    let dpt = Mutex::new(Dpt::default());
    let prewarm_table = Mutex::new(PrewarmPlanTable::default());
    let budget = Mutex::new(PrewarmBudget::default());

    let executor = Arc::new(OpenWhiskClient::new(&config.openwhisk).expect("Failed to create OpenWhisk client"));

    // Phase Manager now manages its own lifecycle logic
    let phase_manager = Arc::new(PhaseManager::new());

    let state = Arc::new(AppState {
        metrics_store,
        dpt,
        prewarm_table,
        budget,
        phase_manager,
        executor,
        prefix_config: config.prefix.into(),
        urgency_config: config.urgency.into(),
        default_ema_params: config.ema.into(),
    });

    // Start background tasks
    // Rebuild scheduler is now integrated into PhaseManager logic (triggered by events)
    // But we might still want a periodic check for Stable phase to ensure regular rebuilds even with low traffic?
    // PhaseManager implementation uses event-driven checks, which is fine. 
    // If traffic is zero, we don't need to rebuild.
    
    // We removed separate RebuildScheduler and PhaseManager background loops 
    // because logic is now driven by request events in PhaseManager::record_observation

    start_server(state, config.http.port).await;
}
