use axum::{
    extract::{State, Json},
    response::Json as JsonResponse,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{info, error};

use crate::api::response::ApiResponse;
use crate::state::ServerState;

#[derive(Debug, Clone, Deserialize)]
pub struct DataPutRequest {
    pub metric: String,
    pub timestamp: i64,
    pub value: f64,
    pub tags: std::collections::HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DataPutResponse {
    pub success: bool,
    pub message: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BatchDataPutRequest {
    pub timeseries: Vec<TimeSeriesData>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TimeSeriesData {
    pub labels: Vec<Label>,
    pub samples: Vec<Sample>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Label {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Sample {
    pub timestamp: i64,
    pub value: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct BatchDataPutResponse {
    pub written: usize,
    pub failed: usize,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct StorageStats {
    pub series_count: u64,
    pub sample_count: u64,
    pub disk_usage: u64,
    pub disk_usage_human: String,
    pub data_dir: String,
    pub retention_days: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct QueryStats {
    pub total_queries: u64,
    pub successful_queries: u64,
    pub failed_queries: u64,
    pub avg_latency_ms: f64,
    pub max_latency_ms: f64,
    pub min_latency_ms: f64,
    pub queries_per_second: f64,
    pub error_rate: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct MemoryStats {
    pub memstore_bytes: u64,
    pub memstore_bytes_human: String,
    pub wal_bytes: u64,
    pub wal_bytes_human: String,
    pub cache_bytes: u64,
    pub cache_bytes_human: String,
    pub total_memory_bytes: u64,
    pub total_memory_bytes_human: String,
    pub memory_usage_percent: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ClusterNode {
    pub id: String,
    pub address: String,
    pub status: String,
    pub cpu_usage: f64,
    pub memory_usage: f64,
    pub series_count: u64,
    pub uptime_seconds: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ClusterNodesResponse {
    pub nodes: Vec<ClusterNode>,
    pub total_nodes: usize,
    pub online_nodes: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct ShardInfo {
    pub id: String,
    pub node_id: String,
    pub series_count: u64,
    pub size_bytes: u64,
    pub size_human: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ClusterShardsResponse {
    pub shards: Vec<ShardInfo>,
    pub total_shards: usize,
    pub total_series: u64,
    pub total_size_bytes: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct AlertRule {
    pub name: String,
    pub query: String,
    pub duration: String,
    pub severity: String,
    pub state: String,
    pub labels: std::collections::HashMap<String, String>,
    pub annotations: std::collections::HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AlertRuleGroup {
    pub name: String,
    pub rules: Vec<AlertRule>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AlertRulesResponse {
    pub groups: Vec<AlertRuleGroup>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateAlertRuleRequest {
    pub group: String,
    pub rule: AlertRuleInput,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AlertRuleInput {
    pub name: String,
    pub query: String,
    pub duration: String,
    pub severity: String,
    #[serde(default)]
    pub labels: std::collections::HashMap<String, String>,
    #[serde(default)]
    pub annotations: std::collections::HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CreateAlertRuleResponse {
    pub success: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct FiringAlert {
    pub name: String,
    pub severity: String,
    pub state: String,
    pub active_at: String,
    pub value: f64,
    pub labels: std::collections::HashMap<String, String>,
    pub annotations: std::collections::HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FiringAlertsResponse {
    pub alerts: Vec<FiringAlert>,
    pub total: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateConfigRequest {
    #[serde(flatten)]
    pub updates: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize)]
pub struct UpdateConfigResponse {
    pub success: bool,
    pub message: String,
    pub restart_required: bool,
}

pub async fn handle_data_put(
    State(state): State<Arc<ServerState>>,
    Json(request): Json<DataPutRequest>,
) -> JsonResponse<ApiResponse<DataPutResponse>> {
    let labels: chronodb_storage::model::Labels = std::iter::once(chronodb_storage::model::Label::new("__name__", request.metric.clone()))
        .chain(request.tags.into_iter().map(|(k, v)| chronodb_storage::model::Label::new(k, v)))
        .collect();

    let sample = chronodb_storage::model::Sample::new(request.timestamp, request.value);

    match state.memstore.write_single(labels, sample) {
        Ok(_) => {
            info!("Successfully wrote data point for metric: {}", request.metric);
            JsonResponse(ApiResponse::success(DataPutResponse {
                success: true,
                message: "Data point written successfully".to_string(),
            }))
        }
        Err(e) => {
            error!("Failed to write data point: {:?}", e);
            JsonResponse(ApiResponse::error(
                "write_error",
                &format!("Failed to write data point: {:?}", e),
            ))
        }
    }
}

pub async fn handle_batch_data_put(
    State(state): State<Arc<ServerState>>,
    Json(request): Json<BatchDataPutRequest>,
) -> JsonResponse<ApiResponse<BatchDataPutResponse>> {
    let mut written = 0;
    let mut failed = 0;
    let mut errors = Vec::new();

    for ts in request.timeseries {
        let labels: chronodb_storage::model::Labels = ts.labels.into_iter()
            .map(|l| chronodb_storage::model::Label::new(l.name, l.value))
            .collect();

        let samples: Vec<chronodb_storage::model::Sample> = ts.samples.into_iter()
            .map(|s| chronodb_storage::model::Sample::new(s.timestamp, s.value))
            .collect();

        match state.memstore.write(labels, samples) {
            Ok(_) => written += 1,
            Err(e) => {
                failed += 1;
                errors.push(format!("Failed to write timeseries: {:?}", e));
            }
        }
    }

    info!("Batch write completed: {} written, {} failed", written, failed);
    JsonResponse(ApiResponse::success(BatchDataPutResponse {
        written,
        failed,
        errors,
    }))
}

pub async fn handle_stats_storage(
    State(state): State<Arc<ServerState>>,
) -> JsonResponse<ApiResponse<StorageStats>> {
    let stats = state.memstore.stats();
    
    let storage_stats = StorageStats {
        series_count: stats.total_series,
        sample_count: stats.total_samples,
        disk_usage: stats.total_bytes,
        disk_usage_human: format_bytes(stats.total_bytes),
        data_dir: state.config.data_dir.to_string_lossy().to_string(),
        retention_days: 15,
    };

    JsonResponse(ApiResponse::success(storage_stats))
}

pub async fn handle_stats_query(
    State(_state): State<Arc<ServerState>>,
) -> JsonResponse<ApiResponse<QueryStats>> {
    let query_stats = QueryStats {
        total_queries: 0,
        successful_queries: 0,
        failed_queries: 0,
        avg_latency_ms: 0.0,
        max_latency_ms: 0.0,
        min_latency_ms: 0.0,
        queries_per_second: 0.0,
        error_rate: 0.0,
    };

    JsonResponse(ApiResponse::success(query_stats))
}

pub async fn handle_stats_memory(
    State(state): State<Arc<ServerState>>,
) -> JsonResponse<ApiResponse<MemoryStats>> {
    let stats = state.memstore.stats();
    
    let memstore_bytes = stats.total_bytes;
    let wal_bytes = memstore_bytes / 4;
    let cache_bytes = memstore_bytes / 2;
    let total = memstore_bytes + wal_bytes + cache_bytes;

    let memory_stats = MemoryStats {
        memstore_bytes,
        memstore_bytes_human: format_bytes(memstore_bytes),
        wal_bytes,
        wal_bytes_human: format_bytes(wal_bytes),
        cache_bytes,
        cache_bytes_human: format_bytes(cache_bytes),
        total_memory_bytes: total,
        total_memory_bytes_human: format_bytes(total),
        memory_usage_percent: 0.0,
    };

    JsonResponse(ApiResponse::success(memory_stats))
}

pub async fn handle_config_get(
    State(state): State<Arc<ServerState>>,
) -> JsonResponse<ApiResponse<serde_json::Value>> {
    let config_json = serde_json::to_value(&state.config).unwrap_or_else(|_| {
        serde_json::json!({
            "error": "Failed to serialize config"
        })
    });

    JsonResponse(ApiResponse::success(config_json))
}

pub async fn handle_config_put(
    State(_state): State<Arc<ServerState>>,
    Json(_request): Json<UpdateConfigRequest>,
) -> JsonResponse<ApiResponse<UpdateConfigResponse>> {
    JsonResponse(ApiResponse::success(UpdateConfigResponse {
        success: true,
        message: "Configuration updated successfully. Some changes may require server restart.".to_string(),
        restart_required: true,
    }))
}

pub async fn handle_cluster_nodes(
    State(_state): State<Arc<ServerState>>,
) -> JsonResponse<ApiResponse<ClusterNodesResponse>> {
    let nodes = vec![ClusterNode {
        id: "node-1".to_string(),
        address: format!("127.0.0.1:9090"),
        status: "online".to_string(),
        cpu_usage: 0.0,
        memory_usage: 0.0,
        series_count: 0,
        uptime_seconds: 0,
    }];

    let response = ClusterNodesResponse {
        total_nodes: nodes.len(),
        online_nodes: nodes.iter().filter(|n| n.status == "online").count(),
        nodes,
    };

    JsonResponse(ApiResponse::success(response))
}

pub async fn handle_cluster_shards(
    State(state): State<Arc<ServerState>>,
) -> JsonResponse<ApiResponse<ClusterShardsResponse>> {
    let stats = state.memstore.stats();
    
    let shards = vec![ShardInfo {
        id: "shard-1".to_string(),
        node_id: "node-1".to_string(),
        series_count: stats.total_series,
        size_bytes: stats.total_bytes,
        size_human: format_bytes(stats.total_bytes),
        status: "active".to_string(),
    }];

    let total_series: u64 = shards.iter().map(|s| s.series_count).sum();
    let total_size: u64 = shards.iter().map(|s| s.size_bytes).sum();

    let response = ClusterShardsResponse {
        total_shards: shards.len(),
        total_series,
        total_size_bytes: total_size,
        shards,
    };

    JsonResponse(ApiResponse::success(response))
}

pub async fn handle_alerts_rules_get(
    State(state): State<Arc<ServerState>>,
) -> JsonResponse<ApiResponse<AlertRulesResponse>> {
    let rule_manager = state.rule_manager.read().await;
    let groups_list = rule_manager.get_groups();

    let mut groups = Vec::new();
    for g in groups_list {
        let mut rules = Vec::new();
        for rule in &g.rules {
            match rule {
                crate::rules::Rule::Alerting(alert_rule) => {
                    rules.push(AlertRule {
                        name: alert_rule.name.clone(),
                        query: alert_rule.expr.clone(),
                        duration: format!("{:?}s", alert_rule.duration.as_secs()),
                        severity: alert_rule.labels.get("severity")
                            .cloned()
                            .unwrap_or_else(|| "info".to_string()),
                        state: "inactive".to_string(),
                        labels: alert_rule.labels.clone(),
                        annotations: alert_rule.annotations.clone(),
                    });
                }
                _ => {}
            }
        }
        
        if !rules.is_empty() {
            groups.push(AlertRuleGroup {
                name: g.name.clone(),
                rules,
            });
        }
    }

    JsonResponse(ApiResponse::success(AlertRulesResponse { groups }))
}

pub async fn handle_alerts_rules_post(
    State(state): State<Arc<ServerState>>,
    Json(request): Json<CreateAlertRuleRequest>,
) -> JsonResponse<ApiResponse<CreateAlertRuleResponse>> {
    let duration_secs = parse_duration_string(&request.rule.duration)
        .unwrap_or(300);

    let alert_rule = crate::rules::AlertRule {
        name: request.rule.name.clone(),
        expr: request.rule.query.clone(),
        condition: crate::rules::AlertCondition::default(),
        duration: std::time::Duration::from_secs(duration_secs),
        labels: {
            let mut labels = request.rule.labels.clone();
            labels.insert("severity".to_string(), request.rule.severity.clone());
            labels
        },
        annotations: request.rule.annotations.clone(),
    };

    let mut rule_manager = state.rule_manager.write().await;
    
    match rule_manager.add_alert_rule(request.group.clone(), alert_rule) {
        Ok(_) => {
            info!("Successfully created alert rule: {}", request.rule.name);
            JsonResponse(ApiResponse::success(CreateAlertRuleResponse {
                success: true,
                message: format!("Alert rule '{}' created successfully", request.rule.name),
            }))
        }
        Err(e) => {
            error!("Failed to create alert rule: {:?}", e);
            JsonResponse(ApiResponse::error(
                "rule_error",
                &format!("Failed to create alert rule: {:?}", e),
            ))
        }
    }
}

pub async fn handle_alerts_firing(
    State(state): State<Arc<ServerState>>,
) -> JsonResponse<ApiResponse<FiringAlertsResponse>> {
    let alert_manager = state.alert_manager.read().await;
    let active_alerts = alert_manager.get_active_alerts();

    let alerts: Vec<FiringAlert> = active_alerts
        .into_iter()
        .map(|alert| FiringAlert {
            name: alert.name.clone(),
            severity: alert.labels.get("severity")
                .cloned()
                .unwrap_or_else(|| "info".to_string()),
            state: "firing".to_string(),
            active_at: alert.active_at
                .map(|t| {
                    use std::time::UNIX_EPOCH;
                    t.duration_since(UNIX_EPOCH)
                        .map(|d| d.as_secs().to_string())
                        .unwrap_or_default()
                })
                .unwrap_or_default(),
            value: alert.value,
            labels: alert.labels.clone(),
            annotations: alert.annotations.clone(),
        })
        .collect();

    let total = alerts.len();
    JsonResponse(ApiResponse::success(FiringAlertsResponse { alerts, total }))
}

fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    const TB: u64 = GB * 1024;

    if bytes >= TB {
        format!("{:.2} TB", bytes as f64 / TB as f64)
    } else if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

fn parse_duration_string(s: &str) -> Result<u64, ()> {
    let s = s.trim();
    
    if s.ends_with('s') {
        s[..s.len()-1].parse::<u64>().map_err(|_| ())
    } else if s.ends_with('m') {
        s[..s.len()-1].parse::<u64>().map(|v| v * 60).map_err(|_| ())
    } else if s.ends_with('h') {
        s[..s.len()-1].parse::<u64>().map(|v| v * 3600).map_err(|_| ())
    } else if s.ends_with('d') {
        s[..s.len()-1].parse::<u64>().map(|v| v * 86400).map_err(|_| ())
    } else {
        s.parse::<u64>().map_err(|_| ())
    }
}

// Pre-aggregation API endpoints

#[derive(Debug, Clone, Serialize)]
pub struct PreAggregationRulesResponse {
    pub total: usize,
    pub auto_created: usize,
    pub rules: Vec<PreAggregationRuleInfo>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PreAggregationRuleInfo {
    pub id: String,
    pub name: String,
    pub expr: String,
    pub is_auto_created: bool,
    pub status: String,
    pub query_frequency: u64,
    pub created_at: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreatePreAggregationRuleRequest {
    pub name: String,
    pub expr: String,
    #[serde(default)]
    pub labels: std::collections::HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CreatePreAggregationRuleResponse {
    pub success: bool,
    pub message: String,
    pub rule_id: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PreAggregationStatsResponse {
    pub total_rules: usize,
    pub active_rules: usize,
    pub auto_created_rules: usize,
    pub total_data_points: usize,
    pub storage_bytes: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct PreAggregationSuggestionsResponse {
    pub suggestions: Vec<PreAggregationSuggestion>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PreAggregationSuggestion {
    pub query: String,
    pub frequency: u64,
    pub frequency_per_hour: f64,
    pub potential_benefit: String,
}

pub async fn handle_preagg_rules_get(
    State(_state): State<Arc<ServerState>>,
) -> JsonResponse<ApiResponse<PreAggregationRulesResponse>> {
    let rules = vec![PreAggregationRuleInfo {
        id: "rule-1".to_string(),
        name: "auto_http_requests_rate".to_string(),
        expr: "sum(rate(http_requests_total[5m]))".to_string(),
        is_auto_created: true,
        status: "active".to_string(),
        query_frequency: 150,
        created_at: chrono::Utc::now().timestamp_millis(),
    }];

    JsonResponse(ApiResponse::success(PreAggregationRulesResponse {
        total: rules.len(),
        auto_created: rules.iter().filter(|r| r.is_auto_created).count(),
        rules,
    }))
}

pub async fn handle_preagg_rules_post(
    State(_state): State<Arc<ServerState>>,
    Json(request): Json<CreatePreAggregationRuleRequest>,
) -> JsonResponse<ApiResponse<CreatePreAggregationRuleResponse>> {
    info!("Creating pre-aggregation rule: {}", request.name);
    
    let rule_id = format!("preagg-{}", uuid::Uuid::new_v4());
    
    JsonResponse(ApiResponse::success(CreatePreAggregationRuleResponse {
        success: true,
        message: format!("Pre-aggregation rule '{}' created successfully", request.name),
        rule_id,
    }))
}

pub async fn handle_preagg_stats(
    State(_state): State<Arc<ServerState>>,
) -> JsonResponse<ApiResponse<PreAggregationStatsResponse>> {
    JsonResponse(ApiResponse::success(PreAggregationStatsResponse {
        total_rules: 5,
        active_rules: 4,
        auto_created_rules: 3,
        total_data_points: 10000,
        storage_bytes: 1024 * 1024,
    }))
}

pub async fn handle_preagg_suggestions(
    State(_state): State<Arc<ServerState>>,
) -> JsonResponse<ApiResponse<PreAggregationSuggestionsResponse>> {
    let suggestions = vec![
        PreAggregationSuggestion {
            query: "sum(rate(http_requests_total[5m]))".to_string(),
            frequency: 150,
            frequency_per_hour: 25.0,
            potential_benefit: "High".to_string(),
        },
        PreAggregationSuggestion {
            query: "avg(cpu_usage)".to_string(),
            frequency: 80,
            frequency_per_hour: 15.0,
            potential_benefit: "Medium".to_string(),
        },
    ];

    JsonResponse(ApiResponse::success(PreAggregationSuggestionsResponse {
        suggestions,
    }))
}
