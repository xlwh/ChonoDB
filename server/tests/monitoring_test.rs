use chronodb_server::monitoring::{MonitoringSystem, MonitoringConfig, Metric, MetricType, AlertRule, AlertLevel};
use std::time::Duration;

/// 测试监控系统基本功能
#[tokio::test]
async fn test_monitoring_system() {
    let config = MonitoringConfig {
        enabled: true,
        metrics_interval_secs: 1,
        alert_evaluation_interval_secs: 1,
        retention_days: 7,
    };

    let monitoring = MonitoringSystem::new(config);

    // 记录一些测试指标
    let metric1 = Metric::new("test_counter", MetricType::Counter, "Test counter metric", 100.0)
        .with_label("env", "test");
    
    let metric2 = Metric::new("test_gauge", MetricType::Gauge, "Test gauge metric", 50.0)
        .with_label("env", "test");
    
    monitoring.collector.record(metric1).await;
    monitoring.collector.record(metric2).await;

    // 获取所有指标
    let metrics = monitoring.collector.get_all_metrics().await;
    assert_eq!(metrics.len(), 2);

    // 导出为 Prometheus 格式
    let prometheus_output = monitoring.collector.export_prometheus().await;
    println!("Prometheus output:\n{}", prometheus_output);
    
    assert!(prometheus_output.contains("test_counter"));
    assert!(prometheus_output.contains("test_gauge"));
    assert!(prometheus_output.contains("# HELP"));
    assert!(prometheus_output.contains("# TYPE"));

    println!("Monitoring system test completed successfully!");
}

/// 测试告警管理
#[tokio::test]
async fn test_alert_management() {
    let config = MonitoringConfig::default();
    let monitoring = MonitoringSystem::new(config);

    // 添加告警规则
    let rule = AlertRule::new(
        "HighTestMetric",
        "test_metric > 80",
        Duration::from_secs(60),
        AlertLevel::Warning,
    )
    .with_label("severity", "warning")
    .with_annotation("summary", "Test metric is high");

    monitoring.alert_manager.add_rule(rule).await;

    // 获取告警规则
    let rules = monitoring.alert_manager.get_rules().await;
    assert_eq!(rules.len(), 1);
    assert_eq!(rules[0].name, "HighTestMetric");
    assert_eq!(rules[0].level, AlertLevel::Warning);

    // 记录一个会触发告警的指标
    let metric = Metric::new("test_metric", MetricType::Gauge, "Test metric", 90.0);
    monitoring.collector.record(metric).await;

    // 获取告警（此时应该还没有告警，因为评估是异步的）
    let alerts = monitoring.alert_manager.get_alerts().await;
    println!("Current alerts: {:?}", alerts);

    println!("Alert management test completed successfully!");
}

/// 测试默认告警规则
#[tokio::test]
async fn test_default_alert_rules() {
    let config = MonitoringConfig::default();
    let monitoring = MonitoringSystem::new(config);

    // 添加默认告警规则
    monitoring.add_default_rules().await;

    // 获取告警规则
    let rules = monitoring.alert_manager.get_rules().await;
    assert_eq!(rules.len(), 3);

    // 验证规则名称
    let rule_names: Vec<String> = rules.iter().map(|r| r.name.clone()).collect();
    assert!(rule_names.contains(&"HighCPUUsage".to_string()));
    assert!(rule_names.contains(&"HighMemoryUsage".to_string()));
    assert!(rule_names.contains(&"LowDiskSpace".to_string()));

    // 验证告警级别
    let cpu_rule = rules.iter().find(|r| r.name == "HighCPUUsage").unwrap();
    assert_eq!(cpu_rule.level, AlertLevel::Warning);

    let memory_rule = rules.iter().find(|r| r.name == "HighMemoryUsage").unwrap();
    assert_eq!(memory_rule.level, AlertLevel::Critical);

    println!("Default alert rules test completed successfully!");
}

/// 测试指标标签
#[tokio::test]
async fn test_metric_labels() {
    let config = MonitoringConfig::default();
    let monitoring = MonitoringSystem::new(config);

    // 记录带标签的指标
    let metric = Metric::new("http_requests_total", MetricType::Counter, "Total HTTP requests", 1000.0)
        .with_label("method", "GET")
        .with_label("status", "200")
        .with_label("endpoint", "/api/v1/query");

    monitoring.collector.record(metric).await;

    // 导出并验证标签
    let output = monitoring.collector.export_prometheus().await;
    println!("Prometheus output with labels:\n{}", output);
    
    assert!(output.contains("method=\"GET\""));
    assert!(output.contains("status=\"200\""));
    assert!(output.contains("endpoint=\"/api/v1/query\""));

    println!("Metric labels test completed successfully!");
}

/// 测试监控配置
#[tokio::test]
async fn test_monitoring_config() {
    // 测试默认配置
    let default_config = MonitoringConfig::default();
    assert!(default_config.enabled);
    assert_eq!(default_config.metrics_interval_secs, 15);
    assert_eq!(default_config.alert_evaluation_interval_secs, 30);
    assert_eq!(default_config.retention_days, 7);

    // 测试自定义配置
    let custom_config = MonitoringConfig {
        enabled: false,
        metrics_interval_secs: 60,
        alert_evaluation_interval_secs: 120,
        retention_days: 14,
    };
    
    assert!(!custom_config.enabled);
    assert_eq!(custom_config.metrics_interval_secs, 60);
    assert_eq!(custom_config.alert_evaluation_interval_secs, 120);
    assert_eq!(custom_config.retention_days, 14);

    println!("Monitoring config test completed successfully!");
}
