use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use super::{Label, Timestamp};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RuleStatus {
    Active,
    Inactive,
    Pending,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreAggregationRule {
    pub id: String,
    pub name: String,
    pub expr: String,
    pub labels: HashMap<String, String>,
    pub is_auto_created: bool,
    pub created_at: Timestamp,
    pub query_frequency: u64,
    pub last_query_time: Timestamp,
    pub last_evaluation: Timestamp,
    pub status: RuleStatus,
    pub evaluation_interval: u64,
    pub retention_days: u32,
}

impl PreAggregationRule {
    pub fn new(
        id: String,
        name: String,
        expr: String,
        labels: HashMap<String, String>,
        is_auto_created: bool,
    ) -> Self {
        let now = chrono::Utc::now().timestamp_millis();
        Self {
            id,
            name,
            expr,
            labels,
            is_auto_created,
            created_at: now,
            query_frequency: 0,
            last_query_time: 0,
            last_evaluation: 0,
            status: RuleStatus::Pending,
            evaluation_interval: 60,
            retention_days: 30,
        }
    }

    pub fn update_query_frequency(&mut self) {
        self.query_frequency += 1;
        self.last_query_time = chrono::Utc::now().timestamp_millis();
    }

    pub fn update_evaluation(&mut self) {
        self.last_evaluation = chrono::Utc::now().timestamp_millis();
    }

    pub fn set_status(&mut self, status: RuleStatus) {
        self.status = status;
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryRecord {
    pub timestamp: Timestamp,
    pub query: String,
}

impl QueryRecord {
    pub fn new(query: String) -> Self {
        Self {
            timestamp: chrono::Utc::now().timestamp_millis(),
            query,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryFrequencyStats {
    pub normalized_query: String,
    pub frequency: u64,
    pub last_query_time: Timestamp,
    pub query_count_window: Vec<QueryRecord>,
    pub window_size_hours: u64,
}

impl QueryFrequencyStats {
    pub fn new(normalized_query: String, window_size_hours: u64) -> Self {
        Self {
            normalized_query,
            frequency: 0,
            last_query_time: 0,
            query_count_window: Vec::new(),
            window_size_hours,
        }
    }

    pub fn record_query(&mut self, query: String) {
        let record = QueryRecord::new(query);
        self.query_count_window.push(record);
        self.frequency += 1;
        self.last_query_time = chrono::Utc::now().timestamp_millis();
        self.cleanup_old_records();
    }

    fn cleanup_old_records(&mut self) {
        let window_millis = self.window_size_hours * 60 * 60 * 1000;
        let now = chrono::Utc::now().timestamp_millis();
        let cutoff = now - window_millis as i64;
        
        self.query_count_window.retain(|r| r.timestamp > cutoff);
        self.frequency = self.query_count_window.len() as u64;
    }

    pub fn get_frequency_per_hour(&self) -> f64 {
        if self.query_count_window.is_empty() {
            return 0.0;
        }
        
        let window_hours = self.window_size_hours as f64;
        self.frequency as f64 / window_hours
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LabelMatcher {
    pub name: String,
    pub value: String,
    pub match_type: MatchType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MatchType {
    Exact,
    Regex,
    NotEqual,
    NotRegex,
}

impl LabelMatcher {
    pub fn new(name: String, value: String, match_type: MatchType) -> Self {
        Self {
            name,
            value,
            match_type,
        }
    }

    pub fn exact(name: String, value: String) -> Self {
        Self::new(name, value, MatchType::Exact)
    }

    pub fn matches(&self, label: &Label) -> bool {
        if self.name != label.name {
            return false;
        }
        
        match self.match_type {
            MatchType::Exact => self.value == label.value,
            MatchType::Regex => {
                let re = regex::Regex::new(&self.value).unwrap();
                re.is_match(&label.value)
            }
            MatchType::NotEqual => self.value != label.value,
            MatchType::NotRegex => {
                let re = regex::Regex::new(&self.value).unwrap();
                !re.is_match(&label.value)
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataLocation {
    pub series_id: u64,
    pub start_time: Timestamp,
    pub end_time: Timestamp,
    pub sample_count: u64,
    pub storage_path: String,
}

impl DataLocation {
    pub fn new(series_id: u64, start_time: Timestamp, end_time: Timestamp) -> Self {
        Self {
            series_id,
            start_time,
            end_time,
            sample_count: 0,
            storage_path: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreAggregationIndex {
    pub rule_id: String,
    pub query_pattern: String,
    pub label_matchers: Vec<LabelMatcher>,
    pub data_location: DataLocation,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

impl PreAggregationIndex {
    pub fn new(
        rule_id: String,
        query_pattern: String,
        label_matchers: Vec<LabelMatcher>,
        data_location: DataLocation,
    ) -> Self {
        let now = chrono::Utc::now().timestamp_millis();
        Self {
            rule_id,
            query_pattern,
            label_matchers,
            data_location,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn update(&mut self, data_location: DataLocation) {
        self.data_location = data_location;
        self.updated_at = chrono::Utc::now().timestamp_millis();
    }

    pub fn matches_query(&self, query: &str, labels: &[Label]) -> bool {
        if !self.query_pattern.contains(query) && query != self.query_pattern {
            return false;
        }
        
        for matcher in &self.label_matchers {
            let matched = labels.iter().any(|l| matcher.matches(l));
            if !matched {
                return false;
            }
        }
        
        true
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteDecision {
    pub use_preaggregated: bool,
    pub rule_id: Option<String>,
    pub rule_name: Option<String>,
    pub match_score: f64,
}

impl RouteDecision {
    pub fn preaggregated(rule_id: String, rule_name: String, match_score: f64) -> Self {
        Self {
            use_preaggregated: true,
            rule_id: Some(rule_id),
            rule_name: Some(rule_name),
            match_score,
        }
    }

    pub fn original() -> Self {
        Self {
            use_preaggregated: false,
            rule_id: None,
            rule_name: None,
            match_score: 0.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pre_aggregation_rule_creation() {
        let rule = PreAggregationRule::new(
            "rule-1".to_string(),
            "test_rule".to_string(),
            "sum(rate(http_requests_total[5m]))".to_string(),
            HashMap::new(),
            false,
        );
        
        assert_eq!(rule.id, "rule-1");
        assert_eq!(rule.name, "test_rule");
        assert_eq!(rule.status, RuleStatus::Pending);
        assert!(!rule.is_auto_created);
    }

    #[test]
    fn test_query_frequency_stats() {
        let mut stats = QueryFrequencyStats::new("test_query".to_string(), 24);
        
        stats.record_query("test_query".to_string());
        stats.record_query("test_query".to_string());
        
        assert_eq!(stats.frequency, 2);
        assert!(stats.last_query_time > 0);
    }

    #[test]
    fn test_label_matcher() {
        let matcher = LabelMatcher::exact("job".to_string(), "prometheus".to_string());
        let label = Label::new("job", "prometheus");
        
        assert!(matcher.matches(&label));
        
        let label2 = Label::new("job", "grafana");
        assert!(!matcher.matches(&label2));
    }

    #[test]
    fn test_route_decision() {
        let decision = RouteDecision::preaggregated(
            "rule-1".to_string(),
            "test_rule".to_string(),
            0.95,
        );
        
        assert!(decision.use_preaggregated);
        assert_eq!(decision.rule_id, Some("rule-1".to_string()));
        assert_eq!(decision.match_score, 0.95);
    }
}
