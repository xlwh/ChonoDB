use axum::{
    body::Bytes,
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, error, info, warn};

use crate::state::ServerState;
use chronodb_storage::remote::{
    RemoteWriteRequest, RemoteReadRequest, RemoteReadResponse, RemoteQueryResult,
    RemoteTimeSeries, RemoteLabel, RemoteSample, SnappyCodec, ProtoCodec,
};
use chronodb_storage::model::{Label, Sample, TimeSeries};

/// Remote Write 处理
pub async fn handle_remote_write(
    State(state): State<Arc<ServerState>>,
    body: Bytes,
) -> impl IntoResponse {
    debug!("Received remote write request, body size: {} bytes", body.len());

    // 解压数据
    let decompressed = match SnappyCodec::decompress(&body) {
        Ok(data) => data,
        Err(e) => {
            error!("Failed to decompress remote write data: {}", e);
            return (StatusCode::BAD_REQUEST, format!("Decompression error: {}", e));
        }
    };

    // 解析 protobuf 请求
    let request: RemoteWriteRequest = match ProtoCodec::decode(&decompressed) {
        Ok(req) => req,
        Err(e) => {
            error!("Failed to decode remote write request: {}", e);
            return (StatusCode::BAD_REQUEST, format!("Decode error: {}", e));
        }
    };

    let series_count = request.timeseries.len();
    info!("Processing remote write with {} time series", series_count);

    // 写入数据到存储
    let mut total_samples = 0;
    for remote_series in request.timeseries {
        let series: TimeSeries = remote_series.into();
        total_samples += series.samples.len();

        if let Err(e) = state.memstore.write(series.labels.to_vec(), series.samples) {
            error!("Failed to write time series: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, format!("Write error: {}", e));
        }
    }

    info!("Remote write completed: {} series, {} samples", series_count, total_samples);

    (StatusCode::OK, "OK".to_string())
}

/// Remote Read 处理
pub async fn handle_remote_read(
    State(state): State<Arc<ServerState>>,
    body: Bytes,
) -> impl IntoResponse {
    debug!("Received remote read request, body size: {} bytes", body.len());

    // 解压数据
    let decompressed = match SnappyCodec::decompress(&body) {
        Ok(data) => data,
        Err(e) => {
            error!("Failed to decompress remote read data: {}", e);
            return (StatusCode::BAD_REQUEST, format!("Decompression error: {}", e)).into_response();
        }
    };

    // 解析 protobuf 请求
    let request: RemoteReadRequest = match ProtoCodec::decode(&decompressed) {
        Ok(req) => req,
        Err(e) => {
            error!("Failed to decode remote read request: {}", e);
            return (StatusCode::BAD_REQUEST, format!("Decode error: {}", e)).into_response();
        }
    };

    info!("Processing remote read with {} queries", request.queries.len());

    // 执行查询
    let mut results = Vec::new();
    for query in request.queries {
        let matchers: Vec<(String, String)> = query.matchers.iter()
            .map(|m| (m.name.clone(), m.value.clone()))
            .collect();

        match state.memstore.query(&matchers, query.start_timestamp_ms, query.end_timestamp_ms) {
            Ok(series_list) => {
                let remote_series: Vec<RemoteTimeSeries> = series_list.into_iter()
                    .map(|ts| ts.into())
                    .collect();

                results.push(RemoteQueryResult {
                    timeseries: remote_series,
                });
            }
            Err(e) => {
                error!("Query error: {}", e);
                results.push(RemoteQueryResult {
                    timeseries: Vec::new(),
                });
            }
        }
    }

    let results_count = results.len();
    let response = RemoteReadResponse { results };

    // 编码响应
    let encoded = match ProtoCodec::encode(&response) {
        Ok(data) => data,
        Err(e) => {
            error!("Failed to encode remote read response: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, format!("Encode error: {}", e)).into_response();
        }
    };

    // 压缩响应
    let compressed = match SnappyCodec::compress(&encoded) {
        Ok(data) => data,
        Err(e) => {
            error!("Failed to compress remote read response: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, format!("Compression error: {}", e)).into_response();
        }
    };

    info!("Remote read completed: {} results", results_count);

    (
        StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, "application/x-protobuf")],
        compressed,
    ).into_response()
}

/// 接收 Prometheus remote write 的 HTTP 处理器
pub async fn receive_remote_write(
    State(state): State<Arc<ServerState>>,
    body: Bytes,
) -> impl IntoResponse {
    handle_remote_write(State(state), body).await
}

/// 接收 Prometheus remote read 的 HTTP 处理器
pub async fn receive_remote_read(
    State(state): State<Arc<ServerState>>,
    body: Bytes,
) -> impl IntoResponse {
    handle_remote_read(State(state), body).await
}

/// Remote write 状态检查
pub async fn remote_write_ready(
    State(state): State<Arc<ServerState>>,
) -> impl IntoResponse {
    // 检查存储是否可写
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
    use chronodb_storage::config::StorageConfig;
    use chronodb_storage::memstore::MemStore;
    use std::sync::Arc;

    fn create_test_state() -> Arc<ServerState> {
        let config = crate::config::ServerConfig::default();
        let storage_config = StorageConfig::default();
        let memstore = Arc::new(MemStore::new(storage_config).unwrap());

        // 简化创建，实际需要完整初始化
        // Arc::new(ServerState::new(config).await.unwrap())
        todo!()
    }

    // 测试用例需要完整的 ServerState 初始化
}
