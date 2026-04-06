pub mod exporter;

use crate::error::Result;
use crate::memstore::MemStore;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::RwLock;
use tracing::{info, debug};

pub use exporter::{PrometheusExporter, ExporterConfig, MetricsEndpoint, QueryMetrics, WriteMetrics, StorageEngineMetrics};

/// 指标类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetricType {
    Counter,
    Gauge,
    Histogram,
    Summary,
}

/// 指标值
#[derive(Debug, Clone)]
pub enum MetricValue {
    Counter(u64),
    Gauge(f64),
    Histogram(Vec<f64>),
    Summary { sum: f64, count: u64 },
}

/// 指标
#[derive(Debug, Clone)]
pub struct Metric {
    pub name: String,
    pub help: String,
    pub metric_type: MetricType,
    pub value: MetricValue,
    pub labels: HashMap<String, String>,
}

impl Metric {
    pub fn new(name: &str, help: &str, metric_type: MetricType) -> Self {
        Self {
            name: name.to_string(),
            help: help.to_string(),
            metric_type,
            value: MetricValue::Gauge(0.0),
            labels: HashMap::new(),
        }
    }

    pub fn with_label(mut self, name: &str, value: &str) -> Self {
        self.labels.insert(name.to_string(), value.to_string());
        self
    }

    pub fn with_value(mut self, value: MetricValue) -> Self {
        self.value = value;
        self
    }
}

/// 指标注册表
pub struct MetricsRegistry {
    metrics: Arc<RwLock<HashMap<String, Metric>>>,
}

impl MetricsRegistry {
    pub fn new() -> Self {
        Self {
            metrics: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn register(&self, metric: Metric) {
        let mut metrics = self.metrics.write().await;
        metrics.insert(metric.name.clone(), metric);
    }

    pub async fn get(&self, name: &str) -> Option<Metric> {
        let metrics = self.metrics.read().await;
        metrics.get(name).cloned()
    }

    pub async fn get_all(&self) -> Vec<Metric> {
        let metrics = self.metrics.read().await;
        metrics.values().cloned().collect()
    }

    /// 导出为Prometheus格式
    pub async fn export_prometheus(&self) -> String {
        let metrics = self.metrics.read().await;
        let mut output = String::new();

        for (_, metric) in metrics.iter() {
            output.push_str(&format!("# HELP {} {}\n", metric.name, metric.help));
            output.push_str(&format!("# TYPE {} {}\n", 
                metric.name, 
                match metric.metric_type {
                    MetricType::Counter => "counter",
                    MetricType::Gauge => "gauge",
                    MetricType::Histogram => "histogram",
                    MetricType::Summary => "summary",
                }
            ));

            let labels_str = if metric.labels.is_empty() {
                String::new()
            } else {
                let labels: Vec<String> = metric.labels
                    .iter()
                    .map(|(k, v)| format!("{}=\"{}\"", k, v))
                    .collect();
                format!("{{{}}}", labels.join(","))
            };

            let value_str = match &metric.value {
                MetricValue::Counter(v) => v.to_string(),
                MetricValue::Gauge(v) => v.to_string(),
                MetricValue::Histogram(_) => "0".to_string(),
                MetricValue::Summary { sum, count } => format!("{} {}", sum, count),
            };

            output.push_str(&format!("{}{} {}\n", metric.name, labels_str, value_str));
        }

        output
    }
}

impl Default for MetricsRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// 存储指标收集器
pub struct StorageMetricsCollector {
    registry: MetricsRegistry,
}

impl StorageMetricsCollector {
    pub fn new() -> Self {
        Self {
            registry: MetricsRegistry::new(),
        }
    }

    pub async fn collect(&self, store: &MemStore) -> Result<()> {
        let stats = store.stats();

        // 系列数量
        self.registry.register(Metric::new(
            "chronodb_series_total",
            "Total number of time series",
            MetricType::Gauge,
        ).with_value(MetricValue::Gauge(stats.total_series as f64))).await;

        // 样本数量
        self.registry.register(Metric::new(
            "chronodb_samples_total",
            "Total number of samples",
            MetricType::Gauge,
        ).with_value(MetricValue::Gauge(stats.total_samples as f64))).await;

        // 存储字节数
        self.registry.register(Metric::new(
            "chronodb_storage_bytes",
            "Total storage size in bytes",
            MetricType::Gauge,
        ).with_value(MetricValue::Gauge(stats.total_bytes as f64))).await;

        // 写入次数
        self.registry.register(Metric::new(
            "chronodb_writes_total",
            "Total number of writes",
            MetricType::Counter,
        ).with_value(MetricValue::Counter(stats.writes))).await;

        // 读取次数
        self.registry.register(Metric::new(
            "chronodb_reads_total",
            "Total number of reads",
            MetricType::Counter,
        ).with_value(MetricValue::Counter(stats.reads))).await;

        Ok(())
    }

    pub fn registry(&self) -> &MetricsRegistry {
        &self.registry
    }
}

impl Default for StorageMetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_metrics_registry() {
        let registry = MetricsRegistry::new();

        let metric = Metric::new(
            "test_metric",
            "A test metric",
            MetricType::Gauge,
        ).with_value(MetricValue::Gauge(42.0));

        registry.register(metric).await;

        let all = registry.get_all().await;
        assert_eq!(all.len(), 1);

        let exported = registry.export_prometheus().await;
        assert!(exported.contains("test_metric"));
        assert!(exported.contains("42"));
    }
}
