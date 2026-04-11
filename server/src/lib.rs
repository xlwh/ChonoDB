pub mod api;
pub mod config;
pub mod error;
pub mod rules;
pub mod server;
pub mod state;
pub mod service_discovery;
pub mod nlp;
pub mod federation;
pub mod targets;
pub mod remote_server;
pub mod static_files;
pub mod monitoring;

pub use server::Server;
pub use config::ServerConfig;
pub use error::{ServerError, Result};
pub use monitoring::{MonitoringSystem, MonitoringConfig, Metric, MetricType, AlertRule, AlertLevel};
