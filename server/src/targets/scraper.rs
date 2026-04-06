use super::{Target, TargetHealth};
use std::time::Duration;
use tracing::{debug, error, info, warn};

/// 抓取器
#[derive(Clone)]
pub struct Scraper {
    client: reqwest::Client,
}

impl Scraper {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");
        
        Self { client }
    }
    
    /// 抓取目标
    pub async fn scrape(&self, target: &Target) -> Result<Vec<u8>, String> {
        debug!("Scraping target: {} at {}", target.name, target.url);
        
        let timeout = Duration::from_secs(target.scrape_timeout);
        
        let response = match tokio::time::timeout(
            timeout,
            self.client.get(&target.url).send()
        ).await {
            Ok(Ok(resp)) => resp,
            Ok(Err(e)) => {
                return Err(format!("Request failed: {}", e));
            }
            Err(_) => {
                return Err("Request timeout".to_string());
            }
        };
        
        if !response.status().is_success() {
            return Err(format!("HTTP error: {}", response.status()));
        }
        
        let body = match response.bytes().await {
            Ok(bytes) => bytes.to_vec(),
            Err(e) => {
                return Err(format!("Failed to read response: {}", e));
            }
        };
        
        Ok(body)
    }
    
    /// 解析 Prometheus 格式的指标数据
    pub fn parse_metrics(&self, data: &[u8]) -> Result<Vec<ParsedMetric>, String> {
        let content = String::from_utf8_lossy(data);
        let mut metrics = Vec::new();
        
        for line in content.lines() {
            let line = line.trim();
            
            // 跳过空行和注释
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            
            // 解析指标行
            match self.parse_metric_line(line) {
                Ok(metric) => metrics.push(metric),
                Err(e) => {
                    debug!("Failed to parse metric line: {} - {}", line, e);
                }
            }
        }
        
        Ok(metrics)
    }
    
    fn parse_metric_line(&self, line: &str) -> Result<ParsedMetric, String> {
        // 简单的指标解析实现
        // 格式: metric_name{label1="value1",label2="value2"} value timestamp
        
        let mut parts = line.split_whitespace();
        
        let name_and_labels = parts.next()
            .ok_or("Missing metric name")?;
        
        let value_str = parts.next()
            .ok_or("Missing metric value")?;
        
        let timestamp = parts.next()
            .and_then(|s| s.parse::<i64>().ok());
        
        // 解析名称和标签
        let (name, labels) = if let Some(bracket_pos) = name_and_labels.find('{') {
            let name = &name_and_labels[..bracket_pos];
            let labels_str = &name_and_labels[bracket_pos..];
            let labels = self.parse_labels(labels_str)?;
            (name.to_string(), labels)
        } else {
            (name_and_labels.to_string(), std::collections::HashMap::new())
        };
        
        let value = value_str.parse::<f64>()
            .map_err(|e| format!("Invalid value: {}", e))?;
        
        Ok(ParsedMetric {
            name,
            labels,
            value,
            timestamp,
        })
    }
    
    fn parse_labels(&self, labels_str: &str) -> Result<std::collections::HashMap<String, String>, String> {
        let mut labels = std::collections::HashMap::new();
        
        // 移除花括号
        let content = labels_str.trim_start_matches('{').trim_end_matches('}');
        
        // 解析标签对
        for pair in content.split(',') {
            let pair = pair.trim();
            if pair.is_empty() {
                continue;
            }
            
            let mut kv = pair.splitn(2, '=');
            let key = kv.next()
                .ok_or("Missing label key")?
                .trim()
                .to_string();
            let value = kv.next()
                .ok_or("Missing label value")?
                .trim()
                .trim_matches('"')
                .to_string();
            
            labels.insert(key, value);
        }
        
        Ok(labels)
    }
}

impl Default for Scraper {
    fn default() -> Self {
        Self::new()
    }
}

/// 解析后的指标
#[derive(Debug, Clone)]
pub struct ParsedMetric {
    pub name: String,
    pub labels: std::collections::HashMap<String, String>,
    pub value: f64,
    pub timestamp: Option<i64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_metric_line() {
        let scraper = Scraper::new();
        
        // 测试简单指标
        let line = "http_requests_total 1027";
        let metric = scraper.parse_metric_line(line).unwrap();
        assert_eq!(metric.name, "http_requests_total");
        assert_eq!(metric.value, 1027.0);
        assert!(metric.labels.is_empty());
        
        // 测试带标签的指标
        let line = "http_requests_total{method=\"GET\",status=\"200\"} 1027";
        let metric = scraper.parse_metric_line(line).unwrap();
        assert_eq!(metric.name, "http_requests_total");
        assert_eq!(metric.value, 1027.0);
        assert_eq!(metric.labels.get("method"), Some(&"GET".to_string()));
        assert_eq!(metric.labels.get("status"), Some(&"200".to_string()));
        
        // 测试带时间戳的指标
        let line = "http_requests_total 1027 1234567890";
        let metric = scraper.parse_metric_line(line).unwrap();
        assert_eq!(metric.name, "http_requests_total");
        assert_eq!(metric.value, 1027.0);
        assert_eq!(metric.timestamp, Some(1234567890));
    }

    #[test]
    fn test_parse_metrics() {
        let scraper = Scraper::new();
        
        let data = r#"
# HELP http_requests_total Total HTTP requests
# TYPE http_requests_total counter
http_requests_total{method="GET",status="200"} 1027
http_requests_total{method="POST",status="200"} 3

# HELP cpu_usage CPU usage
# TYPE cpu_usage gauge
cpu_usage 45.2
"#;
        
        let metrics = scraper.parse_metrics(data.as_bytes()).unwrap();
        assert_eq!(metrics.len(), 3);
        
        assert_eq!(metrics[0].name, "http_requests_total");
        assert_eq!(metrics[0].value, 1027.0);
        
        assert_eq!(metrics[1].name, "http_requests_total");
        assert_eq!(metrics[1].value, 3.0);
        
        assert_eq!(metrics[2].name, "cpu_usage");
        assert_eq!(metrics[2].value, 45.2);
    }
}
