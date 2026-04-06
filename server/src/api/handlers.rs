use axum::{
    extract::{Query, State},
    response::Json,
};
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

    // TODO: 实现实际查询逻辑
    let result = QueryResult::Vector(vec![]);
    Json(ApiResponse::success(result))
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

    // TODO: 实现实际范围查询逻辑
    let result = QueryResult::Matrix(vec![]);
    Json(ApiResponse::success(result))
}

/// 处理系列查询
pub async fn handle_series(
    State(state): State<Arc<ServerState>>,
    Query(params): Query<HashMap<String, String>>,
) -> Json<ApiResponse<Vec<Series>>> {
    // TODO: 实现实际系列查询逻辑
    let series: Vec<Series> = vec![];
    Json(ApiResponse::success(series))
}

/// 处理标签查询
pub async fn handle_labels(
    State(state): State<Arc<ServerState>>,
    Query(params): Query<HashMap<String, String>>,
) -> Json<ApiResponse<Vec<String>>> {
    // TODO: 实现实际标签查询逻辑
    let labels: Vec<String> = vec!["__name__".to_string()];
    Json(ApiResponse::success(labels))
}

/// 处理标签值查询
pub async fn handle_label_values(
    State(state): State<Arc<ServerState>>,
    axum::extract::Path(name): axum::extract::Path<String>,
    Query(params): Query<HashMap<String, String>>,
) -> Json<ApiResponse<Vec<String>>> {
    // TODO: 实现实际标签值查询逻辑
    let values: Vec<String> = vec![];
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
