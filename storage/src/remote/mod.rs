pub mod write;
pub mod read;
pub mod codec;

pub use write::{RemoteWriter, WriteRequest, WriteResponse};
pub use read::{RemoteReader, ReadRequest, ReadResponse};
pub use codec::{SnappyCodec, ProtoCodec};

use crate::model::{Label, Sample, TimeSeries};
use serde::{Deserialize, Serialize};

/// Remote write配置
#[derive(Debug, Clone)]
pub struct RemoteConfig {
    /// 是否启用remote write
    pub enabled: bool,
    /// 远程服务器URL
    pub remote_url: String,
    /// 写入超时时间（秒）
    pub timeout_secs: u64,
    /// 批量大小
    pub batch_size: usize,
    /// 队列大小
    pub queue_size: usize,
    /// 是否启用snappy压缩
    pub enable_snappy: bool,
    /// 重试次数
    pub max_retries: u32,
    /// 重试间隔（毫秒）
    pub retry_interval_ms: u64,
}

impl Default for RemoteConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            remote_url: "http://localhost:9090/api/v1/write".to_string(),
            timeout_secs: 30,
            batch_size: 1000,
            queue_size: 10000,
            enable_snappy: true,
            max_retries: 3,
            retry_interval_ms: 1000,
        }
    }
}

/// Prometheus remote write格式的时间序列
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteTimeSeries {
    pub labels: Vec<RemoteLabel>,
    pub samples: Vec<RemoteSample>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteLabel {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteSample {
    pub timestamp: i64,
    pub value: f64,
}

impl From<TimeSeries> for RemoteTimeSeries {
    fn from(ts: TimeSeries) -> Self {
        Self {
            labels: ts.labels.into_iter()
                .map(|l| RemoteLabel {
                    name: l.name,
                    value: l.value,
                })
                .collect(),
            samples: ts.samples.into_iter()
                .map(|s| RemoteSample {
                    timestamp: s.timestamp,
                    value: s.value,
                })
                .collect(),
        }
    }
}

impl From<RemoteTimeSeries> for TimeSeries {
    fn from(rts: RemoteTimeSeries) -> Self {
        let labels: Vec<Label> = rts.labels.into_iter()
            .map(|l| Label::new(l.name, l.value))
            .collect();
        
        let samples: Vec<Sample> = rts.samples.into_iter()
            .map(|s| Sample::new(s.timestamp, s.value))
            .collect();
        
        let mut ts = TimeSeries::new(0, labels);
        ts.add_samples(samples);
        ts
    }
}

/// Remote write请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteWriteRequest {
    pub timeseries: Vec<RemoteTimeSeries>,
}

impl RemoteWriteRequest {
    pub fn new() -> Self {
        Self {
            timeseries: Vec::new(),
        }
    }

    pub fn add_series(&mut self, series: RemoteTimeSeries) {
        self.timeseries.push(series);
    }

    pub fn is_empty(&self) -> bool {
        self.timeseries.is_empty()
    }

    pub fn len(&self) -> usize {
        self.timeseries.len()
    }
}

/// Remote write响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteWriteResponse {
    pub success: bool,
    pub error: Option<String>,
}

/// Remote read请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteReadRequest {
    pub queries: Vec<RemoteQuery>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteQuery {
    pub start_timestamp_ms: i64,
    pub end_timestamp_ms: i64,
    pub matchers: Vec<RemoteMatcher>,
    pub hints: Option<RemoteHints>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteMatcher {
    pub name: String,
    pub value: String,
    pub matcher_type: MatcherType,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum MatcherType {
    Equal,
    NotEqual,
    RegexMatch,
    RegexNoMatch,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteHints {
    pub step_ms: Option<i64>,
    pub func: Option<String>,
    pub grouping: Vec<String>,
    pub by: bool,
    pub range_ms: Option<i64>,
}

/// Remote read响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteReadResponse {
    pub results: Vec<RemoteQueryResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteQueryResult {
    pub timeseries: Vec<RemoteTimeSeries>,
}

/// 将本地Matcher转换为RemoteMatcher
impl From<crate::query::parser::MatchOp> for MatcherType {
    fn from(op: crate::query::parser::MatchOp) -> Self {
        match op {
            crate::query::parser::MatchOp::Equal => MatcherType::Equal,
            crate::query::parser::MatchOp::NotEqual => MatcherType::NotEqual,
            crate::query::parser::MatchOp::Regex => MatcherType::RegexMatch,
            crate::query::parser::MatchOp::NotRegex => MatcherType::RegexNoMatch,
            crate::query::parser::MatchOp::RegexMatch => MatcherType::RegexMatch,
            crate::query::parser::MatchOp::RegexNotMatch => MatcherType::RegexNoMatch,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_remote_time_series_conversion() {
        let labels = vec![
            Label::new("__name__", "test_metric"),
            Label::new("job", "test"),
        ];
        let samples = vec![
            Sample::new(1000, 10.0),
            Sample::new(2000, 20.0),
        ];

        let mut ts = TimeSeries::new(1, labels);
        ts.add_samples(samples);

        let remote_ts: RemoteTimeSeries = ts.clone().into();
        assert_eq!(remote_ts.labels.len(), 2);
        assert_eq!(remote_ts.samples.len(), 2);

        let converted_back: TimeSeries = remote_ts.into();
        assert_eq!(converted_back.samples.len(), 2);
    }

    #[test]
    fn test_remote_write_request() {
        let mut request = RemoteWriteRequest::new();
        
        let series = RemoteTimeSeries {
            labels: vec![
                RemoteLabel {
                    name: "__name__".to_string(),
                    value: "test".to_string(),
                },
            ],
            samples: vec![
                RemoteSample {
                    timestamp: 1000,
                    value: 10.0,
                },
            ],
        };
        
        request.add_series(series);
        assert_eq!(request.len(), 1);
        assert!(!request.is_empty());
    }
}
