use crate::error::Result;
use crate::model::TimeSeries;
use crate::remote::{RemoteConfig, RemoteReadRequest, RemoteReadResponse, RemoteQueryResult};
use crate::remote::codec::CompressedProtoCodec;
use std::time::Duration;
use tracing::{warn, debug};

/// Remote read请求
#[derive(Debug, Clone)]
pub struct ReadRequest {
    pub queries: Vec<ReadQuery>,
}

#[derive(Debug, Clone)]
pub struct ReadQuery {
    pub start_timestamp_ms: i64,
    pub end_timestamp_ms: i64,
    pub matchers: Vec<(String, String)>, // (name, value)
}

/// Remote read响应
#[derive(Debug, Clone)]
pub struct ReadResponse {
    pub results: Vec<ReadResult>,
}

#[derive(Debug, Clone)]
pub struct ReadResult {
    pub series: Vec<TimeSeries>,
}

/// Remote读取器
pub struct RemoteReader {
    config: RemoteConfig,
}

impl RemoteReader {
    /// 创建新的remote reader
    pub fn new(config: RemoteConfig) -> Self {
        Self { config }
    }

    /// 从远程服务器读取数据
    pub async fn read(&self, request: ReadRequest) -> Result<ReadResponse> {
        if !self.config.enabled {
            return Err(crate::error::Error::Internal(
                "Remote read is disabled".to_string()
            ));
        }

        debug!("Reading {} queries from remote server", request.queries.len());

        // 转换为remote格式
        let remote_request = self.convert_to_remote_request(request)?;

        // 编码并压缩
        let compressed_data = if self.config.enable_snappy {
            CompressedProtoCodec::encode_and_compress(&remote_request)?
        } else {
            CompressedProtoCodec::encode(&remote_request)?
        };

        // 发送请求并获取响应
        let response_data = self.send_request(&compressed_data).await?;

        // 解压并解码
        let remote_response: RemoteReadResponse = if self.config.enable_snappy {
            CompressedProtoCodec::decompress_and_decode(&response_data)?
        } else {
            CompressedProtoCodec::decode(&response_data)?
        };

        // 转换为本地格式
        let results = remote_response.results
            .into_iter()
            .map(|r| ReadResult {
                series: r.timeseries
                    .into_iter()
                    .map(TimeSeries::from)
                    .collect(),
            })
            .collect();

        Ok(ReadResponse { results })
    }

    /// 转换请求格式
    fn convert_to_remote_request(&self, request: ReadRequest) -> Result<RemoteReadRequest> {
        let queries = request.queries
            .into_iter()
            .map(|q| crate::remote::RemoteQuery {
                start_timestamp_ms: q.start_timestamp_ms,
                end_timestamp_ms: q.end_timestamp_ms,
                matchers: q.matchers
                    .into_iter()
                    .map(|(name, value)| crate::remote::RemoteMatcher {
                        name,
                        value,
                        matcher_type: crate::remote::MatcherType::Equal,
                    })
                    .collect(),
                hints: None,
            })
            .collect();

        Ok(RemoteReadRequest { queries })
    }

    /// 发送HTTP请求到远程服务器
    async fn send_request(&self, data: &[u8]) -> Result<Vec<u8>> {
        // 这里应该使用HTTP客户端发送请求
        // 简化实现，实际应该使用reqwest或hyper
        
        // 模拟网络延迟
        tokio::time::sleep(Duration::from_millis(10)).await;
        
        // 模拟返回数据（实际应该从远程服务器获取）
        debug!("Sent {} bytes to remote server for read", data.len());
        
        // 返回模拟的响应数据
        let mock_response = RemoteReadResponse {
            results: vec![RemoteQueryResult {
                timeseries: vec![],
            }],
        };
        
        if self.config.enable_snappy {
            CompressedProtoCodec::encode_and_compress(&mock_response)
        } else {
            CompressedProtoCodec::encode(&mock_response)
        }
    }
}

/// 批量remote reader（支持多个远程目标）
pub struct BatchRemoteReader {
    readers: Vec<RemoteReader>,
}

impl BatchRemoteReader {
    /// 创建批量reader
    pub fn new(configs: Vec<RemoteConfig>) -> Self {
        let readers = configs
            .into_iter()
            .filter(|c| c.enabled)
            .map(RemoteReader::new)
            .collect();

        Self { readers }
    }

    /// 从所有远程目标读取数据并合并
    pub async fn read(&self, request: ReadRequest) -> Result<ReadResponse> {
        let mut all_results: Vec<ReadResult> = Vec::new();

        for reader in &self.readers {
            match reader.read(request.clone()).await {
                Ok(response) => {
                    all_results.extend(response.results);
                }
                Err(e) => {
                    warn!("Failed to read from remote: {}", e);
                }
            }
        }

        Ok(ReadResponse { results: all_results })
    }

    /// 获取reader数量
    pub fn len(&self) -> usize {
        self.readers.len()
    }

    pub fn is_empty(&self) -> bool {
        self.readers.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_remote_reader() {
        let config = RemoteConfig {
            enabled: true,
            remote_url: "http://localhost:9090/api/v1/read".to_string(),
            timeout_secs: 30,
            batch_size: 1000,
            queue_size: 10000,
            enable_snappy: true,
            max_retries: 3,
            retry_interval_ms: 1000,
        };

        let reader = RemoteReader::new(config);

        let request = ReadRequest {
            queries: vec![ReadQuery {
                start_timestamp_ms: 0,
                end_timestamp_ms: 10000,
                matchers: vec![("__name__".to_string(), "test_metric".to_string())],
            }],
        };

        let response = reader.read(request).await;
        assert!(response.is_ok());
    }
}
