use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::RwLock;
use tokio::time::interval;
use tracing::{info, warn, error};

/// 监控指标类型
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum MetricType {
    Counter,
    Gauge,
    Histogram,
    Summary,
}

impl std::fmt::Display for MetricType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MetricType::Counter => write!(f, "counter"),
            MetricType::Gauge => write!(f, "gauge"),
            MetricType::Histogram => write!(f, "histogram"),
            MetricType::Summary => write!(f, "summary"),
        }
    }
}

/// 监控指标
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metric {
    pub name: String,
    pub metric_type: MetricType,
    pub help: String,
    pub labels: HashMap<String, String>,
    pub value: f64,
    pub timestamp: SystemTime,
}

impl Metric {
    pub fn new(name: &str, metric_type: MetricType, help: &str, value: f64) -> Self {
        Self {
            name: name.to_string(),
            metric_type,
            help: help.to_string(),
            labels: HashMap::new(),
            value,
            timestamp: SystemTime::now(),
        }
    }

    pub fn with_label(mut self, key: &str, value: &str) -> Self {
        self.labels.insert(key.to_string(), value.to_string());
        self
    }

    pub fn with_labels(mut self, labels: HashMap<String, String>) -> Self {
        self.labels = labels;
        self
    }
}

/// 系统指标收集器
#[derive(Debug, Clone)]
pub struct SystemMetricsCollector {
    metrics: Arc<RwLock<HashMap<String, Metric>>>,
}

impl SystemMetricsCollector {
    pub fn new() -> Self {
        Self {
            metrics: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 记录指标
    pub async fn record(&self, metric: Metric) {
        let mut metrics = self.metrics.write().await;
        let key = format!("{}:{:?}", metric.name, metric.labels);
        metrics.insert(key, metric);
    }

    /// 获取所有指标
    pub async fn get_all_metrics(&self) -> Vec<Metric> {
        let metrics = self.metrics.read().await;
        metrics.values().cloned().collect()
    }

    /// 获取指定指标
    pub async fn get_metric(&self, name: &str) -> Option<Metric> {
        let metrics = self.metrics.read().await;
        metrics.values().find(|m| m.name == name).cloned()
    }

    /// 导出为 Prometheus 格式
    pub async fn export_prometheus(&self) -> String {
        let metrics = self.metrics.read().await;
        let mut output = String::new();
        let mut grouped: HashMap<String, Vec<&Metric>> = HashMap::new();

        // 按指标名称分组
        for metric in metrics.values() {
            grouped.entry(metric.name.clone()).or_default().push(metric);
        }

        // 生成 Prometheus 格式输出
        for (name, metrics) in grouped {
            if let Some(first) = metrics.first() {
                output.push_str(&format!("# HELP {} {}\n", name, first.help));
                output.push_str(&format!("# TYPE {} {}\n", name, first.metric_type));
            }

            for metric in metrics {
                let labels_str = if metric.labels.is_empty() {
                    String::new()
                } else {
                    let labels: Vec<String> = metric.labels
                        .iter()
                        .map(|(k, v)| format!("{}=\"{}\"", k, v))
                        .collect();
                    format!("{{{}}}", labels.join(","))
                };

                output.push_str(&format!("{}{} {}\n", name, labels_str, metric.value));
            }

            output.push('\n');
        }

        output
    }

    /// 启动指标收集任务
    pub async fn start_collection(&self, interval_secs: u64) {
        let metrics = self.metrics.clone();
        
        tokio::spawn(async move {
            let mut ticker = interval(Duration::from_secs(interval_secs));
            
            loop {
                ticker.tick().await;
                
                // 收集系统指标
                let mut metrics_guard = metrics.write().await;
                
                // CPU 使用率（简化实现）
                metrics_guard.insert(
                    "system_cpu_usage:"
                    .to_string(),
                    Metric::new("system_cpu_usage", MetricType::Gauge, "CPU usage percentage", 0.0),
                );
                
                // 内存使用率（简化实现）
                metrics_guard.insert(
                    "system_memory_usage:"
                    .to_string(),
                    Metric::new("system_memory_usage", MetricType::Gauge, "Memory usage percentage", 0.0),
                );
                
                info!("System metrics collected");
            }
        });
    }
}

impl Default for SystemMetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}

/// 告警级别
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum AlertLevel {
    Info,
    Warning,
    Critical,
}

impl std::fmt::Display for AlertLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AlertLevel::Info => write!(f, "info"),
            AlertLevel::Warning => write!(f, "warning"),
            AlertLevel::Critical => write!(f, "critical"),
        }
    }
}

/// 告警规则
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertRule {
    pub name: String,
    pub expr: String,
    pub duration: Duration,
    pub level: AlertLevel,
    pub labels: HashMap<String, String>,
    pub annotations: HashMap<String, String>,
}

impl AlertRule {
    pub fn new(name: &str, expr: &str, duration: Duration, level: AlertLevel) -> Self {
        Self {
            name: name.to_string(),
            expr: expr.to_string(),
            duration,
            level,
            labels: HashMap::new(),
            annotations: HashMap::new(),
        }
    }

    pub fn with_label(mut self, key: &str, value: &str) -> Self {
        self.labels.insert(key.to_string(), value.to_string());
        self
    }

    pub fn with_annotation(mut self, key: &str, value: &str) -> Self {
        self.annotations.insert(key.to_string(), value.to_string());
        self
    }
}

/// 告警状态
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum AlertState {
    Pending,
    Firing,
    Resolved,
}

/// 告警
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alert {
    pub rule_name: String,
    pub state: AlertState,
    pub level: AlertLevel,
    pub labels: HashMap<String, String>,
    pub annotations: HashMap<String, String>,
    pub value: f64,
    pub starts_at: SystemTime,
    pub ends_at: Option<SystemTime>,
}

/// 告警管理器
#[derive(Debug, Clone)]
pub struct AlertManager {
    rules: Arc<RwLock<Vec<AlertRule>>>,
    alerts: Arc<RwLock<Vec<Alert>>>,
    collector: Arc<SystemMetricsCollector>,
}

impl AlertManager {
    pub fn new(collector: Arc<SystemMetricsCollector>) -> Self {
        Self {
            rules: Arc::new(RwLock::new(Vec::new())),
            alerts: Arc::new(RwLock::new(Vec::new())),
            collector,
        }
    }

    /// 添加告警规则
    pub async fn add_rule(&self, rule: AlertRule) {
        let mut rules = self.rules.write().await;
        rules.push(rule);
        info!("Alert rule added: {}", rules.last().unwrap().name);
    }

    /// 获取所有告警规则
    pub async fn get_rules(&self) -> Vec<AlertRule> {
        let rules = self.rules.read().await;
        rules.clone()
    }

    /// 获取所有告警
    pub async fn get_alerts(&self) -> Vec<Alert> {
        let alerts = self.alerts.read().await;
        alerts.clone()
    }

    /// 获取活跃的告警
    pub async fn get_firing_alerts(&self) -> Vec<Alert> {
        let alerts = self.alerts.read().await;
        alerts.iter()
            .filter(|a| a.state == AlertState::Firing)
            .cloned()
            .collect()
    }

    /// 启动告警评估任务
    pub async fn start_evaluation(&self, interval_secs: u64) {
        let rules = self.rules.clone();
        let alerts = self.alerts.clone();
        let collector = self.collector.clone();
        
        tokio::spawn(async move {
            let mut ticker = interval(Duration::from_secs(interval_secs));
            
            loop {
                ticker.tick().await;
                
                let rules_guard = rules.read().await;
                let metrics = collector.get_all_metrics().await;
                let mut alerts_guard = alerts.write().await;
                
                for rule in rules_guard.iter() {
                    // 简化实现：检查指标是否超过阈值
                    // 实际应该解析 expr 表达式
                    if let Some(metric) = metrics.iter().find(|m| rule.expr.contains(&m.name)) {
                        if metric.value > 0.0 {
                            // 检查是否已存在该告警
                            let exists = alerts_guard.iter()
                                .any(|a| a.rule_name == rule.name && a.state != AlertState::Resolved);
                            
                            if !exists {
                                let alert = Alert {
                                    rule_name: rule.name.clone(),
                                    state: AlertState::Firing,
                                    level: rule.level,
                                    labels: rule.labels.clone(),
                                    annotations: rule.annotations.clone(),
                                    value: metric.value,
                                    starts_at: SystemTime::now(),
                                    ends_at: None,
                                };
                                
                                alerts_guard.push(alert);
                                warn!(
                                    "Alert firing: {} - Level: {}, Value: {}",
                                    rule.name, rule.level, metric.value
                                );
                            }
                        }
                    }
                }
                
                drop(alerts_guard);
                drop(rules_guard);
                
                info!("Alert evaluation completed");
            }
        });
    }

    /// 解决告警
    pub async fn resolve_alert(&self, rule_name: &str) {
        let mut alerts = self.alerts.write().await;
        
        for alert in alerts.iter_mut() {
            if alert.rule_name == rule_name && alert.state == AlertState::Firing {
                alert.state = AlertState::Resolved;
                alert.ends_at = Some(SystemTime::now());
                info!("Alert resolved: {}", rule_name);
            }
        }
    }
}

/// 监控配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringConfig {
    pub enabled: bool,
    pub metrics_interval_secs: u64,
    pub alert_evaluation_interval_secs: u64,
    pub retention_days: u32,
}

impl Default for MonitoringConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            metrics_interval_secs: 15,
            alert_evaluation_interval_secs: 30,
            retention_days: 7,
        }
    }
}

/// 监控系统
#[derive(Debug, Clone)]
pub struct MonitoringSystem {
    pub config: MonitoringConfig,
    pub collector: Arc<SystemMetricsCollector>,
    pub alert_manager: Arc<AlertManager>,
}

impl MonitoringSystem {
    pub fn new(config: MonitoringConfig) -> Self {
        let collector = Arc::new(SystemMetricsCollector::new());
        let alert_manager = Arc::new(AlertManager::new(collector.clone()));
        
        Self {
            config,
            collector,
            alert_manager,
        }
    }

    /// 启动监控系统
    pub async fn start(&self) {
        if !self.config.enabled {
            info!("Monitoring system is disabled");
            return;
        }
        
        info!("Starting monitoring system...");
        
        // 启动指标收集
        self.collector.start_collection(self.config.metrics_interval_secs).await;
        
        // 启动告警评估
        self.alert_manager.start_evaluation(self.config.alert_evaluation_interval_secs).await;
        
        info!("Monitoring system started successfully");
    }

    /// 获取 metrics 端点数据
    pub async fn get_metrics(&self) -> String {
        self.collector.export_prometheus().await
    }

    /// 获取告警列表
    pub async fn get_alerts(&self) -> Vec<Alert> {
        self.alert_manager.get_alerts().await
    }

    /// 添加默认告警规则
    pub async fn add_default_rules(&self) {
        // 高 CPU 使用率告警
        let cpu_rule = AlertRule::new(
            "HighCPUUsage",
            "system_cpu_usage > 80",
            Duration::from_secs(300),
            AlertLevel::Warning,
        )
        .with_annotation("summary", "High CPU usage detected")
        .with_annotation("description", "CPU usage has been above 80% for more than 5 minutes");
        
        self.alert_manager.add_rule(cpu_rule).await;
        
        // 高内存使用率告警
        let memory_rule = AlertRule::new(
            "HighMemoryUsage",
            "system_memory_usage > 90",
            Duration::from_secs(300),
            AlertLevel::Critical,
        )
        .with_annotation("summary", "High memory usage detected")
        .with_annotation("description", "Memory usage has been above 90% for more than 5 minutes");
        
        self.alert_manager.add_rule(memory_rule).await;
        
        // 磁盘空间不足告警
        let disk_rule = AlertRule::new(
            "LowDiskSpace",
            "system_disk_free_percent < 10",
            Duration::from_secs(600),
            AlertLevel::Critical,
        )
        .with_annotation("summary", "Low disk space")
        .with_annotation("description", "Disk space is below 10%");
        
        self.alert_manager.add_rule(disk_rule).await;
        
        info!("Default alert rules added");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_system_metrics_collector() {
        let collector = SystemMetricsCollector::new();
        
        // 记录指标
        let metric = Metric::new("test_metric", MetricType::Gauge, "Test metric", 42.0)
            .with_label("env", "test");
        
        collector.record(metric).await;
        
        // 获取指标
        let metrics = collector.get_all_metrics().await;
        assert_eq!(metrics.len(), 1);
        
        let retrieved = collector.get_metric("test_metric").await;
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().value, 42.0);
    }

    #[tokio::test]
    async fn test_alert_manager() {
        let collector = Arc::new(SystemMetricsCollector::new());
        let alert_manager = AlertManager::new(collector.clone());
        
        // 添加告警规则
        let rule = AlertRule::new(
            "TestAlert",
            "test_metric > 10",
            Duration::from_secs(60),
            AlertLevel::Warning,
        );
        
        alert_manager.add_rule(rule).await;
        
        // 获取规则
        let rules = alert_manager.get_rules().await;
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].name, "TestAlert");
    }
}
