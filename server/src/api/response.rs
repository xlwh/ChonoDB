use serde::{Deserialize, Serialize};

/// Prometheus API 标准响应格式
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    /// 状态: "success" | "error"
    pub status: String,
    
    /// 数据（成功时）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    
    /// 错误类型（错误时）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_type: Option<String>,
    
    /// 错误信息（错误时）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    
    /// 警告信息
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warnings: Option<Vec<String>>,
}

impl<T> ApiResponse<T> {
    /// 创建成功响应
    pub fn success(data: T) -> Self {
        Self {
            status: "success".to_string(),
            data: Some(data),
            error_type: None,
            error: None,
            warnings: None,
        }
    }
    
    /// 创建错误响应
    pub fn error(error_type: &str, error: &str) -> Self {
        Self {
            status: "error".to_string(),
            data: None,
            error_type: Some(error_type.to_string()),
            error: Some(error.to_string()),
            warnings: None,
        }
    }
    
    /// 添加警告
    pub fn with_warnings(mut self, warnings: Vec<String>) -> Self {
        self.warnings = Some(warnings);
        self
    }
}

/// 查询结果数据
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "resultType", content = "result")]
pub enum QueryResult {
    #[serde(rename = "vector")]
    Vector(Vec<InstantVector>),
    
    #[serde(rename = "matrix")]
    Matrix(Vec<RangeVector>),
    
    #[serde(rename = "scalar")]
    Scalar(f64),
    
    #[serde(rename = "string")]
    String(String),
}

/// 瞬时向量
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstantVector {
    pub metric: serde_json::Map<String, serde_json::Value>,
    pub value: (f64, String),
}

/// 范围向量
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RangeVector {
    pub metric: serde_json::Map<String, serde_json::Value>,
    pub values: Vec<(f64, String)>,
}

/// 系列元数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Series {
    #[serde(flatten)]
    pub labels: serde_json::Map<String, serde_json::Value>,
}

/// 目标信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Target {
    /// 目标发现来源
    pub discovered_labels: serde_json::Map<String, serde_json::Value>,
    
    /// 抓取前的标签
    pub labels: serde_json::Map<String, serde_json::Value>,
    
    /// 抓取 URL
    pub scrape_url: String,
    
    /// 最后错误
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
    
    /// 最后抓取时间
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_scrape: Option<String>,
    
    /// 健康状态
    pub health: String,
    
    /// 抓取间隔
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scrape_interval: Option<String>,
    
    /// 抓取超时
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scrape_timeout: Option<String>,
}

/// 规则组
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleGroup {
    pub name: String,
    pub file: String,
    pub interval: f64,
    pub limit: i64,
    pub rules: Vec<Rule>,
}

/// 规则
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Rule {
    #[serde(rename = "recording")]
    Recording {
        name: String,
        query: String,
        labels: serde_json::Map<String, serde_json::Value>,
        health: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        last_error: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        evaluation_time: Option<f64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        last_evaluation: Option<String>,
    },
    
    #[serde(rename = "alerting")]
    Alerting {
        name: String,
        query: String,
        duration: f64,
        labels: serde_json::Map<String, serde_json::Value>,
        annotations: serde_json::Map<String, serde_json::Value>,
        alerts: Vec<Alert>,
        health: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        last_error: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        evaluation_time: Option<f64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        last_evaluation: Option<String>,
    },
}

/// 告警
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alert {
    pub labels: serde_json::Map<String, serde_json::Value>,
    pub annotations: serde_json::Map<String, serde_json::Value>,
    pub state: String,
    pub active_at: Option<String>,
    pub value: String,
}

/// 运行时信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeInfo {
    pub start_time: String,
    pub cwd: String,
    pub reload_config_success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
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

/// 构建信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildInfo {
    pub version: String,
    pub revision: String,
    pub branch: String,
    #[serde(rename = "buildUser")]
    pub build_user: String,
    #[serde(rename = "buildDate")]
    pub build_date: String,
    #[serde(rename = "goVersion")]
    pub go_version: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_response_success() {
        let resp = ApiResponse::success(42);
        assert_eq!(resp.status, "success");
        assert_eq!(resp.data, Some(42));
        assert!(resp.error_type.is_none());
        assert!(resp.error.is_none());
    }

    #[test]
    fn test_api_response_error() {
        let resp: ApiResponse<i32> = ApiResponse::error("bad_data", "invalid query");
        assert_eq!(resp.status, "error");
        assert!(resp.data.is_none());
        assert_eq!(resp.error_type, Some("bad_data".to_string()));
        assert_eq!(resp.error, Some("invalid query".to_string()));
    }

    #[test]
    fn test_api_response_with_warnings() {
        let resp = ApiResponse::success("data")
            .with_warnings(vec!["warning1".to_string()]);
        assert_eq!(resp.status, "success");
        assert_eq!(resp.warnings, Some(vec!["warning1".to_string()]));
    }

    #[test]
    fn test_api_response_serialization() {
        let resp = ApiResponse::success(vec![1, 2, 3]);
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"status\":\"success\""));
        assert!(json.contains("\"data\":[1,2,3]"));
    }

    #[test]
    fn test_api_response_error_serialization() {
        let resp: ApiResponse<String> = ApiResponse::error("execution", "timeout");
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"status\":\"error\""));
        assert!(json.contains("\"error_type\":\"execution\""));
        assert!(json.contains("\"error\":\"timeout\""));
    }

    #[test]
    fn test_query_result_vector_serialization() {
        let result = QueryResult::Vector(vec![InstantVector {
            metric: serde_json::Map::new(),
            value: (1234567890.0, "42".to_string()),
        }]);
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"resultType\":\"vector\""));
    }

    #[test]
    fn test_query_result_matrix_serialization() {
        let result = QueryResult::Matrix(vec![RangeVector {
            metric: serde_json::Map::new(),
            values: vec![(1234567890.0, "42".to_string())],
        }]);
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"resultType\":\"matrix\""));
    }

    #[test]
    fn test_query_result_scalar_serialization() {
        let result = QueryResult::Scalar(42.0);
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"resultType\":\"scalar\""));
    }

    #[test]
    fn test_query_result_string_serialization() {
        let result = QueryResult::String("hello".to_string());
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"resultType\":\"string\""));
    }

    #[test]
    fn test_series_serialization() {
        let mut labels = serde_json::Map::new();
        labels.insert("__name__".to_string(), serde_json::Value::String("test".to_string()));
        let series = Series { labels };
        let json = serde_json::to_string(&series).unwrap();
        assert!(json.contains("__name__"));
    }

    #[test]
    fn test_build_info_serialization() {
        let info = BuildInfo {
            version: "1.0.0".to_string(),
            revision: "abc".to_string(),
            branch: "main".to_string(),
            build_user: "ci".to_string(),
            build_date: "2024-01-01".to_string(),
            go_version: "rust".to_string(),
        };
        let json = serde_json::to_string(&info).unwrap();
        let deserialized: BuildInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.version, "1.0.0");
    }

    #[test]
    fn test_runtime_info_serialization() {
        let info = RuntimeInfo {
            start_time: "2024-01-01T00:00:00Z".to_string(),
            cwd: "/app".to_string(),
            reload_config_success: true,
            last_config_time: None,
            chunk_count: 0,
            time_series_count: 100,
            corruption_count: 0,
            goroutine_count: 10,
            go_max_procs: 8,
            go_version: "rust".to_string(),
            go_arch: "x86_64".to_string(),
            go_os: "linux".to_string(),
        };
        let json = serde_json::to_string(&info).unwrap();
        let deserialized: RuntimeInfo = serde_json::from_str(&json).unwrap();
        assert!(deserialized.reload_config_success);
    }
}
