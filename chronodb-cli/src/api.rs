use axum::{extract::{Query, Path, State}, http::StatusCode, response::IntoResponse, routing::{get, post}, Json, Router};
use chronodb_storage::model::{Label, Sample, TimeSeries};
use chronodb_storage::query::{QueryEngine, QueryResult};
use chronodb_storage::memstore::MemStore;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize)]
pub struct QueryParams {
    query: String,
    time: Option<i64>,
    start: Option<i64>,
    end: Option<i64>,
    step: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SeriesParams {
    #[serde(flatten)]
    matchers: HashMap<String, String>,
    start: Option<i64>,
    end: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LabelsParams {
    #[serde(flatten)]
    matchers: HashMap<String, String>,
    start: Option<i64>,
    end: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LabelValuesParams {
    start: Option<i64>,
    end: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PrometheusResponse {
    status: String,
    data: Option<PrometheusData>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PrometheusData {
    resultType: String,
    result: Vec<PrometheusResult>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PrometheusResult {
    metric: serde_json::Value,
    value: Option<(f64, f64)>,
    values: Option<Vec<(f64, f64)>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WriteRequest {
    timeseries: Vec<TimeSeriesWrite>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TimeSeriesWrite {
    labels: Vec<Label>,
    samples: Vec<Sample>,
}

#[derive(Clone)]
pub struct ApiState {
    store: Arc<MemStore>,
    engine: QueryEngine,
}

pub fn create_router(store: Arc<MemStore>) -> Router {
    let engine = QueryEngine::new(store.clone());
    let state = ApiState {
        store,
        engine,
    };

    Router::new()
        .route("/api/v1/query", get(query_handler))
        .route("/api/v1/query_range", get(query_range_handler))
        .route("/api/v1/write", post(write_handler))
        .route("/api/v1/series", get(series_handler))
        .route("/api/v1/labels", get(labels_handler))
        .route("/api/v1/label/:name/values", get(label_values_handler))
        .route("/api/v1/targets", get(targets_handler))
        .route("/api/v1/status/buildinfo", get(build_info_handler))
        .route("/api/v1/status/runtimeinfo", get(runtime_info_handler))
        .route("/api/v1/status/tsdb", get(tsdb_status_handler))
        .route("/metrics", get(metrics_handler))
        .route("/health", get(health_handler))
        .route("/ready", get(ready_handler))
        .with_state(Arc::new(state))
}

async fn query_handler(
    State(state): State<Arc<ApiState>>,
    Query(params): Query<QueryParams>,
) -> impl IntoResponse {
    let timestamp = params.time.unwrap_or_else(|| chrono::Utc::now().timestamp_millis());
    
    match state.engine.query_instant(&params.query, timestamp).await {
        Ok(result) => {
            let prom_response = convert_to_prometheus_response(&result, "vector");
            (StatusCode::OK, Json(prom_response))
        }
        Err(e) => {
            tracing::error!("Query error: {:?}", e);
            (StatusCode::BAD_REQUEST, Json(PrometheusResponse {
                status: "error".to_string(),
                data: None,
            }))
        }
    }
}

async fn query_range_handler(
    State(state): State<Arc<ApiState>>,
    Query(params): Query<QueryParams>,
) -> impl IntoResponse {
    let start = params.start.unwrap_or_else(|| chrono::Utc::now().timestamp_millis() - 3600000);
    let end = params.end.unwrap_or_else(|| chrono::Utc::now().timestamp_millis());
    let step = params.step.unwrap_or(1000);
    
    match state.engine.query_range(&params.query, start, end, step).await {
        Ok(result) => {
            let prom_response = convert_to_prometheus_response(&result, "matrix");
            (StatusCode::OK, Json(prom_response))
        }
        Err(e) => {
            tracing::error!("Query range error: {:?}", e);
            (StatusCode::BAD_REQUEST, Json(PrometheusResponse {
                status: "error".to_string(),
                data: None,
            }))
        }
    }
}

async fn write_handler(
    State(state): State<Arc<ApiState>>,
    Json(request): Json<WriteRequest>,
) -> impl IntoResponse {
    for ts in request.timeseries {
        if let Err(e) = state.store.write(ts.labels, ts.samples) {
            tracing::error!("Write error: {:?}", e);
            return (StatusCode::BAD_REQUEST, "Write failed");
        }
    }
    
    (StatusCode::OK, "Success")
}

async fn series_handler(
    State(state): State<Arc<ApiState>>,
    Query(params): Query<SeriesParams>,
) -> impl IntoResponse {
    let matchers: Vec<(String, String)> = params.matchers.into_iter().collect();
    
    let start = params.start.unwrap_or(0);
    let end = params.end.unwrap_or(i64::MAX);
    
    match state.store.query(&matchers, start, end) {
        Ok(series_list) => {
            let series_data: Vec<serde_json::Value> = series_list
                .into_iter()
                .map(|ts| {
                    let mut metric = serde_json::Map::new();
                    for label in &ts.labels {
                        metric.insert(label.name.clone(), serde_json::Value::String(label.value.clone()));
                    }
                    serde_json::Value::Object(metric)
                })
                .collect();
            
            let response = serde_json::json!({
                "status": "success",
                "data": series_data
            });
            
            (StatusCode::OK, Json(response))
        }
        Err(e) => {
            tracing::error!("Series query error: {:?}", e);
            let response = serde_json::json!({
                "status": "error",
                "error": format!("{:?}", e)
            });
            (StatusCode::BAD_REQUEST, Json(response))
        }
    }
}

async fn labels_handler(
    State(state): State<Arc<ApiState>>,
    Query(_params): Query<LabelsParams>,
) -> impl IntoResponse {
    let label_names = state.store.label_names();
    
    let response = serde_json::json!({
        "status": "success",
        "data": label_names
    });
    
    (StatusCode::OK, Json(response))
}

async fn label_values_handler(
    State(state): State<Arc<ApiState>>,
    Path(name): Path<String>,
    Query(_params): Query<LabelValuesParams>,
) -> impl IntoResponse {
    let values = state.store.label_values(&name);
    
    let response = serde_json::json!({
        "status": "success",
        "data": values
    });
    
    (StatusCode::OK, Json(response))
}

async fn targets_handler(
    State(_state): State<Arc<ApiState>>,
) -> impl IntoResponse {
    let response = serde_json::json!({
        "status": "success",
        "data": {
            "activeTargets": [],
            "droppedTargets": []
        }
    });
    
    (StatusCode::OK, Json(response))
}

async fn build_info_handler(
    State(_state): State<Arc<ApiState>>,
) -> impl IntoResponse {
    let response = serde_json::json!({
        "status": "success",
        "data": {
            "version": "1.0.0",
            "revision": "unknown",
            "branch": "main",
            "buildUser": "chronodb",
            "buildDate": "2024-01-01",
            "goVersion": "n/a"
        }
    });
    
    (StatusCode::OK, Json(response))
}

async fn runtime_info_handler(
    State(state): State<Arc<ApiState>>,
) -> impl IntoResponse {
    let stats = state.store.stats();
    
    let response = serde_json::json!({
        "status": "success",
        "data": {
            "startTime": "2024-01-01T00:00:00Z",
            "cwd": "/var/lib/chronodb",
            "reloadConfigSuccess": true,
            "lastConfigTime": "2024-01-01T00:00:00Z",
            "corruptionCount": 0,
            "goroutineCount": 10,
            "storage": {
                "series_count": stats.total_series,
                "sample_count": stats.total_samples,
                "disk_usage": stats.total_bytes
            }
        }
    });
    
    (StatusCode::OK, Json(response))
}

async fn tsdb_status_handler(
    State(state): State<Arc<ApiState>>,
) -> impl IntoResponse {
    let stats = state.store.stats();
    
    let response = serde_json::json!({
        "status": "success",
        "data": {
            "headStats": {
                "numSeries": stats.total_series,
                "numLabelPairs": 0,
                "chunkCount": 0,
                "minTime": 0,
                "maxTime": 0
            },
            "seriesCountByMetricName": [],
            "labelValueCountByLabelName": [],
            "memoryInBytesByLabelName": []
        }
    });
    
    (StatusCode::OK, Json(response))
}

async fn metrics_handler(
    State(state): State<Arc<ApiState>>,
) -> impl IntoResponse {
    let stats = state.store.stats();
    
    let mut output = String::new();
    
    output.push_str("# HELP chronodb_series_total Total number of time series\n");
    output.push_str("# TYPE chronodb_series_total gauge\n");
    output.push_str(&format!("chronodb_series_total {}\n", stats.total_series));
    
    output.push_str("# HELP chronodb_samples_total Total number of samples\n");
    output.push_str("# TYPE chronodb_samples_total gauge\n");
    output.push_str(&format!("chronodb_samples_total {}\n", stats.total_samples));
    
    output.push_str("# HELP chronodb_storage_bytes Total storage size in bytes\n");
    output.push_str("# TYPE chronodb_storage_bytes gauge\n");
    output.push_str(&format!("chronodb_storage_bytes {}\n", stats.total_bytes));
    
    output.push_str("# HELP chronodb_writes_total Total number of writes\n");
    output.push_str("# TYPE chronodb_writes_total counter\n");
    output.push_str(&format!("chronodb_writes_total {}\n", stats.writes));
    
    output.push_str("# HELP chronodb_reads_total Total number of reads\n");
    output.push_str("# TYPE chronodb_reads_total counter\n");
    output.push_str(&format!("chronodb_reads_total {}\n", stats.reads));
    
    (StatusCode::OK, output)
}

async fn health_handler(
    State(state): State<Arc<ApiState>>,
) -> impl IntoResponse {
    let is_healthy = state.store.stats().total_series >= 0;
    
    if is_healthy {
        (StatusCode::OK, "OK")
    } else {
        (StatusCode::SERVICE_UNAVAILABLE, "Unhealthy")
    }
}

async fn ready_handler(
    State(state): State<Arc<ApiState>>,
) -> impl IntoResponse {
    let is_ready = state.store.stats().total_series >= 0;
    
    if is_ready {
        (StatusCode::OK, "Ready")
    } else {
        (StatusCode::SERVICE_UNAVAILABLE, "Not Ready")
    }
}

fn convert_to_prometheus_response(result: &QueryResult, result_type: &str) -> PrometheusResponse {
    let mut prom_results = Vec::new();
    
    for series in &result.series {
        let mut metric = serde_json::Map::new();
        for label in &series.labels {
            metric.insert(label.name.clone(), serde_json::Value::String(label.value.clone()));
        }
        
        let prom_result = if result_type == "vector" {
            if let Some(sample) = series.samples.first() {
                PrometheusResult {
                    metric: serde_json::Value::Object(metric),
                    value: Some((sample.timestamp as f64 / 1000.0, sample.value)),
                    values: None,
                }
            } else {
                continue;
            }
        } else {
            let values: Vec<(f64, f64)> = series.samples.iter()
                .map(|s| (s.timestamp as f64 / 1000.0, s.value))
                .collect();
            
            PrometheusResult {
                metric: serde_json::Value::Object(metric),
                value: None,
                values: Some(values),
            }
        };
        
        prom_results.push(prom_result);
    }
    
    PrometheusResponse {
        status: "success".to_string(),
        data: Some(PrometheusData {
            resultType: result_type.to_string(),
            result: prom_results,
        }),
    }
}
