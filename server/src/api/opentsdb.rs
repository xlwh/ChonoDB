use axum::{
    extract::{Query, State},
    http::StatusCode,
    body::Bytes,
    Json,
};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, error, info, warn};

use crate::api::models::*;
use crate::state::ServerState;
use chronodb_storage::model::{Label, Sample};

pub async fn handle_opentsdb_put(
    State(state): State<Arc<ServerState>>,
    Query(params): Query<HashMap<String, String>>,
    body: Bytes,
) -> (StatusCode, String) {
    let summary = params.contains_key("summary");
    let details = params.contains_key("details");

    debug!("OpenTSDB /api/put request, body size: {} bytes, summary: {}, details: {}",
           body.len(), summary, details);

    let body_str = match std::str::from_utf8(&body) {
        Ok(s) => s,
        Err(e) => {
            let err_resp = OpenTSDBErrorResponse {
                error: OpenTSDBErrorDetail {
                    code: 400,
                    message: format!("Invalid UTF-8 body: {}", e),
                },
            };
            return (StatusCode::BAD_REQUEST, serde_json::to_string(&err_resp).unwrap_or_default());
        }
    };

    let data_points: Vec<OpenTSDBPutRequest> = if body_str.trim_start().starts_with('[') {
        match serde_json::from_str(body_str) {
            Ok(pts) => pts,
            Err(e) => {
                let err_resp = OpenTSDBErrorResponse {
                    error: OpenTSDBErrorDetail {
                        code: 400,
                        message: format!("Invalid JSON array: {}", e),
                    },
                };
                return (StatusCode::BAD_REQUEST, serde_json::to_string(&err_resp).unwrap_or_default());
            }
        }
    } else {
        match serde_json::from_str(body_str) {
            Ok(pt) => vec![pt],
            Err(e) => {
                let err_resp = OpenTSDBErrorResponse {
                    error: OpenTSDBErrorDetail {
                        code: 400,
                        message: format!("Invalid JSON object: {}", e),
                    },
                };
                return (StatusCode::BAD_REQUEST, serde_json::to_string(&err_resp).unwrap_or_default());
            }
        }
    };

    let mut success_count: usize = 0;
    let mut failed_count: usize = 0;
    let mut errors: Vec<OpenTSDBPutError> = Vec::new();

    for dp in &data_points {
        match process_datapoint(&state, dp) {
            Ok(()) => {
                success_count += 1;
            }
            Err(e) => {
                warn!("OpenTSDB put failed for metric {}: {}", dp.metric, e);
                if details {
                    errors.push(OpenTSDBPutError {
                        datapoint: dp.clone(),
                        error: e,
                    });
                }
                failed_count += 1;
            }
        }
    }

    info!("OpenTSDB /api/put: {} success, {} failed out of {} total",
          success_count, failed_count, data_points.len());

    if !summary && !details {
        if failed_count > 0 {
            let err_resp = OpenTSDBErrorResponse {
                error: OpenTSDBErrorDetail {
                    code: 400,
                    message: format!("{} of {} data points failed", failed_count, data_points.len()),
                },
            };
            return (StatusCode::BAD_REQUEST, serde_json::to_string(&err_resp).unwrap_or_default());
        }
        return (StatusCode::NO_CONTENT, String::new());
    }

    if details {
        let detail_resp = OpenTSDBPutDetail {
            failed: failed_count,
            success: success_count,
            errors,
        };
        (StatusCode::OK, serde_json::to_string(&detail_resp).unwrap_or_default())
    } else {
        let summary_resp = OpenTSDBPutSummary {
            failed: failed_count,
            success: success_count,
        };
        (StatusCode::OK, serde_json::to_string(&summary_resp).unwrap_or_default())
    }
}

fn process_datapoint(state: &Arc<ServerState>, dp: &OpenTSDBPutRequest) -> Result<(), String> {
    if dp.metric.is_empty() {
        return Err("Metric name cannot be empty".to_string());
    }

    if dp.tags.is_empty() {
        return Err("At least one tag is required".to_string());
    }

    let value = parse_value(&dp.value)?;

    let timestamp_ms = if dp.timestamp > 0 && dp.timestamp < 4102444800 {
        dp.timestamp * 1000
    } else {
        dp.timestamp
    };

    let mut labels: Vec<Label> = Vec::with_capacity(dp.tags.len() + 1);
    labels.push(Label::new("__name__".to_string(), dp.metric.clone()));
    for (k, v) in &dp.tags {
        labels.push(Label::new(k.clone(), v.clone()));
    }
    labels.sort_by(|a, b| a.name.cmp(&b.name));

    let sample = Sample::new(timestamp_ms, value);

    state.memstore.write(labels, vec![sample])
        .map_err(|e| format!("Write error: {}", e))
}

fn parse_value(value: &serde_json::Value) -> Result<f64, String> {
    match value {
        serde_json::Value::Number(n) => {
            n.as_f64().ok_or_else(|| "Invalid numeric value".to_string())
        }
        serde_json::Value::String(s) => {
            s.parse::<f64>().map_err(|e| format!("Cannot parse value '{}': {}", s, e))
        }
        _ => Err("Value must be a number or numeric string".to_string()),
    }
}
