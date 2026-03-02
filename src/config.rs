use serde::Deserialize;
use crate::types::{ColdBufferMode, EmaParams, PrefixConfig, UrgencyConfig, Duration};
use crate::runtime::openwhisk::OpenWhiskConfig;

#[derive(Clone, Debug, Deserialize)]
pub struct AppConfig {
    pub http: HttpConfig,
    pub openwhisk: OpenWhiskConfig,
    pub ema: EmaParamsConfig,
    pub urgency: UrgencyConfigConfig,
    pub prefix: PrefixConfigConfig,
}

#[derive(Clone, Debug, Deserialize)]
pub struct HttpConfig {
    pub bind_addr: String,
    pub port: u16,
}

#[derive(Clone, Debug, Deserialize)]
pub struct EmaParamsConfig {
    pub alpha_exec: f64,
    pub alpha_cold: f64,
    pub alpha_trans: f64,
    pub alpha_count: f64,
}

impl From<EmaParamsConfig> for EmaParams {
    fn from(c: EmaParamsConfig) -> Self {
        Self {
            alpha_exec: c.alpha_exec,
            alpha_cold: c.alpha_cold,
            alpha_trans: c.alpha_trans,
            alpha_count: c.alpha_count,
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct UrgencyConfigConfig {
    pub max_window_len: usize,
    pub default_exec: Duration,
    pub default_cold: Duration,
    pub default_trans: Duration,
    pub cold_buffer_mode: String,
}

impl From<UrgencyConfigConfig> for UrgencyConfig {
    fn from(c: UrgencyConfigConfig) -> Self {
        let mode = match c.cold_buffer_mode.as_str() {
            "NextExecPlusCold" => ColdBufferMode::NextExecPlusCold,
            _ => ColdBufferMode::ExecPlusCold,
        };
        Self {
            max_window_len: c.max_window_len,
            default_exec: c.default_exec,
            default_cold: c.default_cold,
            default_trans: c.default_trans,
            cold_buffer_mode: mode,
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct PrefixConfigConfig {
    pub lmax: usize,
}

impl From<PrefixConfigConfig> for PrefixConfig {
    fn from(c: PrefixConfigConfig) -> Self {
        Self { lmax: c.lmax }
    }
}

pub fn load_config() -> Result<AppConfig, config::ConfigError> {
    let builder = config::Config::builder()
        .add_source(config::File::with_name("Settings").required(false))
        .add_source(config::Environment::default().separator("__"))
        .set_default("http.bind_addr", "0.0.0.0")?
        .set_default("http.port", 3000)?
        .set_default("openwhisk.base_url", "http://owdev-nginx.openwhisk.svc.cluster.local:80")?
        .set_default("openwhisk.api_key", "23bc46b1-71f6-4ed5-8c54-816aa4f8c502:123zO3xZCLrMN6v2BKK1dXYFpXlPkccOFqm12CdAsMgRU4VrNZ9lyGVCGuMDGIwP")?
        .set_default("openwhisk.namespace", "guest")?
        .set_default("openwhisk.timeout_secs", 5)?
        .set_default("ema.alpha_exec", 0.2)?
        .set_default("ema.alpha_cold", 0.2)?
        .set_default("ema.alpha_trans", 0.2)?
        .set_default("ema.alpha_count", 0.2)?
        .set_default("urgency.max_window_len", 16)?
        .set_default("urgency.default_exec", 1)?
        .set_default("urgency.default_cold", 0)?
        .set_default("urgency.default_trans", 0)?
        .set_default("urgency.cold_buffer_mode", "ExecPlusCold")?
        .set_default("prefix.lmax", 8)?;

    builder.build()?.try_deserialize()
}
