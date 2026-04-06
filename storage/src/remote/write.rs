use crate::error::Result;
use crate::model::TimeSeries;
use crate::remote::{RemoteConfig, RemoteTimeSeries, RemoteWriteRequest};
use crate::remote::codec::CompressedProtoCodec;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::{interval, sleep};
use tracing::{info, error, warn, debug};

/// Remote write请求
#[derive(Debug, Clone)]
pub struct WriteRequest {
    pub series: Vec<TimeSeries>,
}

/// Remote write响应
#[derive(Debug, Clone)]
pub struct WriteResponse {
    pub success: bool,
    pub error: Option<String>,
}

/// Remote写入器
pub struct RemoteWriter {
    config: RemoteConfig,
    tx: mpsc::Sender<WriteRequest>,
}

impl RemoteWriter {
    /// 创建新的remote writer
    pub fn new(config: RemoteConfig) -> (Self, RemoteWriterHandle) {
        let (tx, rx) = mpsc::channel(config.queue_size);
        
        let handle = RemoteWriterHandle::new(config.clone(), rx);
        
        let writer = Self { config, tx };
        
        (writer, handle)
    }

    /// 异步写入时间序列
    pub async fn write(&self, series: Vec<TimeSeries>) -> Result<()> {
        let request = WriteRequest { series };
        
        self.tx.send(request).await
            .map_err(|e| crate::error::Error::Internal(format!("Failed to queue write request: {}", e)))
    }

    /// 获取队列大小
    pub fn queue_size(&self) -> usize {
        self.config.queue_size
    }
}

/// Remote writer处理句柄
pub struct RemoteWriterHandle {
    config: RemoteConfig,
    rx: mpsc::Receiver<WriteRequest>,
}

impl RemoteWriterHandle {
    fn new(config: RemoteConfig, rx: mpsc::Receiver<WriteRequest>) -> Self {
        Self { config, rx }
    }

    /// 运行writer处理循环
    pub async fn run(mut self) {
        info!("Remote writer started, url: {}", self.config.remote_url);

        let mut batch_buffer: Vec<TimeSeries> = Vec::with_capacity(self.config.batch_size);
        let mut flush_interval = interval(Duration::from_secs(5));

        loop {
            tokio::select! {
                Some(request) = self.rx.recv() => {
                    batch_buffer.extend(request.series);
                    
                    if batch_buffer.len() >= self.config.batch_size {
                        if let Err(e) = self.flush(&batch_buffer).await {
                            error!("Failed to flush batch: {}", e);
                        }
                        batch_buffer.clear();
                    }
                }
                _ = flush_interval.tick() => {
                    if !batch_buffer.is_empty() {
                        if let Err(e) = self.flush(&batch_buffer).await {
                            error!("Failed to flush batch: {}", e);
                        }
                        batch_buffer.clear();
                    }
                }
                else => {
                    break;
                }
            }
        }

        // 刷新剩余数据
        if !batch_buffer.is_empty() {
            if let Err(e) = self.flush(&batch_buffer).await {
                error!("Failed to flush final batch: {}", e);
            }
        }

        info!("Remote writer stopped");
    }

    /// 刷新批次数据到远程服务器
    async fn flush(&self, series: &[TimeSeries]) -> Result<()> {
        if series.is_empty() {
            return Ok(());
        }

        debug!("Flushing {} series to remote server", series.len());

        // 转换为remote格式
        let remote_series: Vec<RemoteTimeSeries> = series
            .iter()
            .cloned()
            .map(RemoteTimeSeries::from)
            .collect();

        let request = RemoteWriteRequest {
            timeseries: remote_series,
        };

        // 编码并压缩
        let compressed_data = if self.config.enable_snappy {
            CompressedProtoCodec::encode_and_compress(&request)?
        } else {
            CompressedProtoCodec::encode(&request)?
        };

        // 发送请求（带重试）
        let mut last_error = None;
        for attempt in 0..self.config.max_retries {
            match self.send_request(&compressed_data).await {
                Ok(_) => {
                    debug!("Successfully sent {} series to remote server", series.len());
                    return Ok(());
                }
                Err(e) => {
                    warn!("Remote write attempt {} failed: {}", attempt + 1, e);
                    last_error = Some(e);
                    
                    if attempt < self.config.max_retries - 1 {
                        sleep(Duration::from_millis(self.config.retry_interval_ms)).await;
                    }
                }
            }
        }

        Err(last_error.unwrap_or_else(|| {
            crate::error::Error::Internal("Remote write failed after all retries".to_string())
        }))
    }

    /// 发送HTTP请求到远程服务器
    async fn send_request(&self, data: &[u8]) -> Result<()> {
        // 这里应该使用HTTP客户端发送请求
        // 简化实现，实际应该使用reqwest或hyper
        
        // 模拟网络延迟
        sleep(Duration::from_millis(10)).await;
        
        // 模拟成功
        debug!("Sent {} bytes to remote server", data.len());
        
        Ok(())
    }
}

/// 批量remote writer（支持多个远程目标）
pub struct BatchRemoteWriter {
    writers: Vec<RemoteWriter>,
}

impl BatchRemoteWriter {
    /// 创建批量writer
    pub fn new(configs: Vec<RemoteConfig>) -> (Self, Vec<RemoteWriterHandle>) {
        let mut writers = Vec::with_capacity(configs.len());
        let mut handles = Vec::with_capacity(configs.len());

        for config in configs {
            if config.enabled {
                let (writer, handle) = RemoteWriter::new(config);
                writers.push(writer);
                handles.push(handle);
            }
        }

        (Self { writers }, handles)
    }

    /// 写入到所有远程目标
    pub async fn write(&self, series: Vec<TimeSeries>) -> Result<()> {
        for writer in &self.writers {
            writer.write(series.clone()).await?;
        }
        Ok(())
    }

    /// 获取writer数量
    pub fn len(&self) -> usize {
        self.writers.len()
    }

    pub fn is_empty(&self) -> bool {
        self.writers.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Label, Sample};

    fn create_test_series(id: u64) -> TimeSeries {
        let labels = vec![
            Label::new("__name__", "test_metric"),
            Label::new("id", id.to_string()),
        ];

        let samples = vec![
            Sample::new(1000, 10.0),
            Sample::new(2000, 20.0),
        ];

        let mut ts = TimeSeries::new(id, labels);
        ts.add_samples(samples);
        ts
    }

    #[tokio::test]
    async fn test_remote_writer() {
        let config = RemoteConfig {
            enabled: true,
            remote_url: "http://localhost:9090/api/v1/write".to_string(),
            timeout_secs: 30,
            batch_size: 10,
            queue_size: 100,
            enable_snappy: true,
            max_retries: 3,
            retry_interval_ms: 100,
        };

        let (writer, handle) = RemoteWriter::new(config);

        // 启动处理循环
        let handle_task = tokio::spawn(async move {
            handle.run().await;
        });

        // 写入测试数据
        let series = vec![
            create_test_series(1),
            create_test_series(2),
        ];

        writer.write(series).await.unwrap();

        // 等待处理完成
        sleep(Duration::from_millis(100)).await;

        // 关闭writer
        drop(writer);
        
        // 等待处理循环结束
        let _ = tokio::time::timeout(Duration::from_secs(5), handle_task).await;
    }
}
