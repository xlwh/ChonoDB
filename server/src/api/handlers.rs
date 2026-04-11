use axum::{
    extract::{Query, State, Path},
    response::Json,
    body::Bytes,
};
use chrono;
use parse_duration::parse;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{error, warn};

use crate::api::response::*;
use crate::state::ServerState;

/// 处理即时查询（GET方法）
pub async fn handle_query_get(
    State(state): State<Arc<ServerState>>,
    Query(params): Query<HashMap<String, String>>,
) -> Json<ApiResponse<QueryResult>> {
    handle_query_internal(state, params).await
}

/// 处理即时查询（POST方法）
pub async fn handle_query_post(
    State(state): State<Arc<ServerState>>,
    Query(query_params): Query<HashMap<String, String>>,
    body: Bytes,
) -> Json<ApiResponse<QueryResult>> {
    // 尝试从请求体中解析参数
    let mut params = if let Ok(body_str) = std::str::from_utf8(&body) {
        serde_urlencoded::from_str::<HashMap<String, String>>(body_str).unwrap_or_default()
    } else {
        HashMap::new()
    };
    
    // 如果请求体中没有参数，尝试从URL查询字符串中获取
    if params.is_empty() {
        params = query_params;
    }
    
    handle_query_internal(state, params).await
}

/// 处理即时查询的内部逻辑
async fn handle_query_internal(
    state: Arc<ServerState>,
    params: HashMap<String, String>,
) -> Json<ApiResponse<QueryResult>> {
    // 验证参数
    let query = match params.get("query") {
        Some(q) if !q.trim().is_empty() => q,
        Some(_) => {
            return Json(ApiResponse::error(
                "bad_data",
                "Parameter 'query' cannot be empty",
            ));
        }
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

    // 解析超时参数
    let timeout = params.get("timeout")
        .and_then(|t| parse_duration::parse(t).ok())
        .unwrap_or(std::time::Duration::from_secs(10));

    // 检查是否包含聚合函数
    let mut aggregation_function = None;
    let mut processed_query = query.trim();
    
    if processed_query.starts_with("sum(") && processed_query.ends_with(")") {
        aggregation_function = Some("sum");
        processed_query = &processed_query[4..processed_query.len()-1].trim();
    } else if processed_query.starts_with("avg(") && processed_query.ends_with(")") {
        aggregation_function = Some("avg");
        processed_query = &processed_query[4..processed_query.len()-1].trim();
    } else if processed_query.starts_with("max(") && processed_query.ends_with(")") {
        aggregation_function = Some("max");
        processed_query = &processed_query[4..processed_query.len()-1].trim();
    } else if processed_query.starts_with("min(") && processed_query.ends_with(")") {
        aggregation_function = Some("min");
        processed_query = &processed_query[4..processed_query.len()-1].trim();
    }

    // 简单的标签匹配解析（实际项目中需要更复杂的PromQL解析）
    let label_matchers = parse_label_matchers(processed_query);

    // 验证时间范围
    let start_time = time - 3600000; // 过去1小时
    if start_time < 0 {
        return Json(ApiResponse::error(
            "bad_data",
            "Invalid time range: start time cannot be negative",
        ));
    }

    // 使用内存存储查询数据
    let result = match state.memstore.query(&label_matchers, start_time, time) {
        Ok(series) => {
            // 预分配容量，减少内存分配
            let mut instant_vectors = Vec::with_capacity(series.len());
            
            for ts in series {
                // 构建 metric 一次，避免重复克隆
                let mut metric = serde_json::Map::with_capacity(ts.labels.len());
                for label in &ts.labels {
                    metric.insert(label.name.clone(), serde_json::Value::String(label.value.clone()));
                }
                
                // 处理样本
                for s in ts.samples {
                    instant_vectors.push(InstantVector {
                        metric: metric.clone(),
                        value: (s.timestamp as f64 / 1000.0, s.value.to_string()),
                    });
                }
            }
            
            // 处理聚合函数
            if let Some(agg_func) = aggregation_function {
                // 按时间戳分组聚合
                let mut timestamp_values = std::collections::HashMap::new();
                
                for vector in &instant_vectors {
                    let timestamp = vector.value.0;
                    let timestamp_key = (timestamp * 1000.0) as i64; // 转换为整数作为键
                    let value: f64 = vector.value.1.parse().unwrap_or(0.0);
                    
                    timestamp_values.entry(timestamp_key).or_insert(Vec::new()).push(value);
                }
                
                // 计算聚合结果
                let mut aggregated_vectors = Vec::new();
                
                for (timestamp_key, values) in timestamp_values {
                    let timestamp = timestamp_key as f64 / 1000.0; // 转换回原始时间戳
                    let aggregated_value = match agg_func {
                        "sum" => values.iter().sum::<f64>(),
                        "avg" => values.iter().sum::<f64>() / values.len() as f64,
                        "max" => *values.iter().max_by(|a, b| a.partial_cmp(b).unwrap()).unwrap(),
                        "min" => *values.iter().min_by(|a, b| a.partial_cmp(b).unwrap()).unwrap(),
                        _ => 0.0,
                    };
                    
                    aggregated_vectors.push(InstantVector {
                        metric: serde_json::Map::new(), // 聚合结果没有标签
                        value: (timestamp, aggregated_value.to_string()),
                    });
                }
                
                QueryResult::Vector(aggregated_vectors)
            } else {
                QueryResult::Vector(instant_vectors)
            }
        },
        Err(e) => {
            error!("Query execution failed: {:?}", e);
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
    
    // 处理聚合函数
    let query = query.trim();
    let mut processed_query = query;
    
    // 检查是否包含聚合函数
    if query.contains('(') && query.contains(')') {
        // 简单提取括号内的内容
        if let Some(start) = query.find('(') {
            if let Some(end) = query.rfind(')') {
                processed_query = query[start+1..end].trim();
            }
        }
    }
    
    // 提取指标名
    let metric_name = if let Some(start) = processed_query.find('{') {
        processed_query[..start].trim()
    } else {
        processed_query.trim()
    };
    
    // 添加 __name__ 标签
    if !metric_name.is_empty() {
        matchers.push(("__name__".to_string(), metric_name.to_string()));
    }
    
    // 简单解析标签匹配
    if let Some(start) = processed_query.find('{') {
        if let Some(end) = processed_query.find('}') {
            let labels_str = &processed_query[start+1..end];
            for label in labels_str.split(',') {
                let parts: Vec<&str> = label.split('=').collect();
                if parts.len() == 2 {
                    let name = parts[0].trim().to_string();
                    let value = parts[1].trim().trim_matches('"').trim_matches('\'').to_string();
                    matchers.push((name, value));
                }
            }
        }
    }
    
    matchers
}

/// 处理范围查询（GET方法）
pub async fn handle_query_range_get(
    State(state): State<Arc<ServerState>>,
    Query(params): Query<HashMap<String, String>>,
) -> Json<ApiResponse<QueryResult>> {
    handle_query_range_internal(state, params).await
}

/// 处理范围查询（POST方法）
pub async fn handle_query_range_post(
    State(state): State<Arc<ServerState>>,
    Query(query_params): Query<HashMap<String, String>>,
    body: Bytes,
) -> Json<ApiResponse<QueryResult>> {
    // 尝试从请求体中解析参数
    let mut params = if let Ok(body_str) = std::str::from_utf8(&body) {
        serde_urlencoded::from_str::<HashMap<String, String>>(body_str).unwrap_or_default()
    } else {
        HashMap::new()
    };
    
    // 如果请求体中没有参数，尝试从URL查询字符串中获取
    if params.is_empty() {
        params = query_params;
    }
    
    handle_query_range_internal(state, params).await
}

/// 处理范围查询的内部逻辑
async fn handle_query_range_internal(
    state: Arc<ServerState>,
    params: HashMap<String, String>,
) -> Json<ApiResponse<QueryResult>> {
    // 验证查询参数
    let query = match params.get("query") {
        Some(q) if !q.trim().is_empty() => q,
        Some(_) => {
            return Json(ApiResponse::error(
                "bad_data",
                "Parameter 'query' cannot be empty",
            ));
        }
        None => {
            return Json(ApiResponse::error(
                "bad_data",
                "Parameter 'query' is required",
            ));
        }
    };

    // 解析时间参数
    let start = match params.get("start") {
        Some(s) => s.parse::<i64>().ok(),
        None => None,
    };

    let end = match params.get("end") {
        Some(e) => e.parse::<i64>().ok(),
        None => None,
    };

    let step = match params.get("step") {
        Some(s) => {
            // 尝试解析为数字（秒）
            if let Ok(step_seconds) = s.parse::<f64>() {
                Some((step_seconds * 1000.0) as i64)
            } else {
                // 尝试解析为时间间隔字符串（如 "15s"）
                parse_duration::parse(s).ok().map(|d| d.as_millis() as i64)
            }
        },
        None => None,
    };

    let (start, end, step) = match (start, end, step) {
        (Some(s), Some(e), Some(st)) => (s * 1000, e * 1000, st),
        _ => {
            return Json(ApiResponse::error(
                "bad_data",
                "Parameters 'start', 'end' and 'step' are required",
            ));
        }
    };

    // 验证时间参数有效性
    if start < 0 {
        return Json(ApiResponse::error(
            "bad_data",
            "Invalid 'start' parameter: cannot be negative",
        ));
    }

    if end < 0 {
        return Json(ApiResponse::error(
            "bad_data",
            "Invalid 'end' parameter: cannot be negative",
        ));
    }

    if start > end {
        return Json(ApiResponse::error(
            "bad_data",
            "Invalid time range: 'start' cannot be greater than 'end'",
        ));
    }

    if step <= 0 {
        return Json(ApiResponse::error(
            "bad_data",
            "Invalid 'step' parameter: must be positive",
        ));
    }

    // 验证时间范围大小
    let max_time_range = 30 * 24 * 60 * 60 * 1000; // 30 days in milliseconds
    if end - start > max_time_range {
        return Json(ApiResponse::error(
            "bad_data",
            &format!("Time range too large: maximum allowed is {} days", max_time_range / (24 * 60 * 60 * 1000)),
        ));
    }

    // 解析超时参数
    let timeout = params.get("timeout")
        .and_then(|t| parse_duration::parse(t).ok())
        .unwrap_or(std::time::Duration::from_secs(10));

    // 解析标签匹配
    let label_matchers = parse_label_matchers(query);

    // 使用内存存储查询数据
    let result = match state.memstore.query(&label_matchers, start, end) {
        Ok(series) => {
            // 预分配容量，减少内存分配
            let mut range_vectors = Vec::with_capacity(series.len());
            
            for ts in series {
                // 预分配 values 容量
                let mut values = Vec::with_capacity(ts.samples.len());
                for s in ts.samples {
                    values.push((s.timestamp as f64 / 1000.0, format!("{:.6}", s.value)));
                }
                
                // 预分配 metric 容量
                let mut metric = serde_json::Map::with_capacity(ts.labels.len());
                for label in ts.labels {
                    metric.insert(label.name, serde_json::Value::String(label.value));
                }
                
                range_vectors.push(RangeVector {
                    metric,
                    values,
                });
            }
            
            QueryResult::Matrix(range_vectors)
        },
        Err(e) => {
            error!("Query execution failed: {:?}", e);
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
        .map(|m| m.split(',').filter(|s| !s.trim().is_empty()).collect())
        .unwrap_or_default();

    if matchers.is_empty() {
        return Json(ApiResponse::error(
            "bad_data",
            "Parameter 'match[]' is required and cannot be empty",
        ));
    }

    // 验证match参数数量
    const MAX_MATCHERS: usize = 100;
    if matchers.len() > MAX_MATCHERS {
        return Json(ApiResponse::error(
            "bad_data",
            &format!("Too many matchers: maximum allowed is {}", MAX_MATCHERS),
        ));
    }

    // 解析时间参数
    let start = params.get("start")
        .and_then(|s| s.parse::<f64>().ok())
        .map(|t| (t * 1000.0) as i64);

    let end = params.get("end")
        .and_then(|e| e.parse::<f64>().ok())
        .map(|t| (t * 1000.0) as i64);

    let (mut start, mut end) = (start.unwrap_or(0), end.unwrap_or(chrono::Utc::now().timestamp_millis()));

    // 验证时间参数有效性
    if start < 0 {
        start = 0;
        warn!("Invalid 'start' parameter: using 0 instead of negative value");
    }

    if end < 0 {
        return Json(ApiResponse::error(
            "bad_data",
            "Invalid 'end' parameter: cannot be negative",
        ));
    }

    if start > end {
        return Json(ApiResponse::error(
            "bad_data",
            "Invalid time range: 'start' cannot be greater than 'end'",
        ));
    }

    // 验证时间范围大小 - 移除限制，与Prometheus行为一致
    // let max_time_range = 365 * 24 * 60 * 60 * 1000; // 1 year in milliseconds
    // if end - start > max_time_range {
    //     return Json(ApiResponse::error(
    //         "bad_data",
    //         &format!("Time range too large: maximum allowed is {} days", max_time_range / (24 * 60 * 60 * 1000)),
    //     ));
    // }

    // 处理每个match表达式
    let mut all_series = Vec::new();
    for matcher in matchers {
        if matcher.trim().is_empty() {
            continue;
        }
        
        let label_matchers = parse_label_matchers(matcher);
        
        match state.memstore.query(&label_matchers, start, end) {
            Ok(series) => {
                // 预分配容量，减少内存分配
                let mut api_series = Vec::with_capacity(series.len());
                
                for ts in series {
                    // 预分配 labels 容量
                    let mut labels = serde_json::Map::with_capacity(ts.labels.len());
                    for label in ts.labels {
                        labels.insert(label.name, serde_json::Value::String(label.value));
                    }
                    
                    api_series.push(Series {
                        labels,
                    });
                }
                
                all_series.extend(api_series);
            },
            Err(e) => {
                warn!("Error querying for matcher '{}': {:?}", matcher, e);
                // 继续处理其他匹配器，而不是立即返回错误
            }
        }
    }

    Json(ApiResponse::success(all_series))
}

/// 处理标签查询
pub async fn handle_labels(
    State(state): State<Arc<ServerState>>,
    Query(params): Query<HashMap<String, String>>,
) -> Json<ApiResponse<Vec<String>>> {
    // 解析时间参数
    let start = params.get("start")
        .and_then(|s| s.parse::<f64>().ok())
        .map(|t| (t * 1000.0) as i64);

    let end = params.get("end")
        .and_then(|e| e.parse::<f64>().ok())
        .map(|t| (t * 1000.0) as i64);

    let (mut start, mut end) = (start.unwrap_or(0), end.unwrap_or(chrono::Utc::now().timestamp_millis()));

    // 验证时间参数有效性
    if start < 0 {
        start = 0;
        warn!("Invalid 'start' parameter: using 0 instead of negative value");
    }

    if end < 0 {
        return Json(ApiResponse::error(
            "bad_data",
            "Invalid 'end' parameter: cannot be negative",
        ));
    }

    if start > end {
        return Json(ApiResponse::error(
            "bad_data",
            "Invalid time range: 'start' cannot be greater than 'end'",
        ));
    }

    // 使用内存存储获取标签名（简化实现）
    let labels: Vec<String> = vec![];
    Json(ApiResponse::success(labels))
}

/// 处理标签值查询
pub async fn handle_label_values(
    State(state): State<Arc<ServerState>>,
    Path(name): Path<String>,
    Query(params): Query<HashMap<String, String>>,
) -> Json<ApiResponse<Vec<String>>> {
    // 验证标签名
    if name.trim().is_empty() {
        return Json(ApiResponse::error(
            "bad_data",
            "Label name cannot be empty",
        ));
    }

    // 解析时间参数
    let start = params.get("start")
        .and_then(|s| s.parse::<f64>().ok())
        .map(|t| (t * 1000.0) as i64);

    let end = params.get("end")
        .and_then(|e| e.parse::<f64>().ok())
        .map(|t| (t * 1000.0) as i64);

    let (mut start, mut end) = (start.unwrap_or(0), end.unwrap_or(chrono::Utc::now().timestamp_millis()));

    // 验证时间参数有效性
    if start < 0 {
        start = 0;
        warn!("Invalid 'start' parameter: using 0 instead of negative value");
    }

    if end < 0 {
        return Json(ApiResponse::error(
            "bad_data",
            "Invalid 'end' parameter: cannot be negative",
        ));
    }

    if start > end {
        return Json(ApiResponse::error(
            "bad_data",
            "Invalid time range: 'start' cannot be greater than 'end'",
        ));
    }

    // 解析match参数
    let matchers: Vec<&str> = params.get("match[]")
        .map(|m| m.split(',').filter(|s| !s.trim().is_empty()).collect())
        .unwrap_or_default();

    // 验证match参数数量
    const MAX_MATCHERS: usize = 100;
    if matchers.len() > MAX_MATCHERS {
        return Json(ApiResponse::error(
            "bad_data",
            &format!("Too many matchers: maximum allowed is {}", MAX_MATCHERS),
        ));
    }

    // 使用内存存储获取标签值（简化实现）
    let values: Vec<String> = vec![];
    Json(ApiResponse::success(values))
}

/// 处理元数据查询
pub async fn handle_metadata(
    State(state): State<Arc<ServerState>>,
    Query(params): Query<HashMap<String, String>>,
) -> Json<ApiResponse<serde_json::Value>> {
    let metric = params.get("metric");
    let limit = params.get("limit")
        .and_then(|l| l.parse::<usize>().ok())
        .unwrap_or(10000);

    // 构建元数据响应
    let mut metadata = serde_json::Map::new();
    
    // 这里只是一个简单的实现，实际项目中需要从存储中获取元数据
    if let Some(metric_name) = metric {
        let mut metric_metadata = serde_json::Map::new();
        let mut samples = vec![];
        
        let sample = serde_json::json!({
            "type": "counter",
            "help": format!("Help text for {}", metric_name),
            "unit": ""
        });
        samples.push(sample);
        metric_metadata.insert("samples".to_string(), serde_json::Value::Array(samples.into_iter().collect()));
        metadata.insert(metric_name.clone(), serde_json::Value::Object(metric_metadata));
    } else {
        // 返回所有指标的元数据（简化实现）
        let labels: Vec<String> = vec![];

        
        for label in labels {
            if label == "__name__" {
                continue;
            }
            let mut metric_metadata = serde_json::Map::new();
            let mut samples = vec![];
            
            let sample = serde_json::json!({
                "type": "gauge",
                "help": format!("Help text for {}", label),
                "unit": ""
            });
            samples.push(sample);
            metric_metadata.insert("samples".to_string(), serde_json::Value::Array(samples.into_iter().collect()));
            metadata.insert(label, serde_json::Value::Object(metric_metadata));
        }
    }

    Json(ApiResponse::success(serde_json::Value::Object(metadata)))
}

/// 处理目标查询
pub async fn handle_targets(
    State(state): State<Arc<ServerState>>,
) -> Json<ApiResponse<HashMap<String, Vec<Target>>>> {
    let target_manager = state.target_manager.read().await;
    let all_targets = target_manager.get_all_targets();
    
    // 预分配容量，减少内存分配
    let mut targets = Vec::with_capacity(all_targets.len());
    
    for t in all_targets {
        targets.push(Target {
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
        });
    }

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
    let groups_list = rule_manager.get_groups();
    
    // 预分配容量，减少内存分配
    let mut groups = Vec::with_capacity(groups_list.len());
    
    for g in groups_list {
        // 转换规则
        let mut rules = Vec::with_capacity(g.rules.len());
        for rule in &g.rules {
            match rule {
                crate::rules::Rule::Recording(recording_rule) => {
                    // 转换记录规则
                    let mut labels = serde_json::Map::new();
                    for (key, value) in &recording_rule.labels {
                        labels.insert(key.clone(), serde_json::Value::String(value.clone()));
                    }
                    
                    rules.push(Rule::Recording {
                        name: recording_rule.name.clone(),
                        query: recording_rule.expr.clone(),
                        labels,
                        health: "ok".to_string(),
                        last_error: None,
                        evaluation_time: Some(0.0),
                        last_evaluation: Some(chrono::Utc::now().to_rfc3339()),
                    });
                },
                crate::rules::Rule::Alerting(alert_rule) => {
                    // 转换告警规则
                    let mut labels = serde_json::Map::new();
                    for (key, value) in &alert_rule.labels {
                        labels.insert(key.clone(), serde_json::Value::String(value.clone()));
                    }
                    
                    let mut annotations = serde_json::Map::new();
                    for (key, value) in &alert_rule.annotations {
                        annotations.insert(key.clone(), serde_json::Value::String(value.clone()));
                    }
                    
                    rules.push(Rule::Alerting {
                        name: alert_rule.name.clone(),
                        query: alert_rule.expr.clone(),
                        duration: alert_rule.duration.as_secs_f64(),
                        labels,
                        annotations,
                        alerts: vec![],
                        health: "ok".to_string(),
                        last_error: None,
                        evaluation_time: Some(0.0),
                        last_evaluation: Some(chrono::Utc::now().to_rfc3339()),
                    });
                },
            }
        }
        
        groups.push(RuleGroup {
            name: g.name.clone(),
            file: "rules.yml".to_string(),
            interval: g.interval.map(|d| d.as_secs_f64()).unwrap_or(60.0),
            limit: g.limit.map(|l| l as i64).unwrap_or(0),
            rules,
        });
    }

    let mut data = HashMap::new();
    data.insert("groups".to_string(), groups);

    Json(ApiResponse::success(data))
}

/// 处理告警查询
pub async fn handle_alerts(
    State(state): State<Arc<ServerState>>,
) -> Json<ApiResponse<HashMap<String, Vec<Alert>>>> {
    let _alert_manager = state.alert_manager.read().await;

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
    State(_state): State<Arc<ServerState>>,
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
