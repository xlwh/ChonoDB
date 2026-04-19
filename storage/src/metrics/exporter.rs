use crate::error::{Error, Result};
use crate::metrics::{Metric, MetricType, MetricValue, MetricsRegistry};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::time::interval;
use tracing::{error, info};

/// Prometheus指标导出器
pub struct PrometheusExporter {
    registry: Arc<RwLock<MetricsRegistry>>,
    config: ExporterConfig,
}

/// 导出器配置
#[derive(Debug, Clone)]
pub struct ExporterConfig {
    /// 导出间隔（秒）
    pub export_interval_secs: u64,
    /// 是否启用默认指标
    pub enable_default_metrics: bool,
    /// 指标前缀
    pub metrics_prefix: String,
}

impl Default for ExporterConfig {
    fn default() -> Self {
        Self {
            export_interval_secs: 15,
            enable_default_metrics: true,
            metrics_prefix: "chronodb".to_string(),
        }
    }
}

impl PrometheusExporter {
    pub fn new(registry: Arc<RwLock<MetricsRegistry>>, config: ExporterConfig) -> Self {
        Self {
            registry,
            config,
        }
    }

    /// 启动导出器
    pub async fn run(&self) -> Result<()> {
        info!("Prometheus exporter started");

        let mut ticker = interval(Duration::from_secs(self.config.export_interval_secs));

        loop {
            ticker.tick().await;

            if let Err(e) = self.collect_system_metrics().await {
                error!("Failed to collect system metrics: {}", e);
            }
        }
    }

    /// 收集系统指标
    async fn collect_system_metrics(&self) -> Result<()> {
        let registry = self.registry.write().await;

        // CPU使用率
        if let Ok(cpu_usage) = self.get_cpu_usage() {
            registry.register(Metric::new(
                &format!("{}_cpu_usage_percent", self.config.metrics_prefix),
                "CPU usage percentage",
                MetricType::Gauge,
            ).with_value(MetricValue::Gauge(cpu_usage))).await;
        }

        // 内存使用
        if let Ok(memory_usage) = self.get_memory_usage() {
            registry.register(Metric::new(
                &format!("{}_memory_usage_bytes", self.config.metrics_prefix),
                "Memory usage in bytes",
                MetricType::Gauge,
            ).with_value(MetricValue::Gauge(memory_usage as f64))).await;
        }

        // 磁盘使用
        if let Ok(disk_usage) = self.get_disk_usage() {
            registry.register(Metric::new(
                &format!("{}_disk_usage_bytes", self.config.metrics_prefix),
                "Disk usage in bytes",
                MetricType::Gauge,
            ).with_value(MetricValue::Gauge(disk_usage as f64))).await;
        }

        // Goroutine/线程数
        if let Ok(threads) = self.get_thread_count() {
            registry.register(Metric::new(
                &format!("{}_threads_total", self.config.metrics_prefix),
                "Number of threads",
                MetricType::Gauge,
            ).with_value(MetricValue::Gauge(threads as f64))).await;
        }

        // 打开的文件描述符数
        if let Ok(open_fds) = self.get_open_fds() {
            registry.register(Metric::new(
                &format!("{}_open_fds", self.config.metrics_prefix),
                "Number of open file descriptors",
                MetricType::Gauge,
            ).with_value(MetricValue::Gauge(open_fds as f64))).await;
        }

        Ok(())
    }

    /// 获取CPU使用率
    fn get_cpu_usage(&self) -> Result<f64> {
        // 简化实现，实际应该使用系统API
        #[cfg(target_os = "linux")]
        {
            use std::fs::File;
            use std::io::{BufRead, BufReader};

            let file = File::open("/proc/stat")
                .map_err(|e| Error::Internal(format!("Failed to read /proc/stat: {}", e)))?;
            let reader = BufReader::new(file);

            for line in reader.lines() {
                let line = line.map_err(|e| Error::Internal(e.to_string()))?;
                if line.starts_with("cpu ") {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 5 {
                        let user: f64 = parts[1].parse().unwrap_or(0.0);
                        let system: f64 = parts[3].parse().unwrap_or(0.0);
                        let idle: f64 = parts[4].parse().unwrap_or(0.0);
                        let total = user + system + idle;
                        if total > 0.0 {
                            return Ok((user + system) / total * 100.0);
                        }
                    }
                }
            }
        }

        Ok(0.0)
    }

    /// 获取内存使用
    fn get_memory_usage(&self) -> Result<u64> {
        #[cfg(target_os = "linux")]
        {
            use std::fs::File;
            use std::io::{BufRead, BufReader};

            let file = File::open("/proc/self/status")
                .map_err(|e| Error::Internal(format!("Failed to read /proc/self/status: {}", e)))?;
            let reader = BufReader::new(file);

            for line in reader.lines() {
                let line = line.map_err(|e| Error::Internal(e.to_string()))?;
                if line.starts_with("VmRSS:") {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 2 {
                        let kb: u64 = parts[1].parse().unwrap_or(0);
                        return Ok(kb * 1024);
                    }
                }
            }
        }

        Ok(0)
    }

    /// 获取磁盘使用
    fn get_disk_usage(&self) -> Result<u64> {
        // 简化实现
        Ok(0)
    }

    /// 获取线程数
    fn get_thread_count(&self) -> Result<usize> {
        #[cfg(target_os = "linux")]
        {
            use std::fs;
            let entries = fs::read_dir("/proc/self/task")
                .map_err(|e| Error::Internal(format!("Failed to read /proc/self/task: {}", e)))?;
            return Ok(entries.count());
        }

        Ok(1)
    }

    /// 获取打开的文件描述符数
    fn get_open_fds(&self) -> Result<usize> {
        #[cfg(target_os = "linux")]
        {
            use std::fs;
            let entries = fs::read_dir("/proc/self/fd")
                .map_err(|e| Error::Internal(format!("Failed to read /proc/self/fd: {}", e)))?;
            return Ok(entries.count());
        }

        Ok(0)
    }

    /// 导出为Prometheus格式
    pub async fn export(&self) -> Result<String> {
        let registry = self.registry.read().await;
        Ok(registry.export_prometheus().await)
    }
}

/// HTTP指标端点
pub struct MetricsEndpoint {
    exporter: Arc<PrometheusExporter>,
}

impl MetricsEndpoint {
    pub fn new(exporter: Arc<PrometheusExporter>) -> Self {
        Self { exporter }
    }

    /// 处理指标请求
    pub async fn handle_metrics(&self) -> Result<String> {
        self.exporter.export().await
    }
}

/// 查询性能指标
#[derive(Debug, Clone, Default)]
pub struct QueryMetrics {
    pub queries_total: u64,
    pub queries_success: u64,
    pub queries_failed: u64,
    pub query_duration_ms: Vec<f64>,
    pub series_scanned: u64,
    pub samples_scanned: u64,
    pub slow_query_count: u64,
    pub slow_query_threshold_ms: f64,
    pub concurrent_queries: u64,
    pub query_queue_length: u64,
}

impl QueryMetrics {
    pub fn record_query(&mut self, duration_ms: f64, success: bool, series: u64, samples: u64) {
        self.queries_total += 1;
        if success {
            self.queries_success += 1;
        } else {
            self.queries_failed += 1;
        }
        self.query_duration_ms.push(duration_ms);
        self.series_scanned += series;
        self.samples_scanned += samples;
        
        if duration_ms > self.slow_query_threshold_ms {
            self.slow_query_count += 1;
        }
    }

    pub fn avg_duration_ms(&self) -> f64 {
        if self.query_duration_ms.is_empty() {
            0.0
        } else {
            self.query_duration_ms.iter().sum::<f64>() / self.query_duration_ms.len() as f64
        }
    }

    pub fn p99_duration_ms(&self) -> f64 {
        if self.query_duration_ms.is_empty() {
            0.0
        } else {
            let mut sorted = self.query_duration_ms.clone();
            sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let idx = (sorted.len() as f64 * 0.99) as usize;
            sorted.get(idx).copied().unwrap_or(0.0)
        }
    }

    pub fn error_rate(&self) -> f64 {
        if self.queries_total == 0 {
            0.0
        } else {
            self.queries_failed as f64 / self.queries_total as f64
        }
    }
}

/// 写入性能指标
#[derive(Debug, Clone, Default)]
pub struct WriteMetrics {
    pub writes_total: u64,
    pub writes_success: u64,
    pub writes_failed: u64,
    pub write_duration_ms: Vec<f64>,
    pub bytes_written: u64,
    pub samples_written: u64,
    pub write_queue_length: u64,
    pub batch_write_size_distribution: Vec<u64>,
}

impl WriteMetrics {
    pub fn record_write(&mut self, duration_ms: f64, success: bool, bytes: u64, samples: u64) {
        self.writes_total += 1;
        if success {
            self.writes_success += 1;
        } else {
            self.writes_failed += 1;
        }
        self.write_duration_ms.push(duration_ms);
        self.bytes_written += bytes;
        self.samples_written += samples;
    }

    pub fn avg_duration_ms(&self) -> f64 {
        if self.write_duration_ms.is_empty() {
            0.0
        } else {
            self.write_duration_ms.iter().sum::<f64>() / self.write_duration_ms.len() as f64
        }
    }

    pub fn error_rate(&self) -> f64 {
        if self.writes_total == 0 {
            0.0
        } else {
            self.writes_failed as f64 / self.writes_total as f64
        }
    }

    pub fn throughput_samples_per_sec(&self) -> f64 {
        if self.write_duration_ms.is_empty() {
            0.0
        } else {
            let total_seconds: f64 = self.write_duration_ms.iter().sum::<f64>() / 1000.0;
            if total_seconds > 0.0 {
                self.samples_written as f64 / total_seconds
            } else {
                0.0
            }
        }
    }
}

/// 存储引擎指标
#[derive(Debug, Clone, Default)]
pub struct StorageEngineMetrics {
    pub memstore_series: u64,
    pub memstore_samples: u64,
    pub memstore_bytes: u64,
    pub block_count: u64,
    pub block_bytes: u64,
    pub index_entries: u64,
    pub wal_size_bytes: u64,
    pub flush_count: u64,
    pub compaction_count: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exporter_config_default() {
        let config = ExporterConfig::default();
        assert_eq!(config.export_interval_secs, 15);
        assert!(config.enable_default_metrics);
        assert_eq!(config.metrics_prefix, "chronodb");
    }

    #[test]
    fn test_query_metrics() {
        let mut metrics = QueryMetrics::default();
        metrics.record_query(100.0, true, 10, 1000);
        metrics.record_query(200.0, false, 5, 500);

        assert_eq!(metrics.queries_total, 2);
        assert_eq!(metrics.queries_success, 1);
        assert_eq!(metrics.queries_failed, 1);
        assert_eq!(metrics.avg_duration_ms(), 150.0);
    }

    #[test]
    fn test_write_metrics() {
        let mut metrics = WriteMetrics::default();
        metrics.record_write(50.0, true, 1024, 100);

        assert_eq!(metrics.writes_total, 1);
        assert_eq!(metrics.writes_success, 1);
        assert_eq!(metrics.bytes_written, 1024);
    }
}
