use axum::{
    body::Bytes,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
};
use std::sync::Arc;
use tracing::{debug, error, info, warn};

use crate::state::ServerState;
use chronodb_storage::model::{Label, Sample};
use chronodb_storage::remote::{
    ProtoCodec, RemoteWriteRequest, SnappyCodec,
};

pub async fn handle_remote_write(
    State(state): State<Arc<ServerState>>,
    headers: HeaderMap,
    body: Bytes,
) -> impl IntoResponse {
    debug!("Received remote write request, body size: {} bytes", body.len());

    let content_type = headers
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    let is_protobuf = content_type.contains("application/x-protobuf")
        || content_type.contains("application/vnd.google.protobuf");

    if is_protobuf {
        match process_protobuf_write(&state, &body) {
            Ok((series_count, sample_count)) => {
                info!(
                    "Remote write completed (protobuf): {} series, {} samples",
                    series_count, sample_count
                );
                (StatusCode::NO_CONTENT, String::new())
            }
            Err(e) => {
                error!("Protobuf write failed: {}", e);
                (StatusCode::BAD_REQUEST, format!("Write error: {}", e))
            }
        }
    } else if is_likely_text(&body) {
        match process_text_write(&state, &body) {
            Ok((series_count, sample_count)) => {
                info!(
                    "Remote write completed (text): {} series, {} samples",
                    series_count, sample_count
                );
                (StatusCode::NO_CONTENT, String::new())
            }
            Err(e) => {
                error!("Text write failed: {}", e);
                (StatusCode::BAD_REQUEST, format!("Write error: {}", e))
            }
        }
    } else {
        match process_protobuf_write(&state, &body) {
            Ok((series_count, sample_count)) => {
                info!(
                    "Remote write completed (protobuf fallback): {} series, {} samples",
                    series_count, sample_count
                );
                (StatusCode::NO_CONTENT, String::new())
            }
            Err(_) => match process_text_write(&state, &body) {
                Ok((series_count, sample_count)) => {
                    info!(
                        "Remote write completed (text fallback): {} series, {} samples",
                        series_count, sample_count
                    );
                    (StatusCode::NO_CONTENT, String::new())
                }
                Err(e) => {
                    error!("All write formats failed: {}", e);
                    (StatusCode::BAD_REQUEST, format!("Write error: {}", e))
                }
            },
        }
    }
}

fn is_likely_text(data: &[u8]) -> bool {
    if data.is_empty() {
        return false;
    }
    let sample_size = std::cmp::min(data.len(), 512);
    let text_chars = data[..sample_size]
        .iter()
        .filter(|&&b| b == b'\n' || b == b'\r' || (b >= 32 && b < 127) || b == b'\t')
        .count();
    text_chars as f64 / sample_size as f64 > 0.9
}

fn process_protobuf_write(
    state: &Arc<ServerState>,
    body: &[u8],
) -> Result<(usize, usize), String> {
    let decompressed = SnappyCodec::decompress(body)
        .map_err(|e| format!("Snappy decompression failed: {}", e))?;

    let request: RemoteWriteRequest = ProtoCodec::decode(&decompressed)
        .map_err(|e| format!("Protobuf decode failed: {}", e))?;

    let series_count = request.timeseries.len();
    let mut total_samples = 0;

    for remote_series in request.timeseries {
        let series: chronodb_storage::model::TimeSeries = remote_series.into();
        total_samples += series.samples.len();

        if let Err(e) = state.memstore.write(series.labels.to_vec(), series.samples) {
            warn!("Failed to write time series: {}", e);
        }
    }

    Ok((series_count, total_samples))
}

fn process_text_write(
    state: &Arc<ServerState>,
    body: &[u8],
) -> Result<(usize, usize), String> {
    let text = std::str::from_utf8(body).map_err(|e| format!("Invalid UTF-8: {}", e))?;

    let lines: Vec<&str> = text
        .lines()
        .filter(|line| !line.trim().is_empty() && !line.starts_with('#'))
        .collect();

    let series_count = lines.len();
    let mut total_samples = 0;

    let mut batch_writes: std::collections::HashMap<Vec<Label>, Vec<Sample>> =
        std::collections::HashMap::new();

    for line in &lines {
        if let Some((labels, samples)) = parse_text_line(line) {
            total_samples += samples.len();
            batch_writes
                .entry(labels)
                .or_default()
                .extend(samples);
        }
    }

    for (labels, samples) in batch_writes {
        let sample_count = samples.len();
        if let Err(e) = state.memstore.write(labels, samples) {
            warn!("Failed to write batch: {}", e);
            continue;
        }
        debug!("Batch write successful for {} samples", sample_count);
    }

    Ok((series_count, total_samples))
}

fn parse_text_line(line: &str) -> Option<(Vec<Label>, Vec<Sample>)> {
    let line = line.trim();
    if line.is_empty() {
        return None;
    }

    let (metric_part, rest_str) = if let Some(brace_end) = line.rfind('}') {
        if let Some(space_pos) = line[brace_end..].find(' ') {
            let space_pos = brace_end + space_pos;
            (&line[..space_pos], line[space_pos..].trim())
        } else {
            (line, "")
        }
    } else if let Some(space_pos) = line.find(' ') {
        (&line[..space_pos], line[space_pos..].trim())
    } else {
        debug!("No space found in line, skipping: {}", line);
        return None;
    };

    let (value_part, timestamp_part_str) = if let Some(space_pos) = rest_str.find(' ') {
        let (v, t) = rest_str.split_at(space_pos);
        (v, t.trim())
    } else if rest_str.is_empty() {
        return None;
    } else {
        (rest_str, "")
    };

    let label_pairs = if let Some(brace_pos) = metric_part.find('{') {
        let name = &metric_part[..brace_pos];
        let labels_str = if let Some(close_brace_pos) = metric_part.rfind('}') {
            &metric_part[brace_pos + 1..close_brace_pos]
        } else {
            &metric_part[brace_pos + 1..]
        };

        let mut label_pairs = vec![("__name__".to_string(), name.to_string())];

        for label in labels_str.split(',') {
            let label = label.trim();
            if label.is_empty() {
                continue;
            }
            if let Some(equal_pos) = label.find('=') {
                let label_name = label[..equal_pos].trim().to_string();
                let label_value = label[equal_pos + 1..]
                    .trim()
                    .trim_matches('"')
                    .to_string();
                label_pairs.push((label_name, label_value));
            }
        }

        label_pairs
    } else {
        vec![("__name__".to_string(), metric_part.to_string())]
    };

    let sample_value = value_part.parse::<f64>().ok()?;
    let timestamp = if timestamp_part_str.is_empty() {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64
    } else {
        timestamp_part_str.parse::<i64>().unwrap_or(0)
    };

    let mut labels: Vec<Label> = label_pairs
        .into_iter()
        .map(|(name, value)| Label::new(name, value))
        .collect();
    labels.sort_by(|a, b| a.name.cmp(&b.name));

    let sample = Sample::new(timestamp, sample_value);

    Some((labels, vec![sample]))
}

/// Remote Read 处理
pub async fn handle_remote_read(
    State(state): State<Arc<ServerState>>,
    body: Bytes,
) -> impl IntoResponse {
    debug!("Received remote read request, body size: {} bytes", body.len());

    let decompressed = match SnappyCodec::decompress(&body) {
        Ok(data) => data,
        Err(e) => {
            error!("Failed to decompress remote read data: {}", e);
            return (
                StatusCode::BAD_REQUEST,
                format!("Decompression error: {}", e),
            )
                .into_response();
        }
    };

    let request: chronodb_storage::remote::RemoteReadRequest = match ProtoCodec::decode(&decompressed)
    {
        Ok(req) => req,
        Err(e) => {
            error!("Failed to decode remote read request: {}", e);
            return (
                StatusCode::BAD_REQUEST,
                format!("Decode error: {}", e),
            )
                .into_response();
        }
    };

    info!(
        "Processing remote read with {} queries",
        request.queries.len()
    );

    let mut results = Vec::new();
    for query in request.queries {
        let matchers: Vec<(String, String)> = query
            .matchers
            .iter()
            .filter(|m| {
                matches!(
                    m.matcher_type,
                    chronodb_storage::remote::MatcherType::Equal
                )
            })
            .map(|m| (m.name.clone(), m.value.clone()))
            .collect();

        match state
            .memstore
            .query(&matchers, query.start_timestamp_ms, query.end_timestamp_ms)
        {
            Ok(series_list) => {
                let remote_series: Vec<chronodb_storage::remote::RemoteTimeSeries> =
                    series_list.into_iter().map(|ts| ts.into()).collect();

                results.push(chronodb_storage::remote::RemoteQueryResult {
                    timeseries: remote_series,
                });
            }
            Err(e) => {
                error!("Query error: {}", e);
                results.push(chronodb_storage::remote::RemoteQueryResult {
                    timeseries: Vec::new(),
                });
            }
        }
    }

    let results_count = results.len();
    let response = chronodb_storage::remote::RemoteReadResponse { results };

    let encoded = match ProtoCodec::encode(&response) {
        Ok(data) => data,
        Err(e) => {
            error!("Failed to encode remote read response: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Encode error: {}", e),
            )
                .into_response();
        }
    };

    let compressed = match SnappyCodec::compress(&encoded) {
        Ok(data) => data,
        Err(e) => {
            error!("Failed to compress remote read response: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Compression error: {}", e),
            )
                .into_response();
        }
    };

    info!("Remote read completed: {} results", results_count);

    (
        StatusCode::OK,
        [
            (axum::http::header::CONTENT_TYPE, "application/x-protobuf"),
            (axum::http::header::CONTENT_ENCODING, "snappy"),
        ],
        compressed,
    )
        .into_response()
}

pub async fn receive_remote_write(
    State(state): State<Arc<ServerState>>,
    headers: HeaderMap,
    body: Bytes,
) -> impl IntoResponse {
    handle_remote_write(State(state), headers, body).await
}

pub async fn receive_remote_read(
    State(state): State<Arc<ServerState>>,
    body: Bytes,
) -> impl IntoResponse {
    handle_remote_read(State(state), body).await
}

pub async fn remote_write_ready(
    State(state): State<Arc<ServerState>>,
) -> impl IntoResponse {
    let stats = state.memstore.stats();

    if stats.writes > 0 {
        (StatusCode::OK, "Remote write is ready")
    } else {
        (StatusCode::SERVICE_UNAVAILABLE, "Remote write not ready")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_text_line_basic() {
        let line = r#"cpu_usage{job="webserver",instance="server1"} 75.5 1700000000000"#;
        let result = parse_text_line(line).unwrap();
        let (labels, samples) = result;

        assert_eq!(samples.len(), 1);
        assert_eq!(samples[0].timestamp, 1700000000000);
        assert!((samples[0].value - 75.5).abs() < f64::EPSILON);

        assert!(labels.iter().any(|l| l.name == "__name__" && l.value == "cpu_usage"));
        assert!(labels.iter().any(|l| l.name == "job" && l.value == "webserver"));
        assert!(labels.iter().any(|l| l.name == "instance" && l.value == "server1"));
    }

    #[test]
    fn test_parse_text_line_no_labels() {
        let line = r#"cpu_usage 50.0 1700000000000"#;
        let result = parse_text_line(line).unwrap();
        let (labels, samples) = result;

        assert_eq!(samples.len(), 1);
        assert!(labels.iter().any(|l| l.name == "__name__" && l.value == "cpu_usage"));
    }

    #[test]
    fn test_parse_text_line_no_timestamp() {
        let line = r#"cpu_usage{job="test"} 42.5"#;
        let result = parse_text_line(line).unwrap();
        let (labels, samples) = result;

        assert_eq!(samples.len(), 1);
        assert!(labels.iter().any(|l| l.name == "__name__" && l.value == "cpu_usage"));
        assert!(labels.iter().any(|l| l.name == "job" && l.value == "test"));
    }

    #[test]
    fn test_parse_text_line_empty() {
        assert!(parse_text_line("").is_none());
        assert!(parse_text_line("   ").is_none());
    }

    #[test]
    fn test_parse_text_line_complex_labels() {
        let line = r#"http_requests_total{job="api",instance="host:8080",region="us-east-1",env="production"} 1234 1700000000000"#;
        let result = parse_text_line(line).unwrap();
        let (labels, samples) = result;

        assert_eq!(samples.len(), 1);
        assert_eq!(labels.len(), 5);
        assert!(labels.iter().any(|l| l.name == "job" && l.value == "api"));
        assert!(labels.iter().any(|l| l.name == "region" && l.value == "us-east-1"));
        assert!(labels.iter().any(|l| l.name == "env" && l.value == "production"));
    }

    #[test]
    fn test_is_likely_text() {
        assert!(is_likely_text(b"cpu_usage{job=\"test\"} 42.5 1700000000000\n"));
        assert!(is_likely_text(b"hello world\n"));
        assert!(!is_likely_text(&[0xff, 0xfe, 0x00, 0x01, 0x02, 0x03]));
    }
}
