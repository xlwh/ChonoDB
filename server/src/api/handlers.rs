use chronodb_storage::model::{Sample, TimeSeries};
use axum::{
    extract::{Query, State},
    response::Json,
};
use chrono;
use std::collections::HashMap;
use std::sync::Arc;

use crate::api::response::*;
use crate::state::ServerState;

/// 处理即时查询
pub async fn handle_query(
    State(state): State<Arc<ServerState>>,
    Query(params): Query<HashMap<String, String>>,
) -> Json<ApiResponse<QueryResult>> {
    let query = match params.get("query") {
        Some(q) => q,
        None => {
            return Json(ApiResponse::error(
                "bad_data",
                "Parameter 'query' is required",
            ));
        }
    };

    // 解析时间参数
    let time = params.get("time")
        .and_then(|t| t.parse::<f64>().ok())
        .map(|t| (t * 1000.0) as i64)
        .unwrap_or_else(|| chrono::Utc::now().timestamp_millis());

    // 简单的标签匹配解析（实际项目中需要更复杂的PromQL解析）
    let label_matchers = parse_label_matchers(query);

    // 使用内存存储查询数据
    let result = match state.memstore.query(&label_matchers, time - 1000, time) {
        Ok(series) => {
            let instant_vectors: Vec<InstantVector> = series.into_iter()
                .flat_map(|ts| {
                    let mut metric = serde_json::Map::new();
                    for label in &ts.labels {
                        metric.insert(label.name.clone(), serde_json::Value::String(label.value.clone()));
                    }
                    ts.samples.into_iter().map(move |s| {
                        InstantVector {
                            metric: metric.clone(),
                            value: (s.timestamp as f64 / 1000.0, s.value.to_string()),
                        }
                    })
                })
                .collect();
            QueryResult::Vector(instant_vectors)
        },
        Err(e) => {
            return Json(ApiResponse::error(
                "execution",
                &format!("Query execution failed: {:?}", e),
            ));
        }
    };

    Json(ApiResponse::success(result))
}

/// 简单的标签匹配解析
pub fn parse_label_matchers(query: &str) -> Vec<(String, String)> {
    // 这里只是一个简单的实现，实际项目中需要使用PromQL解析器
    let mut matchers = Vec::new();
    
    // 如果查询是简单的指标名
    if !query.contains('{') && !query.contains('}') {
        matchers.push(("__name__".to_string(), query.to_string()));
        return matchers;
    }
    
    // 简单解析标签匹配
    if let Some(start) = query.find('{') {
        if let Some(end) = query.find('}') {
            let labels_str = &query[start+1..end];
            for label in labels_str.split(',') {
                let parts: Vec<&str> = label.split('=').collect();
                if parts.len() == 2 {
                    let name = parts[0].trim().to_string();
                    let value = parts[1].trim().trim_matches('"').to_string();
                    matchers.push((name, value));
                }
            }
        }
    }
    
    matchers
}

/// 处理范围查询
pub async fn handle_query_range(
    State(state): State<Arc<ServerState>>,
    Query(params): Query<HashMap<String, String>>,
) -> Json<ApiResponse<QueryResult>> {
    let query = match params.get("query") {
        Some(q) => q,
        None => {
            return Json(ApiResponse::error(
                "bad_data",
                "Parameter 'query' is required",
            ));
        }
    };

    // 解析时间参数
    let start = match params.get("start") {
        Some(s) => s.parse::<f64>().ok().map(|t| (t * 1000.0) as i64),
        None => None,
    };

    let end = match params.get("end") {
        Some(e) => e.parse::<f64>().ok().map(|t| (t * 1000.0) as i64),
        None => None,
    };

    let (start, end) = match (start, end) {
        (Some(s), Some(e)) => (s, e),
        _ => {
            return Json(ApiResponse::error(
                "bad_data",
                "Parameters 'start' and 'end' are required",
            ));
        }
    };

    // 解析标签匹配
    let label_matchers = parse_label_matchers(query);

    // 使用内存存储查询数据
    let result = match state.memstore.query(&label_matchers, start, end) {
        Ok(series) => {
            let range_vectors: Vec<RangeVector> = series.into_iter()
                .map(|ts| {
                    let values: Vec<(f64, String)> = ts.samples.into_iter()
                        .map(|s| (s.timestamp as f64 / 1000.0, s.value.to_string()))
                        .collect();
                    let mut metric = serde_json::Map::new();
                    for label in ts.labels {
                        metric.insert(label.name, serde_json::Value::String(label.value));
                    }
                    RangeVector {
                        metric,
                        values,
                    }
                })
                .collect();
            QueryResult::Matrix(range_vectors)
        },
        Err(e) => {
            return Json(ApiResponse::error(
                "execution",
                &format!("Query execution failed: {:?}", e),
            ));
        }
    };

    Json(ApiResponse::success(result))
}

/// 处理系列查询
pub async fn handle_series(
    State(state): State<Arc<ServerState>>,
    Query(params): Query<HashMap<String, String>>,
) -> Json<ApiResponse<Vec<Series>>> {
    // 解析match参数
    let matchers: Vec<&str> = params.get("match[]")
        .map(|m| m.split(',').collect())
        .unwrap_or_default();

    // 解析时间参数
    let start = params.get("start")
        .and_then(|s| s.parse::<f64>().ok())
        .map(|t| (t * 1000.0) as i64);

    let end = params.get("end")
        .and_then(|e| e.parse::<f64>().ok())
        .map(|t| (t * 1000.0) as i64);

    let (start, end) = (start.unwrap_or(0), end.unwrap_or(chrono::Utc::now().timestamp_millis()));

    // 处理每个match表达式
    let mut all_series = Vec::new();
    for matcher in matchers {
        let label_matchers = parse_label_matchers(matcher);
        
        if let Ok(series) = state.memstore.query(&label_matchers, start, end) {
            let api_series: Vec<Series> = series.into_iter()
                .map(|ts| {
                    let mut labels = serde_json::Map::new();
                    for label in ts.labels {
                        labels.insert(label.name, serde_json::Value::String(label.value));
                    }
                    Series {
                        labels,
                    }
                })
                .collect();
            all_series.extend(api_series);
        }
    }

    Json(ApiResponse::success(all_series))
}

/// 处理标签查询
pub async fn handle_labels(
    State(state): State<Arc<ServerState>>,
    Query(params): Query<HashMap<String, String>>,
) -> Json<ApiResponse<Vec<String>>> {
    // 使用内存存储获取标签名
    let labels = state.memstore.label_names();
    Json(ApiResponse::success(labels))
}

/// 处理标签值查询
pub async fn handle_label_values(
    State(state): State<Arc<ServerState>>,
    axum::extract::Path(name): axum::extract::Path<String>,
    Query(params): Query<HashMap<String, String>>,
) -> Json<ApiResponse<Vec<String>>> {
    // 使用内存存储获取标签值
    let values = state.memstore.label_values(&name);
    Json(ApiResponse::success(values))
}

/// 处理目标查询
pub async fn handle_targets(
    State(state): State<Arc<ServerState>>,
) -> Json<ApiResponse<HashMap<String, Vec<Target>>>> {
    let target_manager = state.target_manager.read().await;
    let targets: Vec<Target> = target_manager
        .get_all_targets()
        .iter()
        .map(|t| Target {
            discovered_labels: serde_json::Map::new(),
            labels: serde_json::Map::new(),
            scrape_url: t.url.clone(),
            last_error: t.last_error.clone(),
            last_scrape: t
                .last_scrape
                .map(|s| format!("{:?}", s)),
            health: match t.health {
                crate::targets::TargetHealth::Up => "up".to_string(),
                crate::targets::TargetHealth::Down => "down".to_string(),
                crate::targets::TargetHealth::Unknown => "unknown".to_string(),
            },
            scrape_interval: Some(format!("{}s", t.scrape_interval)),
            scrape_timeout: Some(format!("{}s", t.scrape_timeout)),
        })
        .collect();

    let mut data = HashMap::new();
    data.insert("activeTargets".to_string(), targets);
    data.insert("droppedTargets".to_string(), vec![]);

    Json(ApiResponse::success(data))
}

/// 处理规则查询
pub async fn handle_rules(
    State(state): State<Arc<ServerState>>,
) -> Json<ApiResponse<HashMap<String, Vec<RuleGroup>>>> {
    let rule_manager = state.rule_manager.read().await;

    let groups: Vec<RuleGroup> = rule_manager
        .get_groups()
        .iter()
        .map(|g| RuleGroup {
            name: g.name.clone(),
            file: "rules.yml".to_string(),
            interval: g.interval.map(|d| d.as_secs_f64()).unwrap_or(60.0),
            limit: g.limit.map(|l| l as i64).unwrap_or(0),
            rules: vec![],
        })
        .collect();

    let mut data = HashMap::new();
    data.insert("groups".to_string(), groups);

    Json(ApiResponse::success(data))
}

/// 处理告警查询
pub async fn handle_alerts(
    State(state): State<Arc<ServerState>>,
) -> Json<ApiResponse<HashMap<String, Vec<Alert>>>> {
    let alert_manager = state.alert_manager.read().await;

    let alerts: Vec<Alert> = vec![]; // TODO: 从 alert_manager 获取实际告警

    let mut data = HashMap::new();
    data.insert("alerts".to_string(), alerts);

    Json(ApiResponse::success(data))
}

/// 健康检查
pub async fn handle_healthy() -> &'static str {
    "ChronoDB is Healthy.\n"
}

/// 就绪检查
pub async fn handle_ready() -> &'static str {
    "ChronoDB is Ready.\n"
}

/// 运行时信息
pub async fn handle_runtime_info(
    State(state): State<Arc<ServerState>>,
) -> Json<ApiResponse<RuntimeInfo>> {
    let info = RuntimeInfo {
        start_time: "2024-01-01T00:00:00Z".to_string(),
        cwd: std::env::current_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default(),
        reload_config_success: true,
        last_config_time: None,
        chunk_count: 0,
        time_series_count: 0,
        corruption_count: 0,
        goroutine_count: 0,
        go_max_procs: num_cpus::get() as i64,
        go_version: "rust".to_string(),
        go_arch: std::env::consts::ARCH.to_string(),
        go_os: std::env::consts::OS.to_string(),
    };

    Json(ApiResponse::success(info))
}

/// 构建信息
pub async fn handle_build_info() -> Json<ApiResponse<BuildInfo>> {
    let info = BuildInfo {
        version: env!("CARGO_PKG_VERSION").to_string(),
        revision: "unknown".to_string(),
        branch: "main".to_string(),
        build_user: "chronodb".to_string(),
        build_date: "2024-01-01".to_string(),
        go_version: "rust".to_string(),
    };

    Json(ApiResponse::success(info))
}
