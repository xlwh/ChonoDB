pub mod exporter;

use crate::error::Result;
use crate::memstore::MemStore;
use crate::distributed::{ClusterManager, ReplicationManager};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;

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

/// API 统计信息
#[derive(Debug, Clone, Default)]
pub struct ApiStats {
    pub total_requests: u64,
    pub successful_requests: u64,
    pub failed_requests: u64,
    pub avg_request_time_ms: f64,
    pub max_request_time_ms: u64,
    pub min_request_time_ms: u64,
    pub request_time_sum_ms: f64,
    pub request_time_count: u64,
    pub average_latency_ms: f64,
    pub endpoint_stats: std::collections::HashMap<String, EndpointStats>,
}

/// 端点统计信息
#[derive(Debug, Clone, Default)]
pub struct EndpointStats {
    pub total_requests: u64,
    pub successful_requests: u64,
    pub failed_requests: u64,
    pub avg_request_time_ms: f64,
    pub max_request_time_ms: u64,
    pub min_request_time_ms: u64,
    pub requests: u64,
    pub average_latency_ms: f64,
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
                    output.push_str(&format!("{}_sum{}{} 0\n", metric.name, labels_str, timestamp_str));
                    // 输出计数
                    output.push_str(&format!("{}_count{}{} 0\n", metric.name, labels_str, timestamp_str));
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
                    output.push_str(&format!("{}_sum{}{} {}\n", metric.name, labels_str, sum, timestamp_str));
                    // 输出计数
                    output.push_str(&format!("{}_count{}{} {}\n", metric.name, labels_str, count, timestamp_str));
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

        self.registry.register(Metric::new(
            "chronodb_series_total",
            "Total number of time series",
            MetricType::Gauge,
        ).with_value(MetricValue::Gauge(stats.total_series as f64))).await;

        self.registry.register(Metric::new(
            "chronodb_series_count",
            "Total number of time series (for Grafana compatibility)",
            MetricType::Gauge,
        ).with_value(MetricValue::Gauge(stats.total_series as f64))).await;

        self.registry.register(Metric::new(
            "chronodb_samples_total",
            "Total number of samples",
            MetricType::Gauge,
        ).with_value(MetricValue::Gauge(stats.total_samples as f64))).await;

        self.registry.register(Metric::new(
            "chronodb_storage_bytes",
            "Total storage size in bytes",
            MetricType::Gauge,
        ).with_value(MetricValue::Gauge(stats.total_bytes as f64))).await;

        self.registry.register(Metric::new(
            "chronodb_disk_usage_bytes",
            "Disk usage in bytes",
            MetricType::Gauge,
        ).with_value(MetricValue::Gauge(stats.total_bytes as f64))).await;

        self.registry.register(Metric::new(
            "chronodb_block_count",
            "Number of storage blocks",
            MetricType::Gauge,
        ).with_value(MetricValue::Gauge(0.0))).await;

        self.registry.register(Metric::new(
            "chronodb_compression_ratio",
            "Data compression ratio",
            MetricType::Gauge,
        ).with_value(MetricValue::Gauge(1.0))).await;

        self.registry.register(Metric::new(
            "chronodb_wal_size_bytes",
            "WAL size in bytes",
            MetricType::Gauge,
        ).with_value(MetricValue::Gauge(0.0))).await;

        self.registry.register(Metric::new(
            "chronodb_index_size_bytes",
            "Index size in bytes",
            MetricType::Gauge,
        ).with_value(MetricValue::Gauge(0.0))).await;

        self.registry.register(Metric::new(
            "chronodb_writes_total",
            "Total number of writes",
            MetricType::Counter,
        ).with_value(MetricValue::Counter(stats.writes))).await;

        self.registry.register(Metric::new(
            "chronodb_write_requests_total",
            "Total number of write requests (for Grafana compatibility)",
            MetricType::Counter,
        ).with_value(MetricValue::Counter(stats.writes))).await;

        self.registry.register(Metric::new(
            "chronodb_reads_total",
            "Total number of reads",
            MetricType::Counter,
        ).with_value(MetricValue::Counter(stats.reads))).await;

        self.registry.register(Metric::new(
            "chronodb_query_requests_total",
            "Total number of query requests (for Grafana compatibility)",
            MetricType::Counter,
        ).with_value(MetricValue::Counter(stats.reads))).await;

        let query_latency = self.query_latency.read().await;
        if !query_latency.is_empty() {
            let avg_latency = query_latency.iter().sum::<f64>() / query_latency.len() as f64;
            self.registry.register(Metric::new(
                "chronodb_query_latency_ms",
                "Average query latency in milliseconds",
                MetricType::Gauge,
            ).with_value(MetricValue::Gauge(avg_latency))).await;

            let sum_seconds = query_latency.iter().sum::<f64>() / 1000.0;
            let count = query_latency.len() as u64;
            self.registry.register(Metric::new(
                "chronodb_query_duration_seconds_sum",
                "Total query duration in seconds (for Grafana compatibility)",
                MetricType::Counter,
            ).with_value(MetricValue::Counter(sum_seconds as u64))).await;
            self.registry.register(Metric::new(
                "chronodb_query_duration_seconds_count",
                "Number of query requests (for Grafana compatibility)",
                MetricType::Counter,
            ).with_value(MetricValue::Counter(count))).await;
        }

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

    /// 收集降采样指标
    pub async fn collect_downsample_metrics(&self, downsample_stats: &crate::downsample::DownsampleStats) -> Result<()> {
        // 降采样任务总数
        self.registry.register(Metric::new(
            "chronodb_downsample_tasks_total",
            "Total number of downsample tasks",
            MetricType::Counter,
        ).with_value(MetricValue::Counter(downsample_stats.total_tasks))).await;

        // 已完成的降采样任务数
        self.registry.register(Metric::new(
            "chronodb_downsample_tasks_completed_total",
            "Number of completed downsample tasks",
            MetricType::Counter,
        ).with_value(MetricValue::Counter(downsample_stats.completed_tasks))).await;

        // 失败的降采样任务数
        self.registry.register(Metric::new(
            "chronodb_downsample_tasks_failed_total",
            "Number of failed downsample tasks",
            MetricType::Counter,
        ).with_value(MetricValue::Counter(downsample_stats.failed_tasks))).await;

        // 处理的样本数
        self.registry.register(Metric::new(
            "chronodb_downsample_samples_processed_total",
            "Total number of samples processed by downsampling",
            MetricType::Counter,
        ).with_value(MetricValue::Counter(downsample_stats.total_samples_processed))).await;

        // 生成的样本数
        self.registry.register(Metric::new(
            "chronodb_downsample_samples_generated_total",
            "Total number of samples generated by downsampling",
            MetricType::Counter,
        ).with_value(MetricValue::Counter(downsample_stats.total_samples_generated))).await;

        // 各降采样级别的统计
        for (level, level_stats) in &downsample_stats.level_stats {
            let level_str = format!("{:?}", level);
            
            // 各级别处理的样本数
            self.registry.register(Metric::new(
                "chronodb_downsample_level_samples_processed_total",
                "Number of samples processed by downsampling per level",
                MetricType::Counter,
            ).with_label("level", &level_str)
              .with_value(MetricValue::Counter(level_stats.samples_processed))).await;

            // 各级别生成的样本数
            self.registry.register(Metric::new(
                "chronodb_downsample_level_samples_generated_total",
                "Number of samples generated by downsampling per level",
                MetricType::Counter,
            ).with_label("level", &level_str)
              .with_value(MetricValue::Counter(level_stats.samples_generated))).await;

            // 各级别的任务数
            self.registry.register(Metric::new(
                "chronodb_downsample_level_tasks_total",
                "Number of downsample tasks per level",
                MetricType::Counter,
            ).with_label("level", &level_str)
              .with_value(MetricValue::Counter(level_stats.task_count))).await;
        }

        // 最后一次运行时间
        if let Some(last_run) = downsample_stats.last_run_timestamp {
            self.registry.register(Metric::new(
                "chronodb_downsample_last_run_timestamp",
                "Timestamp of the last downsample run",
                MetricType::Gauge,
            ).with_value(MetricValue::Gauge(last_run as f64))).await;
        }

        Ok(())
    }

    /// 记录查询延迟
    pub async fn record_query_latency(&self, latency_ms: f64) {
        let mut latency = self.query_latency.write().await;
        latency.push(latency_ms);
        // 只保留最近1000个值
        if latency.len() > 1000 {
            let len = latency.len();
            if len > 1000 {
                latency.drain(0..len - 1000);
            }
        }
    }

    /// 记录写入延迟
    pub async fn record_write_latency(&self, latency_ms: f64) {
        let mut latency = self.write_latency.write().await;
        latency.push(latency_ms);
        // 只保留最近1000个值
        if latency.len() > 1000 {
            let len = latency.len();
            if len > 1000 {
                latency.drain(0..len - 1000);
            }
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

    /// 收集分层存储指标
    pub async fn collect_tiered_storage_metrics(&self, tiered_manager: &crate::tiered::manager::TieredStorageManager) -> Result<()> {
        // 收集分层存储统计信息
        let stats = tiered_manager.collect_stats().await?;
        
        // 总系列数
        self.registry.register(Metric::new(
            "chronodb_tiered_series_total",
            "Total number of series in tiered storage",
            MetricType::Gauge,
        ).with_value(MetricValue::Gauge(stats.total_series as f64))).await;

        // 总样本数
        self.registry.register(Metric::new(
            "chronodb_tiered_samples_total",
            "Total number of samples in tiered storage",
            MetricType::Gauge,
        ).with_value(MetricValue::Gauge(stats.total_samples as f64))).await;

        // 总存储字节数
        self.registry.register(Metric::new(
            "chronodb_tiered_storage_bytes",
            "Total storage size in bytes for tiered storage",
            MetricType::Gauge,
        ).with_value(MetricValue::Gauge(stats.total_bytes as f64))).await;

        // 每层的统计信息
        for (tier_name, tier_stats) in stats.tier_stats {
            // 系列数
            self.registry.register(Metric::new(
                "chronodb_tier_series_total",
                "Number of series per tier",
                MetricType::Gauge,
            ).with_label("tier", &tier_name)
              .with_value(MetricValue::Gauge(tier_stats.series_count as f64))).await;

            // 样本数
            self.registry.register(Metric::new(
                "chronodb_tier_samples_total",
                "Number of samples per tier",
                MetricType::Gauge,
            ).with_label("tier", &tier_name)
              .with_value(MetricValue::Gauge(tier_stats.sample_count as f64))).await;

            // 存储字节数
            self.registry.register(Metric::new(
                "chronodb_tier_storage_bytes",
                "Storage size in bytes per tier",
                MetricType::Gauge,
            ).with_label("tier", &tier_name)
              .with_value(MetricValue::Gauge(tier_stats.total_bytes as f64))).await;

            // 文件数
            self.registry.register(Metric::new(
                "chronodb_tier_file_count",
                "Number of files per tier",
                MetricType::Gauge,
            ).with_label("tier", &tier_name)
              .with_value(MetricValue::Gauge(tier_stats.file_count as f64))).await;
        }

        // 最后迁移时间
        if let Some(last_migration) = stats.last_migration_time {
            self.registry.register(Metric::new(
                "chronodb_tiered_last_migration_timestamp",
                "Timestamp of the last data migration",
                MetricType::Gauge,
            ).with_value(MetricValue::Gauge(last_migration as f64))).await;
        }

        Ok(())
    }

    /// 收集 API 性能指标
    pub async fn collect_api_metrics(&self, api_stats: &ApiStats) -> Result<()> {
        // API 请求总数
        self.registry.register(Metric::new(
            "chronodb_api_requests_total",
            "Total number of API requests",
            MetricType::Counter,
        ).with_value(MetricValue::Counter(api_stats.total_requests))).await;

        // 成功请求数
        self.registry.register(Metric::new(
            "chronodb_api_requests_successful_total",
            "Number of successful API requests",
            MetricType::Counter,
        ).with_value(MetricValue::Counter(api_stats.successful_requests))).await;

        // 失败请求数
        self.registry.register(Metric::new(
            "chronodb_api_requests_failed_total",
            "Number of failed API requests",
            MetricType::Counter,
        ).with_value(MetricValue::Counter(api_stats.failed_requests))).await;

        // 平均请求延迟
        self.registry.register(Metric::new(
            "chronodb_api_request_latency_ms",
            "Average API request latency in milliseconds",
            MetricType::Gauge,
        ).with_value(MetricValue::Gauge(api_stats.average_latency_ms))).await;

        // 按端点统计
        for (endpoint, endpoint_stats) in &api_stats.endpoint_stats {
            // 端点请求数
            self.registry.register(Metric::new(
                "chronodb_api_endpoint_requests_total",
                "Number of requests per API endpoint",
                MetricType::Counter,
            ).with_label("endpoint", endpoint)
              .with_value(MetricValue::Counter(endpoint_stats.requests))).await;

            // 端点失败数
            self.registry.register(Metric::new(
                "chronodb_api_endpoint_requests_failed_total",
                "Number of failed requests per API endpoint",
                MetricType::Counter,
            ).with_label("endpoint", endpoint)
              .with_value(MetricValue::Counter(endpoint_stats.failed_requests))).await;

            // 端点平均延迟
            self.registry.register(Metric::new(
                "chronodb_api_endpoint_latency_ms",
                "Average latency per API endpoint",
                MetricType::Gauge,
            ).with_label("endpoint", endpoint)
              .with_value(MetricValue::Gauge(endpoint_stats.average_latency_ms))).await;
        }

        Ok(())
    }

    /// 添加默认告警规则
    pub async fn add_default_alert_rules(&self) {
        // 系列数量告警
        self.registry.add_alert_rule(AlertRule {
            name: "HighSeriesCount".to_string(),
            expr: "chronodb_series_total > 1000000".to_string(),
            for_duration: Duration::from_mins(5),
            labels: HashMap::from([("severity".to_string(), "warning".to_string())]),
            annotations: HashMap::from([
                ("summary".to_string(), "High series count".to_string()),
                ("description".to_string(), "The number of time series has exceeded 1,000,000".to_string()),
            ]),
        }).await;

        // 查询延迟告警
        self.registry.add_alert_rule(AlertRule {
            name: "HighQueryLatency".to_string(),
            expr: "chronodb_query_latency_ms > 1000".to_string(),
            for_duration: Duration::from_mins(5),
            labels: HashMap::from([("severity".to_string(), "warning".to_string())]),
            annotations: HashMap::from([
                ("summary".to_string(), "High query latency".to_string()),
                ("description".to_string(), "Average query latency has exceeded 1000ms".to_string()),
            ]),
        }).await;

        // 节点健康告警
        self.registry.add_alert_rule(AlertRule {
            name: "ClusterNodeDown".to_string(),
            expr: "chronodb_cluster_healthy_nodes_total < chronodb_cluster_nodes_total".to_string(),
            for_duration: Duration::from_mins(2),
            labels: HashMap::from([("severity".to_string(), "critical".to_string())]),
            annotations: HashMap::from([
                ("summary".to_string(), "Cluster node down".to_string()),
                ("description".to_string(), "One or more nodes in the cluster are down".to_string()),
            ]),
        }).await;

        // 分层存储告警
        // 热层容量告警
        self.registry.add_alert_rule(AlertRule {
            name: "HotTierFull".to_string(),
            expr: "chronodb_tier_storage_bytes{ tier=\"hot\" } / 1024 / 1024 / 1024 > 9".to_string(), // 假设热层最大容量为10GB
            for_duration: Duration::from_mins(5),
            labels: HashMap::from([("severity".to_string(), "warning".to_string())]),
            annotations: HashMap::from([
                ("summary".to_string(), "Hot tier nearly full".to_string()),
                ("description".to_string(), "The hot tier is nearly full (over 9GB)".to_string()),
            ]),
        }).await;

        // API 性能告警
        // API 延迟告警
        self.registry.add_alert_rule(AlertRule {
            name: "HighApiLatency".to_string(),
            expr: "chronodb_api_request_latency_ms > 500".to_string(),
            for_duration: Duration::from_mins(5),
            labels: HashMap::from([("severity".to_string(), "warning".to_string())]),
            annotations: HashMap::from([
                ("summary".to_string(), "High API latency".to_string()),
                ("description".to_string(), "Average API request latency has exceeded 500ms".to_string()),
            ]),
        }).await;

        // API 错误率告警
        self.registry.add_alert_rule(AlertRule {
            name: "HighApiErrorRate".to_string(),
            expr: "rate(chronodb_api_requests_failed_total[5m]) / rate(chronodb_api_requests_total[5m]) > 0.05".to_string(), // 5% 错误率
            for_duration: Duration::from_mins(5),
            labels: HashMap::from([("severity".to_string(), "warning".to_string())]),
            annotations: HashMap::from([
                ("summary".to_string(), "High API error rate".to_string()),
                ("description".to_string(), "API error rate has exceeded 5%".to_string()),
            ]),
        }).await;

        // 存储容量告警
        self.registry.add_alert_rule(AlertRule {
            name: "StorageFull".to_string(),
            expr: "chronodb_storage_bytes / 1024 / 1024 / 1024 > 90".to_string(), // 假设总存储容量为100GB
            for_duration: Duration::from_mins(5),
            labels: HashMap::from([("severity".to_string(), "critical".to_string())]),
            annotations: HashMap::from([
                ("summary".to_string(), "Storage nearly full".to_string()),
                ("description".to_string(), "Total storage usage has exceeded 90GB".to_string()),
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
            name: "TestAlert".to_string(),
            expr: "test_metric > 100".to_string(),
            for_duration: Duration::from_mins(5),
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
