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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_query_request_deserialization() {
        let json = r#"{"query":"up","time":1234567890}"#;
        let req: QueryRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.query, "up");
        assert_eq!(req.time, Some(1234567890));
    }

    #[test]
    fn test_query_range_request_deserialization() {
        let json = r#"{"query":"up","start":1000,"end":2000,"step":15}"#;
        let req: QueryRangeRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.query, "up");
        assert_eq!(req.start, 1000);
        assert_eq!(req.end, 2000);
        assert_eq!(req.step, 15);
    }

    #[test]
    fn test_series_request_deserialization() {
        let json = r#"{"match[]":["up","down"],"start":1000,"end":2000}"#;
        let req: SeriesRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.matchers.len(), 2);
    }

    #[test]
    fn test_opentsdb_put_request_deserialization() {
        let json = r#"{"metric":"sys.cpu","timestamp":1234567890,"value":42.5,"tags":{"host":"server1"}}"#;
        let req: OpenTSDBPutRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.metric, "sys.cpu");
        assert_eq!(req.tags.len(), 1);
    }

    #[test]
    fn test_target_info_serialization() {
        let info = TargetInfo {
            discovered_labels: HashMap::new(),
            labels: HashMap::new(),
            scrape_pool: "default".to_string(),
            scrape_url: "http://localhost:9090/metrics".to_string(),
            last_error: None,
            last_scrape: Some(1000),
            health: "up".to_string(),
        };
        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("scrape_url"));
    }

    #[test]
    fn test_rule_info_serialization() {
        let info = RuleInfo {
            name: "HighCPU".to_string(),
            query: "cpu > 90".to_string(),
            duration: 300,
            labels: HashMap::new(),
            health: "ok".to_string(),
            evaluation_time: 0.5,
            last_evaluation: 1000,
            alerts: None,
        };
        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("HighCPU"));
    }

    #[test]
    fn test_opentsdb_put_summary_serialization() {
        let summary = OpenTSDBPutSummary {
            failed: 1,
            success: 9,
        };
        let json = serde_json::to_string(&summary).unwrap();
        assert!(json.contains("\"failed\":1"));
        assert!(json.contains("\"success\":9"));
    }

    #[test]
    fn test_opentsdb_error_response_serialization() {
        let resp = OpenTSDBErrorResponse {
            error: OpenTSDBErrorDetail {
                code: 400,
                message: "Bad request".to_string(),
            },
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"code\":400"));
    }
}
