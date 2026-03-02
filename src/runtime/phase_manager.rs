use std::sync::atomic::{AtomicUsize, Ordering};
use std::collections::VecDeque;
use tokio::sync::RwLock;
use tracing::{info, warn};
use std::sync::Arc;

use crate::types::{EmaParams, Phase};
use crate::runtime::http_server::AppState;
use crate::core::dpt::{build_dpt, BuildConfig};

// Configuration constants
const WINDOW_SIZE: usize = 100; // K
const VAR_THRESHOLD: f64 = 0.05; // theta_var
const MIN_SAMPLES: usize = 50; // N_min
const DRIFT_THRESHOLD_SIGMA: f64 = 3.0;
const DRIFT_THRESHOLD_ABS: f64 = 0.70; // 70% miss rate
const DECAY_FACTOR: f64 = 0.5; // gamma

// Phase-dependent params
const ALPHA_EXPLORE: f64 = 0.8;
const ALPHA_STABLE: f64 = 0.15;
const REBUILD_FREQ_EXPLORE: usize = 10;
const REBUILD_FREQ_STABLE: usize = 1000; // Approximation for "per hour" if traffic is high

pub struct PhaseManager {
    // Shared state
    current_phase: RwLock<Phase>,
    ema_params: RwLock<EmaParams>,
    
    // Statistics window
    window: RwLock<VecDeque<bool>>, // true = hit (prediction match), false = miss (prediction mismatch)
    
    // Drift detection stats
    history_miss_rate_mean: RwLock<f64>,
    history_miss_rate_var: RwLock<f64>,
    
    // Counters
    observation_count: AtomicUsize,
    last_rebuild_count: AtomicUsize,
}

impl PhaseManager {
    pub fn new() -> Self {
        Self {
            current_phase: RwLock::new(Phase::Explore),
            ema_params: RwLock::new(Self::get_params_for_phase(Phase::Explore)),
            window: RwLock::new(VecDeque::with_capacity(WINDOW_SIZE)),
            history_miss_rate_mean: RwLock::new(0.5), // Initial guess
            history_miss_rate_var: RwLock::new(0.1),
            observation_count: AtomicUsize::new(0),
            last_rebuild_count: AtomicUsize::new(0),
        }
    }

    fn get_params_for_phase(phase: Phase) -> EmaParams {
        let alpha = match phase {
            Phase::Explore | Phase::Drift => ALPHA_EXPLORE,
            Phase::Stable => ALPHA_STABLE,
        };
        EmaParams {
            alpha_exec: alpha,
            alpha_cold: alpha,
            alpha_trans: alpha,
            alpha_count: alpha,
        }
    }

    pub async fn get_current_ema_params(&self) -> EmaParams {
        *self.ema_params.read().await
    }

    // Now accepts is_hit directly (true = prediction matched, false = prediction mismatched)
    pub async fn record_observation(&self, is_hit: bool) {
        let mut window = self.window.write().await;
        
        if window.len() >= WINDOW_SIZE {
            window.pop_front();
        }
        window.push_back(is_hit);
        
        self.observation_count.fetch_add(1, Ordering::Relaxed);
        
        // Analyze phase after update
        self.analyze_phase(&window).await;
    }

    async fn analyze_phase(&self, window: &VecDeque<bool>) {
        let n = window.len();
        if n == 0 { return; }

        let hits = window.iter().filter(|&&h| h).count();
        let hit_rate = hits as f64 / n as f64;
        let miss_rate = 1.0 - hit_rate;

        // Calculate variance of hit rate (simplified: variance of Bernoulli trial is p(1-p))
        let variance = hit_rate * (1.0 - hit_rate);

        let mut phase = self.current_phase.write().await;
        let old_phase = *phase;
        
        // 1. Check Drift
        let drift_detected = {
            let mu = *self.history_miss_rate_mean.read().await;
            let sigma = self.history_miss_rate_var.read().await.sqrt();
            
            // Update history stats (simple moving average for stability)
            // We only update history stats if we were in Stable phase to capture baseline
            if old_phase == Phase::Stable {
                let mut mu_guard = self.history_miss_rate_mean.write().await;
                let mut var_guard = self.history_miss_rate_var.write().await;
                *mu_guard = 0.99 * *mu_guard + 0.01 * miss_rate;
                *var_guard = 0.99 * *var_guard + 0.01 * ((miss_rate - *mu_guard).powi(2));
            }

            miss_rate > mu + DRIFT_THRESHOLD_SIGMA * sigma || miss_rate > DRIFT_THRESHOLD_ABS
        };

        let new_phase = if drift_detected && old_phase == Phase::Stable {
            info!("Drift detected! Miss Rate: {:.2}, Baseline: {:.2} +/- 3*{:.2}", miss_rate, *self.history_miss_rate_mean.read().await, self.history_miss_rate_var.read().await.sqrt());
            Phase::Drift
        } else if variance > VAR_THRESHOLD || n < MIN_SAMPLES {
            Phase::Explore
        } else {
            Phase::Stable
        };

        if new_phase != old_phase {
            info!("Phase transition: {:?} -> {:?}", old_phase, new_phase);
            *phase = new_phase;
            
            // Update EMA params
            let mut params = self.ema_params.write().await;
            *params = Self::get_params_for_phase(new_phase);

            // Handle Drift specific actions
            if new_phase == Phase::Drift {
                // Decay mechanism will be triggered by the caller or here if we have access to metrics
            }
        }
    }

    pub async fn check_and_trigger_rebuild(&self, app_state: &AppState) {
        let phase = *self.current_phase.read().await;
        let count = self.observation_count.load(Ordering::Relaxed);
        let last = self.last_rebuild_count.load(Ordering::Relaxed);
        let diff = count - last;

        let should_rebuild = match phase {
            Phase::Explore | Phase::Drift => diff >= REBUILD_FREQ_EXPLORE,
            Phase::Stable => diff >= REBUILD_FREQ_STABLE,
        };

        // If Drift just happened, we might want to trigger immediate decay
        if phase == Phase::Drift && diff == 0 {
             self.apply_decay(app_state).await;
        }

        if should_rebuild {
            self.rebuild_dpt(app_state).await;
            self.last_rebuild_count.store(count, Ordering::Relaxed);
        }
    }

    async fn apply_decay(&self, app_state: &AppState) {
        info!("Applying decay factor {} to all metrics due to Drift", DECAY_FACTOR);
        let mut metrics = app_state.metrics_store.lock().await;
        metrics.decay(DECAY_FACTOR);
        // Force transition to Explore after decay? 
        // The logic says "switch back to explore state", which we already did in analyze_phase
    }

    async fn rebuild_dpt(&self, app_state: &AppState) {
        let phase = *self.current_phase.read().await;
        info!("Triggering DPT rebuild (Phase: {:?})", phase);
        
        let current_version = {
            let dpt = app_state.dpt.lock().await;
            dpt.version()
        };
        let new_version = current_version + 1;

        let metrics_snapshot = {
            let metrics = app_state.metrics_store.lock().await;
            metrics.clone()
        };

        let prefix_config = app_state.prefix_config;
        
        // Spawn blocking task
        let new_dpt = tokio::task::spawn_blocking(move || {
            build_dpt(
                &metrics_snapshot, 
                prefix_config, 
                BuildConfig::default(), 
                new_version
            )
        }).await.unwrap();

        {
            let mut dpt_guard = app_state.dpt.lock().await;
            *dpt_guard = new_dpt;
        }
        
        info!("DPT rebuild completed. Version: {}", new_version);
    }
}
