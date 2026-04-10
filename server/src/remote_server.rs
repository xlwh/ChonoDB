use axum::{
    body::Bytes,
    extract::State,
    http::StatusCode,
    response::IntoResponse,
};
use std::sync::Arc;
use tracing::{debug, error, info};

use crate::state::ServerState;
use http::HeaderMap;
use chronodb_storage::remote::{
    RemoteWriteRequest, RemoteReadRequest, RemoteReadResponse, RemoteQueryResult,
    RemoteTimeSeries, SnappyCodec, ProtoCodec,
};
use chronodb_storage::model::{
    TimeSeries, Label, Sample
};

/// Remote Write 处理
pub async fn handle_remote_write(
    State(state): State<Arc<ServerState>>,
    body: Bytes,
) -> impl IntoResponse {
    debug!("Received remote write request, body size: {} bytes", body.len());

    // 尝试解析数据
    let series_count: usize;
    let mut total_samples: usize;
    
    // 尝试解压数据
    match SnappyCodec::decompress(&body) {
        Ok(decompressed) => {
            // 尝试解析 protobuf 请求
            match ProtoCodec::decode::<RemoteWriteRequest>(&decompressed) {
                Ok(request) => {
                    // 处理 Protobuf 格式
                    series_count = request.timeseries.len();
                    info!("Processing remote write with {} time series (Protobuf format)", series_count);
                    
                    total_samples = 0;
                    for remote_series in request.timeseries {
                        let series: TimeSeries = remote_series.into();
                        total_samples += series.samples.len();

                        if let Err(e) = state.memstore.write(series.labels.to_vec(), series.samples) {
                            error!("Failed to write time series: {}", e);
                            return (StatusCode::INTERNAL_SERVER_ERROR, format!("Write error: {}", e));
                        }
                    }
                },
                Err(_) => {
                    // 尝试解析文本格式
                    match String::from_utf8(decompressed) {
                        Ok(text) => {
                            // 处理文本格式
                            info!("Processing remote write in text format");
                            
                            // 解析文本格式的指标
                            let lines: Vec<&str> = text.lines().filter(|line| !line.trim().is_empty() && !line.starts_with('#')).collect();
                            series_count = lines.len();
                            total_samples = lines.len();
                            
                            // 批量写入优化
                            let mut batch_writes: std::collections::HashMap<Vec<Label>, Vec<Sample>> = std::collections::HashMap::new();
                            
                            for line in lines {
                                debug!("Parsing line: {}", line);

                                // 解析文本格式：metric_name{label1="value1",label2="value2"} value timestamp
                                // 需要正确处理标签值中的空格，所以找到最后一个 '}' 后的第一个空格
                                let (metric_part, rest_str) = if let Some(brace_end) = line.rfind('}') {
                                    // 找到 '}' 后的第一个空格
                                    if let Some(space_pos) = line[brace_end..].find(' ') {
                                        let space_pos = brace_end + space_pos;
                                        (&line[..space_pos], line[space_pos..].trim())
                                    } else {
                                        // 没有空格，只有指标部分
                                        (line, "")
                                    }
                                } else {
                                    // 没有标签，找到第一个空格
                                    if let Some(space_pos) = line.find(' ') {
                                        (&line[..space_pos], line[space_pos..].trim())
                                    } else {
                                        debug!("No space found in line, skipping");
                                        continue;
                                    }
                                };
                                debug!("Metric part: {}, Rest: {}", metric_part, rest_str);
                                
                                // 然后将剩余部分分割为值和时间戳
                                let value_part: &str;
                                let timestamp_part_str: &str;
                                if let Some(space_pos) = rest_str.find(' ') {
                                    let (v, t) = rest_str.split_at(space_pos);
                                    value_part = v;
                                    timestamp_part_str = t.trim();
                                } else {
                                    // 直接使用当前时间作为时间戳
                                    value_part = rest_str;
                                    timestamp_part_str = "0";
                                };
                                debug!("Value: {}, Timestamp: {}", value_part, timestamp_part_str);
                                
                                // 解析指标名和标签
                                let label_pairs = if let Some(brace_pos) = metric_part.find('{') {
                                    let name = &metric_part[..brace_pos];
                                    // 找到匹配的 '}'，而不是简单地使用 len()-1
                                    let labels_str = if let Some(close_brace_pos) = metric_part.rfind('}') {
                                        &metric_part[brace_pos+1..close_brace_pos]
                                    } else {
                                        &metric_part[brace_pos+1..]
                                    };
                                    debug!("Name: {}, Labels str: '{}'", name, labels_str);

                                    let mut label_pairs = vec![("__name__".to_string(), name.to_string())];

                                    for label in labels_str.split(',') {
                                        let label = label.trim();
                                        if label.is_empty() {
                                            continue;
                                        }
                                        if let Some(equal_pos) = label.find('=') {
                                            let label_name = label[..equal_pos].trim().to_string();
                                            let label_value = label[equal_pos+1..].trim().trim_matches('"').to_string();
                                            debug!("Label: {} = {}", label_name, label_value);
                                            label_pairs.push((label_name, label_value));
                                        }
                                    }

                                    label_pairs
                                } else {
                                    vec![("__name__".to_string(), metric_part.to_string())]
                                };
                                
                                debug!("Label pairs: {:?}", label_pairs);
                                
                                // 解析值和时间戳
                                if let Ok(sample_value) = value_part.parse::<f64>() {
                                    if let Ok(timestamp) = timestamp_part_str.parse::<i64>() {
                                        // 转换为正确的类型
                            let mut labels: Vec<Label> = label_pairs.into_iter().map(|(name, value)| Label::new(name, value)).collect();
                            // 对标签进行排序，与calculate_series_id函数的行为一致
                            labels.sort_by(|a, b| a.name.cmp(&b.name));
                            let sample = Sample::new(timestamp, sample_value);
                            
                            // 批量收集数据
                            batch_writes.entry(labels).or_default().push(sample);
                                    } else {
                                        debug!("Failed to parse timestamp: {}", timestamp_part_str);
                                    }
                                } else {
                                    debug!("Failed to parse value: {}", value_part);
                                }
                            }
                            
                            // 执行批量写入
                            for (labels, samples) in batch_writes {
                                let sample_count = samples.len();
                                if let Err(e) = state.memstore.write(labels, samples) {
                                    error!("Failed to write time series: {}", e);
                                    // 继续处理其他批量，而不是立即返回
                                    continue;
                                }
                                debug!("Batch write successful for {} samples", sample_count);
                            }
                        },
                        Err(e) => {
                            error!("Failed to parse remote write data as text: {}", e);
                            return (StatusCode::BAD_REQUEST, format!("Parse error: {}", e));
                        }
                    }
                }
            }
        },
        Err(_) => {
            // 尝试直接解析文本格式（未压缩）
            match String::from_utf8(body.to_vec()) {
                Ok(text) => {
                    // 处理文本格式
                    info!("Processing remote write in text format (uncompressed)");
                    
                    // 解析文本格式的指标
                    let lines: Vec<&str> = text.lines().filter(|line| !line.trim().is_empty() && !line.starts_with('#')).collect();
                    series_count = lines.len();
                    total_samples = lines.len();
                    
                    // 批量写入优化
                    let mut batch_writes: std::collections::HashMap<Vec<Label>, Vec<Sample>> = std::collections::HashMap::new();
                    
                    for line in lines {
                        info!("Parsing line (uncompressed): {}", line);
                        
                        // 解析文本格式：metric_name{label1="value1",label2="value2"} value timestamp
                        // 需要正确处理标签值中的空格，所以找到最后一个 '}' 后的第一个空格
                        let (metric_part, rest_str) = if let Some(brace_end) = line.rfind('}') {
                            // 找到 '}' 后的第一个空格
                            if let Some(space_pos) = line[brace_end..].find(' ') {
                                let space_pos = brace_end + space_pos;
                                (&line[..space_pos], line[space_pos..].trim())
                            } else {
                                // 没有空格，只有指标部分
                                (line, "")
                            }
                        } else {
                            // 没有标签，找到第一个空格
                            if let Some(space_pos) = line.find(' ') {
                                (&line[..space_pos], line[space_pos..].trim())
                            } else {
                                info!("No space found in line, skipping");
                                continue;
                            }
                        };
                        info!("Metric part: '{}', Rest: '{}'", metric_part, rest_str);
                        
                        // 然后将剩余部分分割为值和时间戳
                        let value_part: &str;
                        let timestamp_part_str: &str;
                        if let Some(space_pos) = rest_str.find(' ') {
                            let (v, t) = rest_str.split_at(space_pos);
                            value_part = v;
                            timestamp_part_str = t.trim();
                        } else {
                            // 直接使用当前时间作为时间戳
                            value_part = rest_str;
                            timestamp_part_str = "0";
                        };
                        info!("Value: '{}', Timestamp: '{}'", value_part, timestamp_part_str);
                        
                        // 解析指标名和标签
                        let label_pairs = if let Some(brace_pos) = metric_part.find('{') {
                            let name = &metric_part[..brace_pos];
                            // 找到匹配的 '}'，而不是简单地使用 len()-1
                            let labels_str = if let Some(close_brace_pos) = metric_part.rfind('}') {
                                &metric_part[brace_pos+1..close_brace_pos]
                            } else {
                                &metric_part[brace_pos+1..]
                            };
                            info!("Name: '{}', Labels str: '{}'", name, labels_str);

                            let mut label_pairs = vec![("__name__".to_string(), name.to_string())];

                            for label in labels_str.split(',') {
                                let label = label.trim();
                                if label.is_empty() {
                                    continue;
                                }
                                if let Some(equal_pos) = label.find('=') {
                                    let label_name = label[..equal_pos].trim().to_string();
                                    let label_value = label[equal_pos+1..].trim().trim_matches('"').to_string();
                                    info!("Label: {} = {}", label_name, label_value);
                                    label_pairs.push((label_name, label_value));
                                }
                            }

                            label_pairs
                        } else {
                            vec![("__name__".to_string(), metric_part.to_string())]
                        };
                        
                        info!("Label pairs: {:?}", label_pairs);
                        
                        // 解析值和时间戳
                        if let Ok(sample_value) = value_part.parse::<f64>() {
                            if let Ok(timestamp) = timestamp_part_str.parse::<i64>() {
                                // 转换为正确的类型
                            let mut labels: Vec<Label> = label_pairs.into_iter().map(|(name, value)| Label::new(name, value)).collect();
                            // 对标签进行排序，与calculate_series_id函数的行为一致
                            labels.sort_by(|a, b| a.name.cmp(&b.name));
                            let sample = Sample::new(timestamp, sample_value);
                            
                            // 批量收集数据
                            batch_writes.entry(labels).or_default().push(sample);
                            } else {
                                info!("Failed to parse timestamp: {}", timestamp_part_str);
                            }
                        } else {
                            info!("Failed to parse value: {}", value_part);
                        }
                    }
                    
                    // 执行批量写入
                    for (labels, samples) in batch_writes {
                        let sample_count = samples.len();
                        if let Err(e) = state.memstore.write(labels, samples) {
                            error!("Failed to write time series: {}", e);
                            // 继续处理其他批量，而不是立即返回
                            continue;
                        }
                        info!("Batch write successful for {} samples", sample_count);
                    }
                },
                Err(e) => {
                    error!("Failed to parse remote write data as text: {}", e);
                    return (StatusCode::BAD_REQUEST, format!("Parse error: {}", e));
                }
            }
        }
    }

    info!("Remote write completed: {} series, {} samples", series_count, total_samples);

    (StatusCode::NO_CONTENT, "".to_string())
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
        // 处理不同类型的匹配器
        let matchers: Vec<(String, String)> = query.matchers.iter()
            .filter(|m| matches!(m.matcher_type, chronodb_storage::remote::MatcherType::Equal))
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
        [
            (axum::http::header::CONTENT_TYPE, "application/x-protobuf"),
            (axum::http::header::CONTENT_ENCODING, "snappy")
        ],
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
