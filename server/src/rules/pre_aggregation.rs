use std::collections::HashMap;
use std::sync::Arc;
use parking_lot::RwLock;
use chrono::Utc;
use chronodb_storage::model::{PreAggregationRule, RuleStatus};
use chronodb_storage::query::{FrequencyTracker, FrequencyConfig, normalize_query};
use crate::error::Result;

pub struct PreAggregationManager {
    rules: Arc<RwLock<HashMap<String, PreAggregationRule>>>,
    frequency_tracker: Arc<FrequencyTracker>,
    config: PreAggregationManagerConfig,
}

#[derive(Debug, Clone)]
pub struct PreAggregationManagerConfig {
    pub auto_create_enabled: bool,
    pub frequency_threshold: u64,
    pub auto_cleanup_enabled: bool,
    pub low_frequency_threshold: u64,
    pub observation_period_hours: u64,
    pub max_auto_rules: usize,
}

impl Default for PreAggregationManagerConfig {
    fn default() -> Self {
        Self {
            auto_create_enabled: true,
            frequency_threshold: 20,
            auto_cleanup_enabled: true,
            low_frequency_threshold: 5,
            observation_period_hours: 48,
            max_auto_rules: 100,
        }
    }
}

impl PreAggregationManager {
    pub fn new(frequency_tracker: Arc<FrequencyTracker>) -> Self {
        Self {
            rules: Arc::new(RwLock::new(HashMap::new())),
            frequency_tracker,
            config: PreAggregationManagerConfig::default(),
        }
    }

    pub fn with_config(
        frequency_tracker: Arc<FrequencyTracker>,
        config: PreAggregationManagerConfig,
    ) -> Self {
        Self {
            rules: Arc::new(RwLock::new(HashMap::new())),
            frequency_tracker,
            config,
        }
    }

    pub fn create_rule(
        &self,
        name: String,
        expr: String,
        labels: HashMap<String, String>,
        is_auto_created: bool,
    ) -> Result<String> {
        let id = generate_rule_id();
        let mut rule = PreAggregationRule::new(
            id.clone(),
            name,
            expr,
            labels,
            is_auto_created,
        );
        rule.set_status(RuleStatus::Active);
        
        let mut rules = self.rules.write();
        rules.insert(id.clone(), rule);
        
        Ok(id)
    }

    pub fn get_rule(&self, id: &str) -> Option<PreAggregationRule> {
        self.rules.read().get(id).cloned()
    }

    pub fn get_all_rules(&self) -> Vec<PreAggregationRule> {
        self.rules.read().values().cloned().collect()
    }

    pub fn get_active_rules(&self) -> Vec<PreAggregationRule> {
        self.rules.read()
            .values()
            .filter(|r| r.status == RuleStatus::Active)
            .cloned()
            .collect()
    }

    pub fn get_auto_created_rules(&self) -> Vec<PreAggregationRule> {
        self.rules.read()
            .values()
            .filter(|r| r.is_auto_created)
            .cloned()
            .collect()
    }

    pub fn update_rule(&self, id: &str, updates: RuleUpdates) -> Result<()> {
        let mut rules = self.rules.write();
        
        if let Some(rule) = rules.get_mut(id) {
            if let Some(name) = updates.name {
                rule.name = name;
            }
            if let Some(expr) = updates.expr {
                rule.expr = expr;
            }
            if let Some(labels) = updates.labels {
                rule.labels = labels;
            }
            if let Some(status) = updates.status {
                rule.status = status;
            }
            if let Some(interval) = updates.evaluation_interval {
                rule.evaluation_interval = interval;
            }
        }
        
        Ok(())
    }

    pub fn delete_rule(&self, id: &str) -> Result<bool> {
        let mut rules = self.rules.write();
        Ok(rules.remove(id).is_some())
    }

    pub fn update_rule_frequency(&self, id: &str) {
        let mut rules = self.rules.write();
        if let Some(rule) = rules.get_mut(id) {
            rule.update_query_frequency();
        }
    }

    pub fn update_rule_evaluation(&self, id: &str) {
        let mut rules = self.rules.write();
        if let Some(rule) = rules.get_mut(id) {
            rule.update_evaluation();
        }
    }

    pub fn auto_create_rules(&self) -> Result<Vec<String>> {
        if !self.config.auto_create_enabled {
            return Ok(Vec::new());
        }

        let auto_rules_count = self.get_auto_created_rules().len();
        if auto_rules_count >= self.config.max_auto_rules {
            return Ok(Vec::new());
        }

        let high_freq_queries = self.frequency_tracker.get_high_frequency_queries();
        
        let mut created_ids = Vec::new();
        
        for (query, frequency) in high_freq_queries {
            if frequency >= self.config.frequency_threshold {
                let name = generate_rule_name(&query);
                let id = self.create_rule(
                    name,
                    query,
                    HashMap::new(),
                    true,
                )?;
                created_ids.push(id);
                
                if auto_rules_count + created_ids.len() >= self.config.max_auto_rules {
                    break;
                }
            }
        }
        
        Ok(created_ids)
    }

    pub fn auto_cleanup_rules(&self) -> Result<Vec<String>> {
        if !self.config.auto_cleanup_enabled {
            return Ok(Vec::new());
        }

        let auto_rules = self.get_auto_created_rules();
        let mut deleted_ids = Vec::new();
        
        for rule in auto_rules {
            let freq_per_hour = self.frequency_tracker.get_frequency_per_hour(&rule.expr);
            
            if freq_per_hour < self.config.low_frequency_threshold as f64 {
                let observation_millis = self.config.observation_period_hours * 60 * 60 * 1000;
                let now = Utc::now().timestamp_millis();
                
                if now - rule.last_query_time > observation_millis as i64 {
                    self.delete_rule(&rule.id)?;
                    deleted_ids.push(rule.id);
                }
            }
        }
        
        Ok(deleted_ids)
    }

    pub fn get_rules_count(&self) -> usize {
        self.rules.read().len()
    }

    pub fn get_auto_rules_count(&self) -> usize {
        self.get_auto_created_rules().len()
    }

    pub fn find_matching_rules(&self, query: &str) -> Vec<PreAggregationRule> {
        let normalized = normalize_query(query);
        
        self.rules.read()
            .values()
            .filter(|rule| {
                rule.status == RuleStatus::Active && 
                normalize_query(&rule.expr) == normalized
            })
            .cloned()
            .collect()
    }
}

#[derive(Debug, Clone)]
pub struct RuleUpdates {
    pub name: Option<String>,
    pub expr: Option<String>,
    pub labels: Option<HashMap<String, String>>,
    pub status: Option<RuleStatus>,
    pub evaluation_interval: Option<u64>,
}

impl RuleUpdates {
    pub fn new() -> Self {
        Self {
            name: None,
            expr: None,
            labels: None,
            status: None,
            evaluation_interval: None,
        }
    }

    pub fn name(mut self, name: String) -> Self {
        self.name = Some(name);
        self
    }

    pub fn expr(mut self, expr: String) -> Self {
        self.expr = Some(expr);
        self
    }

    pub fn labels(mut self, labels: HashMap<String, String>) -> Self {
        self.labels = Some(labels);
        self
    }

    pub fn status(mut self, status: RuleStatus) -> Self {
        self.status = Some(status);
        self
    }

    pub fn evaluation_interval(mut self, interval: u64) -> Self {
        self.evaluation_interval = Some(interval);
        self
    }
}

fn generate_rule_id() -> String {
    format!("preagg-{}", uuid::Uuid::new_v4())
}

fn generate_rule_name(query: &str) -> String {
    let sanitized: String = query
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '_')
        .take(50)
        .collect();
    
    format!("auto_{}", sanitized)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_rule() {
        let tracker = Arc::new(FrequencyTracker::new(FrequencyConfig::default()));
        let manager = PreAggregationManager::new(tracker);
        
        let id = manager.create_rule(
            "test_rule".to_string(),
            "sum(rate(http_requests_total[5m]))".to_string(),
            HashMap::new(),
            false,
        ).unwrap();
        
        assert!(manager.get_rule(&id).is_some());
    }

    #[test]
    fn test_update_rule() {
        let tracker = Arc::new(FrequencyTracker::new(FrequencyConfig::default()));
        let manager = PreAggregationManager::new(tracker);
        
        let id = manager.create_rule(
            "test_rule".to_string(),
            "sum(rate(http_requests_total[5m]))".to_string(),
            HashMap::new(),
            false,
        ).unwrap();
        
        let updates = RuleUpdates::new()
            .name("updated_rule".to_string())
            .status(RuleStatus::Inactive);
        
        manager.update_rule(&id, updates).unwrap();
        
        let rule = manager.get_rule(&id).unwrap();
        assert_eq!(rule.name, "updated_rule");
        assert_eq!(rule.status, RuleStatus::Inactive);
    }

    #[test]
    fn test_delete_rule() {
        let tracker = Arc::new(FrequencyTracker::new(FrequencyConfig::default()));
        let manager = PreAggregationManager::new(tracker);
        
        let id = manager.create_rule(
            "test_rule".to_string(),
            "sum(rate(http_requests_total[5m]))".to_string(),
            HashMap::new(),
            false,
        ).unwrap();
        
        assert!(manager.delete_rule(&id).unwrap());
        assert!(manager.get_rule(&id).is_none());
    }
}
