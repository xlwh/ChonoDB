use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Deserialize)]
pub struct QueryRequest {
    pub query: String,
    pub time: Option<i64>,
    pub timeout: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct QueryRangeRequest {
    pub query: String,
    pub start: i64,
    pub end: i64,
    pub step: i64,
    pub timeout: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SeriesRequest {
    #[serde(rename = "match[]")]
    pub matchers: Vec<String>,
    pub start: Option<i64>,
    pub end: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LabelValuesRequest {
    pub name: String,
    pub start: Option<i64>,
    pub end: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TargetInfo {
    pub discovered_labels: HashMap<String, String>,
    pub labels: HashMap<String, String>,
    pub scrape_pool: String,
    pub scrape_url: String,
    pub last_error: Option<String>,
    pub last_scrape: Option<i64>,
    pub health: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct RuleInfo {
    pub name: String,
    pub query: String,
    pub duration: i64,
    pub labels: HashMap<String, String>,
    pub health: String,
    pub evaluation_time: f64,
    pub last_evaluation: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alerts: Option<Vec<AlertInfo>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AlertInfo {
    pub labels: HashMap<String, String>,
    pub annotations: HashMap<String, String>,
    pub state: String,
    pub active_at: Option<i64>,
    pub value: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct RuntimeInfo {
    pub start_time: String,
    pub cwd: String,
    pub reload_config_success: bool,
    pub last_config_time: Option<String>,
    pub chunk_count: i64,
    pub time_series_count: i64,
    pub corruption_count: i64,
    pub goroutine_count: i64,
    pub go_max_procs: i64,
    pub go_version: String,
    pub go_arch: String,
    pub go_os: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct BuildInfo {
    pub version: String,
    pub revision: String,
    pub branch: String,
    pub build_user: String,
    pub build_date: String,
    pub go_version: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OpenTSDBPutRequest {
    pub metric: String,
    pub timestamp: i64,
    pub value: serde_json::Value,
    pub tags: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct OpenTSDBPutSummary {
    pub failed: usize,
    pub success: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct OpenTSDBPutDetail {
    pub failed: usize,
    pub success: usize,
    pub errors: Vec<OpenTSDBPutError>,
}

#[derive(Debug, Clone, Serialize)]
pub struct OpenTSDBPutError {
    pub datapoint: OpenTSDBPutRequest,
    pub error: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct OpenTSDBErrorResponse {
    pub error: OpenTSDBErrorDetail,
}

#[derive(Debug, Clone, Serialize)]
pub struct OpenTSDBErrorDetail {
    pub code: u16,
    pub message: String,
}
