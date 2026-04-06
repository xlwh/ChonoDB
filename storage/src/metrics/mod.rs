pub mod exporter;

use crate::error::Result;
use crate::memstore::MemStore;
use crate::distributed::{ClusterManager, ReplicationManager};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use tracing::{info, debug, warn};

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
    Histogram(Vec<(f64, u64)>), // (bucket, count)
    Summary { sum: f64, count: u64, quantiles: HashMap<f64, f64> },
}

/// 告警规则
#[derive(Debug, Clone)]
pub struct AlertRule {
    pub name: String,
    pub expr: String,
    pub for_duration: Duration,
    pub labels: HashMap<String, String>,
    pub annotations: HashMap<String, String>,
}

/// 告警状态
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AlertStatus {
    Firing,
    Pending,
    Resolved,
}

/// 告警
#[derive(Debug, Clone)]
pub struct Alert {
    pub rule_name: String,
    pub status: AlertStatus,
    pub labels: HashMap<String, String>,
    pub annotations: HashMap<String, String>,
    pub starts_at: SystemTime,
    pub ends_at: Option<SystemTime>,
    pub value: f64,
}

/// 指标
#[derive(Debug, Clone)]
pub struct Metric {
    pub name: String,
    pub help: String,
    pub metric_type: MetricType,
    pub value: MetricValue,
    pub labels: HashMap<String, String>,
    pub timestamp: Option<SystemTime>,
}

impl Metric {
    pub fn new(name: &str, help: &str, metric_type: MetricType) -> Self {
        Self {
            name: name.to_string(),
            help: help.to_string(),
            metric_type,
            value: MetricValue::Gauge(0.0),
            labels: HashMap::new(),
            timestamp: None,
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

    pub fn with_timestamp(mut self, timestamp: SystemTime) -> Self {
        self.timestamp = Some(timestamp);
        self
    }
}

/// 指标注册表
pub struct MetricsRegistry {
    metrics: Arc<RwLock<HashMap<String, Metric>>>,
    alerts: Arc<RwLock<Vec<Alert>>>,
    alert_rules: Arc<RwLock<Vec<AlertRule>>>,
}

impl MetricsRegistry {
    pub fn new() -> Self {
        Self {
            metrics: Arc::new(RwLock::new(HashMap::new())),
            alerts: Arc::new(RwLock::new(Vec::new())),
            alert_rules: Arc::new(RwLock::new(Vec::new())),
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

            let timestamp_str = metric.timestamp
                .map(|ts| {
                    let duration = ts.duration_since(UNIX_EPOCH).unwrap();
                    format!(" {}", duration.as_millis())
                })
                .unwrap_or_default();

            match &metric.value {
                MetricValue::Counter(v) => {
                    output.push_str(&format!("{}{} {}{}\n", metric.name, labels_str, v, timestamp_str));
                }
                MetricValue::Gauge(v) => {
                    output.push_str(&format!("{}{} {}{}\n", metric.name, labels_str, v, timestamp_str));
                }
                MetricValue::Histogram(buckets) => {
                    // 输出各桶的计数
                    for (bucket, count) in buckets {
                        let bucket_labels = format!("{{{},le=\"{}\"}}", 
                            if labels_str.is_empty() { "" } else { &labels_str[1..labels_str.len()-1] }, 
                            bucket
                        );
                        output.push_str(&format!("{}_bucket{}{} {}{}\n", metric.name, bucket_labels, labels_str, count, timestamp_str));
                    }
                    // 输出总和
                    output.push_str(&format!("{}_sum{}{} 0{}\n", metric.name, labels_str, timestamp_str));
                    // 输出计数
                    output.push_str(&format!("{}_count{}{} 0{}\n", metric.name, labels_str, timestamp_str));
                }
                MetricValue::Summary { sum, count, quantiles } => {
                    // 输出各分位数
                    for (quantile, value) in quantiles {
                        let quantile_labels = format!("{{{},quantile=\"{}\"}}", 
                            if labels_str.is_empty() { "" } else { &labels_str[1..labels_str.len()-1] }, 
                            quantile
                        );
                        output.push_str(&format!("{}{} {}{}\n", metric.name, quantile_labels, value, timestamp_str));
                    }
                    // 输出总和
                    output.push_str(&format!("{}_sum{}{} {}{}\n", metric.name, labels_str, sum, timestamp_str));
                    // 输出计数
                    output.push_str(&format!("{}_count{}{} {}{}\n", metric.name, labels_str, count, timestamp_str));
                }
            }
        }

        output
    }

    /// 添加告警规则
    pub async fn add_alert_rule(&self, rule: AlertRule) {
        let mut rules = self.alert_rules.write().await;
        rules.push(rule);
    }

    /// 获取所有告警规则
    pub async fn get_alert_rules(&self) -> Vec<AlertRule> {
        let rules = self.alert_rules.read().await;
        rules.clone()
    }

    /// 添加告警
    pub async fn add_alert(&self, alert: Alert) {
        let mut alerts = self.alerts.write().await;
        alerts.push(alert);
    }

    /// 获取所有告警
    pub async fn get_alerts(&self) -> Vec<Alert> {
        let alerts = self.alerts.read().await;
        alerts.clone()
    }

    /// 获取活跃告警
    pub async fn get_active_alerts(&self) -> Vec<Alert> {
        let alerts = self.alerts.read().await;
        alerts
            .iter()
            .filter(|a| a.status == AlertStatus::Firing || a.status == AlertStatus::Pending)
            .cloned()
            .collect()
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
    query_latency: Arc<RwLock<Vec<f64>>>,
    write_latency: Arc<RwLock<Vec<f64>>>,
}

impl StorageMetricsCollector {
    pub fn new() -> Self {
        Self {
            registry: MetricsRegistry::new(),
            query_latency: Arc::new(RwLock::new(Vec::new())),
            write_latency: Arc::new(RwLock::new(Vec::new())),
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

        // 查询延迟
        let query_latency = self.query_latency.read().await;
        if !query_latency.is_empty() {
            let avg_latency = query_latency.iter().sum::<f64>() / query_latency.len() as f64;
            self.registry.register(Metric::new(
                "chronodb_query_latency_ms",
                "Average query latency in milliseconds",
                MetricType::Gauge,
            ).with_value(MetricValue::Gauge(avg_latency))).await;
        }

        // 写入延迟
        let write_latency = self.write_latency.read().await;
        if !write_latency.is_empty() {
            let avg_latency = write_latency.iter().sum::<f64>() / write_latency.len() as f64;
            self.registry.register(Metric::new(
                "chronodb_write_latency_ms",
                "Average write latency in milliseconds",
                MetricType::Gauge,
            ).with_value(MetricValue::Gauge(avg_latency))).await;
        }

        Ok(())
    }

    /// 记录查询延迟
    pub async fn record_query_latency(&self, latency_ms: f64) {
        let mut latency = self.query_latency.write().await;
        latency.push(latency_ms);
        // 只保留最近1000个值
        if latency.len() > 1000 {
            latency.drain(0..latency.len() - 1000);
        }
    }

    /// 记录写入延迟
    pub async fn record_write_latency(&self, latency_ms: f64) {
        let mut latency = self.write_latency.write().await;
        latency.push(latency_ms);
        // 只保留最近1000个值
        if latency.len() > 1000 {
            latency.drain(0..latency.len() - 1000);
        }
    }

    /// 收集分布式系统指标
    pub async fn collect_distributed_metrics(&self, cluster_manager: &ClusterManager, replication_manager: &ReplicationManager) -> Result<()> {
        // 集群节点数量
        let nodes = cluster_manager.get_nodes().await?;
        self.registry.register(Metric::new(
            "chronodb_cluster_nodes_total",
            "Total number of nodes in the cluster",
            MetricType::Gauge,
        ).with_value(MetricValue::Gauge(nodes.len() as f64))).await;

        // 健康节点数量
        let healthy_nodes = cluster_manager.get_healthy_nodes().await?;
        self.registry.register(Metric::new(
            "chronodb_cluster_healthy_nodes_total",
            "Number of healthy nodes in the cluster",
            MetricType::Gauge,
        ).with_value(MetricValue::Gauge(healthy_nodes.len() as f64))).await;

        // 复制指标
        let replication_metrics = replication_manager.get_metrics().await?;
        self.registry.register(Metric::new(
            "chronodb_replication_total",
            "Total number of replication operations",
            MetricType::Counter,
        ).with_value(MetricValue::Counter(replication_metrics.total_replications))).await;

        self.registry.register(Metric::new(
            "chronodb_replication_successful_total",
            "Number of successful replication operations",
            MetricType::Counter,
        ).with_value(MetricValue::Counter(replication_metrics.successful_replications))).await;

        self.registry.register(Metric::new(
            "chronodb_replication_failed_total",
            "Number of failed replication operations",
            MetricType::Counter,
        ).with_value(MetricValue::Counter(replication_metrics.failed_replications))).await;

        self.registry.register(Metric::new(
            "chronodb_replication_latency_ms",
            "Average replication latency in milliseconds",
            MetricType::Gauge,
        ).with_value(MetricValue::Gauge(replication_metrics.replication_latency_ms))).await;

        Ok(())
    }

    /// 添加默认告警规则
    pub async fn add_default_alert_rules(&self) {
        // 系列数量告警
        self.registry.add_alert_rule(AlertRule {
            name: "HighSeriesCount",
            expr: "chronodb_series_total > 1000000",
            for_duration: Duration::from_minutes(5),
            labels: HashMap::from([("severity".to_string(), "warning".to_string())]),
            annotations: HashMap::from([
                ("summary".to_string(), "High series count".to_string()),
                ("description".to_string(), "The number of time series has exceeded 1,000,000".to_string()),
            ]),
        }).await;

        // 查询延迟告警
        self.registry.add_alert_rule(AlertRule {
            name: "HighQueryLatency",
            expr: "chronodb_query_latency_ms > 1000",
            for_duration: Duration::from_minutes(5),
            labels: HashMap::from([("severity".to_string(), "warning".to_string())]),
            annotations: HashMap::from([
                ("summary".to_string(), "High query latency".to_string()),
                ("description".to_string(), "Average query latency has exceeded 1000ms".to_string()),
            ]),
        }).await;

        // 节点健康告警
        self.registry.add_alert_rule(AlertRule {
            name: "ClusterNodeDown",
            expr: "chronodb_cluster_healthy_nodes_total < chronodb_cluster_nodes_total",
            for_duration: Duration::from_minutes(2),
            labels: HashMap::from([("severity".to_string(), "critical".to_string())]),
            annotations: HashMap::from([
                ("summary".to_string(), "Cluster node down".to_string()),
                ("description".to_string(), "One or more nodes in the cluster are down".to_string()),
            ]),
        }).await;
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

    #[tokio::test]
    async fn test_alert_rules() {
        let registry = MetricsRegistry::new();

        let rule = AlertRule {
            name: "TestAlert",
            expr: "test_metric > 100",
            for_duration: Duration::from_minutes(5),
            labels: HashMap::new(),
            annotations: HashMap::new(),
        };

        registry.add_alert_rule(rule).await;

        let rules = registry.get_alert_rules().await;
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].name, "TestAlert");
    }

    #[tokio::test]
    async fn test_storage_metrics_collector() {
        let collector = StorageMetricsCollector::new();

        // 记录查询延迟
        collector.record_query_latency(100.0).await;
        collector.record_query_latency(200.0).await;

        // 记录写入延迟
        collector.record_write_latency(10.0).await;
        collector.record_write_latency(20.0).await;

        // 添加默认告警规则
        collector.add_default_alert_rules().await;

        let rules = collector.registry().get_alert_rules().await;
        assert!(!rules.is_empty());
    }
}
