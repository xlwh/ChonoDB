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
    labels: serde_json::Map<String, serde_json::Value>,
    annotations: serde_json::Map<String, serde_json::Value>,
    state: String,
    active_at: Option<String>,
    value: String,
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
