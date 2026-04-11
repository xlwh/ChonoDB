use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 监控指标类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum MetricType {
    Gauge,
    Counter,
    Histogram,
    Summary,
}

/// 监控指标
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metric {
    pub name: String,
    pub metric_type: MetricType,
    pub help: String,
    pub labels: Vec<(String, String)>,
    pub unit: Option<String>,
}

/// 监控指标管理器
#[derive(Debug, Clone)]
pub struct MetricManager {
    metrics: HashMap<String, Metric>,
}

impl MetricManager {
    pub fn new() -> Self {
        Self {
            metrics: HashMap::new(),
        }
    }

    pub fn add_metric(&mut self, metric: Metric) {
        self.metrics.insert(metric.name.clone(), metric);
    }

    pub fn get_metric(&self, name: &str) -> Option<&Metric> {
        self.metrics.get(name)
    }

    pub fn get_all_metrics(&self) -> Vec<&Metric> {
        self.metrics.values().collect()
    }

    pub fn get_metrics_by_type(&self, metric_type: MetricType) -> Vec<&Metric> {
        self.metrics
            .values()
            .filter(|m| m.metric_type == metric_type)
            .collect()
    }

    pub fn remove_metric(&mut self, name: &str) -> Option<Metric> {
        self.metrics.remove(name)
    }

    pub fn metric_count(&self) -> usize {
        self.metrics.len()
    }
}

impl Default for MetricManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metric_manager() {
        let mut manager = MetricManager::new();

        let metric = Metric {
            name: "http_requests_total".to_string(),
            metric_type: MetricType::Counter,
            help: "Total number of HTTP requests".to_string(),
            labels: vec![("method".to_string(), "GET".to_string()), ("status".to_string(), "200".to_string())],
            unit: None,
        };

        manager.add_metric(metric);

        assert_eq!(manager.metric_count(), 1);
        assert!(manager.get_metric("http_requests_total").is_some());
        assert_eq!(manager.get_metrics_by_type(MetricType::Counter).len(), 1);

        manager.remove_metric("http_requests_total");
        assert_eq!(manager.metric_count(), 0);
        assert!(manager.get_metric("http_requests_total").is_none());
    }
}