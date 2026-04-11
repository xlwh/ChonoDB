use std::collections::HashMap;
use std::sync::Arc;
use parking_lot::RwLock;
use crate::model::QueryFrequencyStats;
use crate::error::Result;

pub struct FrequencyTracker {
    stats: Arc<RwLock<HashMap<String, QueryFrequencyStats>>>,
    config: FrequencyConfig,
}

#[derive(Debug, Clone)]
pub struct FrequencyConfig {
    pub window_size_hours: u64,
    pub frequency_threshold: u64,
    pub cleanup_interval_hours: u64,
    pub max_tracked_queries: usize,
}

impl Default for FrequencyConfig {
    fn default() -> Self {
        Self {
            window_size_hours: 24,
            frequency_threshold: 20,
            cleanup_interval_hours: 1,
            max_tracked_queries: 10000,
        }
    }
}

impl FrequencyTracker {
    pub fn new(config: FrequencyConfig) -> Self {
        Self {
            stats: Arc::new(RwLock::new(HashMap::new())),
            config,
        }
    }

    pub fn record_query(&self, query: &str) {
        let normalized = normalize_query(query);
        
        let mut stats = self.stats.write();
        
        if let Some(query_stats) = stats.get_mut(&normalized) {
            query_stats.record_query(query.to_string());
        } else {
            if stats.len() >= self.config.max_tracked_queries {
                self.cleanup_low_frequency_queries(&mut stats);
            }
            
            let mut query_stats = QueryFrequencyStats::new(
                normalized.clone(),
                self.config.window_size_hours,
            );
            query_stats.record_query(query.to_string());
            stats.insert(normalized, query_stats);
        }
    }

    fn cleanup_low_frequency_queries(&self, stats: &mut HashMap<String, QueryFrequencyStats>) {
        let threshold = self.config.frequency_threshold / 2;
        stats.retain(|_, s| s.frequency > threshold);
    }

    pub fn get_frequency(&self, query: &str) -> u64 {
        let normalized = normalize_query(query);
        let stats = self.stats.read();
        stats.get(&normalized).map(|s| s.frequency).unwrap_or(0)
    }

    pub fn get_frequency_per_hour(&self, query: &str) -> f64 {
        let normalized = normalize_query(query);
        let stats = self.stats.read();
        stats.get(&normalized).map(|s| s.get_frequency_per_hour()).unwrap_or(0.0)
    }

    pub fn get_high_frequency_queries(&self) -> Vec<(String, u64)> {
        let stats = self.stats.read();
        stats.iter()
            .filter(|(_, s)| s.frequency >= self.config.frequency_threshold)
            .map(|(k, s)| (k.clone(), s.frequency))
            .collect()
    }

    pub fn get_all_stats(&self) -> HashMap<String, QueryFrequencyStats> {
        self.stats.read().clone()
    }

    pub fn get_query_stats(&self, query: &str) -> Option<QueryFrequencyStats> {
        let normalized = normalize_query(query);
        self.stats.read().get(&normalized).cloned()
    }

    pub fn clear_stats(&self) {
        self.stats.write().clear();
    }

    pub fn cleanup_expired(&self) {
        let mut stats = self.stats.write();
        stats.iter_mut().for_each(|(_, s)| {
            s.cleanup_old_records();
        });
        stats.retain(|_, s| s.frequency > 0);
    }
}

pub fn normalize_query(query: &str) -> String {
    let mut normalized = query.trim().to_string();
    
    normalized = normalized.replace('\n', " ");
    normalized = normalized.replace('\t', " ");
    
    while normalized.contains("  ") {
        normalized = normalized.replace("  ", " ");
    }
    
    normalized = remove_time_parameters(&normalized);
    
    normalized = sort_label_matchers(&normalized);
    
    normalized = simplify_constants(&normalized);
    
    normalized.trim().to_string()
}

fn remove_time_parameters(query: &str) -> String {
    let re = regex::Regex::new(r"\[\d+[smhdwy]\]").unwrap();
    re.replace_all(query, "[DURATION]").to_string()
}

fn sort_label_matchers(query: &str) -> String {
    let re = regex::Regex::new(r"\{([^}]+)\}").unwrap();
    
    re.replace_all(query, |caps: &regex::Captures| {
        let labels_str = &caps[1];
        let mut labels: Vec<&str> = labels_str.split(',').map(|s| s.trim()).collect();
        labels.sort();
        format!("{{{}}}", labels.join(", "))
    }).to_string()
}

fn simplify_constants(query: &str) -> String {
    let mut result = query.to_string();
    
    let replacements = [
        ("true", "TRUE"),
        ("false", "FALSE"),
        ("inf", "INF"),
        ("nan", "NAN"),
    ];
    
    for (from, to) in replacements {
        let re = regex::Regex::new(&format!(r"\b{}\b", from)).unwrap();
        result = re.replace_all(&result, to).to_string();
    }
    
    result
}

pub fn extract_query_pattern(query: &str) -> QueryPattern {
    let normalized = normalize_query(query);
    
    let has_aggregation = contains_aggregation(&normalized);
    let has_rate = contains_rate(&normalized);
    let has_sum = contains_sum(&normalized);
    let metric_names = extract_metric_names(&normalized);
    let label_filters = extract_label_filters(&normalized);
    
    QueryPattern {
        normalized_query: normalized,
        has_aggregation,
        has_rate,
        has_sum,
        metric_names,
        label_filters,
    }
}

#[derive(Debug, Clone)]
pub struct QueryPattern {
    pub normalized_query: String,
    pub has_aggregation: bool,
    pub has_rate: bool,
    pub has_sum: bool,
    pub metric_names: Vec<String>,
    pub label_filters: Vec<(String, String)>,
}

fn contains_aggregation(query: &str) -> bool {
    let aggregation_funcs = [
        "sum(", "avg(", "min(", "max(", "count(", 
        "stddev(", "stdvar(", "var(", "group(",
        "sum by", "avg by", "min by", "max by", "count by",
    ];
    
    aggregation_funcs.iter().any(|func| 
        query.to_lowercase().contains(func)
    )
}

fn contains_rate(query: &str) -> bool {
    query.to_lowercase().contains("rate(") || 
    query.to_lowercase().contains("irate(") ||
    query.to_lowercase().contains("increase(")
}

fn contains_sum(query: &str) -> bool {
    query.to_lowercase().contains("sum(") || 
    query.to_lowercase().contains("sum by")
}

fn extract_metric_names(query: &str) -> Vec<String> {
    let re = regex::Regex::new(r"\b([a-zA-Z_][a-zA-Z0-9_]*)\b").unwrap();
    
    re.captures_iter(query)
        .filter_map(|cap| {
            let name = cap[1].to_string();
            let keywords = [
                "sum", "avg", "min", "max", "count", "rate", "irate", 
                "increase", "by", "without", "group", "stddev", "stdvar",
                "and", "or", "unless", "offset", "bool", "on", "ignoring",
            ];
            
            if !keywords.contains(&name.to_lowercase().as_str()) {
                Some(name)
            } else {
                None
            }
        })
        .collect()
}

fn extract_label_filters(query: &str) -> Vec<(String, String)> {
    let re = regex::Regex::new(r#"([a-zA-Z_][a-zA-Z0-9_]*)\s*[=~=!]+\s*"([^"]+)""#).unwrap();
    
    re.captures_iter(query)
        .map(|cap| (cap[1].to_string(), cap[2].to_string()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_query() {
        let query = "sum(rate(http_requests_total{job=\"api\"}[5m])) by (status)";
        let normalized = normalize_query(query);
        
        assert!(normalized.contains("[DURATION]"));
        assert!(normalized.contains("job=\"api\""));
    }

    #[test]
    fn test_frequency_tracker() {
        let tracker = FrequencyTracker::new(FrequencyConfig::default());
        
        tracker.record_query("up");
        tracker.record_query("up");
        tracker.record_query("up");
        
        assert_eq!(tracker.get_frequency("up"), 3);
    }

    #[test]
    fn test_extract_query_pattern() {
        let query = "sum(rate(http_requests_total{job=\"api\"}[5m])) by (status)";
        let pattern = extract_query_pattern(query);
        
        assert!(pattern.has_aggregation);
        assert!(pattern.has_rate);
        assert!(pattern.has_sum);
        assert!(pattern.metric_names.contains(&"http_requests_total".to_string()));
    }

    #[test]
    fn test_high_frequency_queries() {
        let config = FrequencyConfig {
            frequency_threshold: 2,
            ..Default::default()
        };
        let tracker = FrequencyTracker::new(config);
        
        tracker.record_query("query1");
        tracker.record_query("query1");
        tracker.record_query("query1");
        
        tracker.record_query("query2");
        
        let high_freq = tracker.get_high_frequency_queries();
        assert_eq!(high_freq.len(), 1);
        assert_eq!(high_freq[0].0, "query1");
    }
}
