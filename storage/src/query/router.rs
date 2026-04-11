use std::sync::Arc;
use std::collections::HashMap;
use parking_lot::RwLock;
use crate::model::{PreAggregationRule, PreAggregationIndex, RouteDecision, LabelMatcher};
use crate::query::normalize_query;
use crate::error::Result;

pub struct QueryRouter {
    rules: Arc<RwLock<HashMap<String, PreAggregationRule>>>,
    indexes: Arc<RwLock<HashMap<String, PreAggregationIndex>>>,
    config: RouterConfig,
}

#[derive(Debug, Clone)]
pub struct RouterConfig {
    pub enable_routing: bool,
    pub max_match_candidates: usize,
    pub cache_size: usize,
}

impl Default for RouterConfig {
    fn default() -> Self {
        Self {
            enable_routing: true,
            max_match_candidates: 10,
            cache_size: 1000,
        }
    }
}

impl QueryRouter {
    pub fn new(
        rules: Arc<RwLock<HashMap<String, PreAggregationRule>>>,
        indexes: Arc<RwLock<HashMap<String, PreAggregationIndex>>>,
    ) -> Self {
        Self {
            rules,
            indexes,
            config: RouterConfig::default(),
        }
    }

    pub fn with_config(
        rules: Arc<RwLock<HashMap<String, PreAggregationRule>>>,
        indexes: Arc<RwLock<HashMap<String, PreAggregationIndex>>>,
        config: RouterConfig,
    ) -> Self {
        Self {
            rules,
            indexes,
            config,
        }
    }

    pub fn route(&self, query: &str, start: i64, end: i64) -> RouteDecision {
        if !self.config.enable_routing {
            return RouteDecision::original();
        }

        let normalized = normalize_query(query);
        
        let matching_rules = self.find_matching_rules(&normalized);
        
        if matching_rules.is_empty() {
            return RouteDecision::original();
        }

        let best_match = self.select_best_match(&matching_rules, start, end);
        
        if let Some((rule, score)) = best_match {
            RouteDecision::preaggregated(rule.id.clone(), rule.name.clone(), score)
        } else {
            RouteDecision::original()
        }
    }

    fn find_matching_rules(&self, normalized_query: &str) -> Vec<PreAggregationRule> {
        let rules = self.rules.read();
        
        rules.values()
            .filter(|rule| {
                let rule_normalized = normalize_query(&rule.expr);
                rule_normalized == normalized_query
            })
            .cloned()
            .collect()
    }

    fn select_best_match(
        &self,
        rules: &[PreAggregationRule],
        _start: i64,
        _end: i64,
    ) -> Option<(PreAggregationRule, f64)> {
        if rules.is_empty() {
            return None;
        }

        let mut best_rule: Option<&PreAggregationRule> = None;
        let mut best_score = 0.0;

        for rule in rules {
            let score = self.calculate_match_score(rule);
            
            if score > best_score {
                best_score = score;
                best_rule = Some(rule);
            }
        }

        best_rule.map(|r| (r.clone(), best_score))
    }

    fn calculate_match_score(&self, rule: &PreAggregationRule) -> f64 {
        let mut score = 0.0;

        if rule.is_auto_created {
            score += 0.1;
        }

        score += (rule.query_frequency as f64).log10() / 10.0;

        let now = chrono::Utc::now().timestamp_millis();
        let time_since_last_query = now - rule.last_query_time;
        let hours_since_query = time_since_last_query as f64 / (1000.0 * 60.0 * 60.0);
        
        if hours_since_query < 1.0 {
            score += 0.3;
        } else if hours_since_query < 24.0 {
            score += 0.2;
        } else if hours_since_query < 168.0 {
            score += 0.1;
        }

        score.min(1.0)
    }

    pub fn add_index(&self, index: PreAggregationIndex) {
        let mut indexes = self.indexes.write();
        indexes.insert(index.rule_id.clone(), index);
    }

    pub fn remove_index(&self, rule_id: &str) {
        let mut indexes = self.indexes.write();
        indexes.remove(rule_id);
    }

    pub fn get_index(&self, rule_id: &str) -> Option<PreAggregationIndex> {
        self.indexes.read().get(rule_id).cloned()
    }

    pub fn get_all_indexes(&self) -> Vec<PreAggregationIndex> {
        self.indexes.read().values().cloned().collect()
    }

    pub fn update_index(&self, rule_id: &str, data_location: crate::model::DataLocation) {
        let mut indexes = self.indexes.write();
        if let Some(index) = indexes.get_mut(rule_id) {
            index.update(data_location);
        }
    }

    pub fn get_routing_stats(&self) -> RoutingStats {
        let rules = self.rules.read();
        let indexes = self.indexes.read();
        
        RoutingStats {
            total_rules: rules.len(),
            active_rules: rules.values().filter(|r| r.status == crate::model::RuleStatus::Active).count(),
            total_indexes: indexes.len(),
            routing_enabled: self.config.enable_routing,
        }
    }
}

#[derive(Debug, Clone)]
pub struct RoutingStats {
    pub total_rules: usize,
    pub active_rules: usize,
    pub total_indexes: usize,
    pub routing_enabled: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::RuleStatus;
    use std::collections::HashMap;

    #[test]
    fn test_route_no_match() {
        let rules = Arc::new(RwLock::new(HashMap::new()));
        let indexes = Arc::new(RwLock::new(HashMap::new()));
        let router = QueryRouter::new(rules, indexes);
        
        let decision = router.route("up", 0, 1000);
        
        assert!(!decision.use_preaggregated);
    }

    #[test]
    fn test_route_with_match() {
        let rules = Arc::new(RwLock::new(HashMap::new()));
        let indexes = Arc::new(RwLock::new(HashMap::new()));
        
        let mut rule = PreAggregationRule::new(
            "rule-1".to_string(),
            "test_rule".to_string(),
            "up".to_string(),
            HashMap::new(),
            false,
        );
        rule.status = RuleStatus::Active;
        rule.query_frequency = 100;
        
        rules.write().insert("rule-1".to_string(), rule);
        
        let router = QueryRouter::new(rules, indexes);
        let decision = router.route("up", 0, 1000);
        
        assert!(decision.use_preaggregated);
        assert_eq!(decision.rule_id, Some("rule-1".to_string()));
    }
}
