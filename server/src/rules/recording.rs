use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use crate::state::ServerState;
use crate::api::handlers::parse_label_matchers;
use chronodb_storage::model::Label;
use std::time::{SystemTime, UNIX_EPOCH};

/// 记录规则
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingRule {
    /// 规则名称（新指标名）
    pub name: String,
    
    /// PromQL 表达式
    pub expr: String,
    
    /// 标签
    pub labels: HashMap<String, String>,
}

/// 记录管理器
pub struct RecordingManager {
    rules: Vec<RecordingRule>,
    state: Option<Arc<ServerState>>,
}

impl RecordingManager {
    pub fn new() -> Self {
        Self {
            rules: Vec::new(),
            state: None,
        }
    }
    
    /// 设置服务器状态
    pub fn set_state(&mut self, state: Arc<ServerState>) {
        self.state = Some(state);
    }
    
    /// 添加记录规则
    pub fn add_rule(&mut self, rule: RecordingRule) {
        self.rules.push(rule);
    }
    
    /// 获取所有记录规则
    pub fn get_rules(&self) -> &[RecordingRule] {
        &self.rules
    }
    
    /// 获取规则数量
    pub fn get_rule_count(&self) -> usize {
        self.rules.len()
    }
    
    /// 执行所有记录规则
    pub async fn evaluate_all(&self) -> crate::Result<()> {
        if let Some(ref state) = self.state {
            for rule in &self.rules {
                if let Err(e) = self.evaluate_rule(rule, state).await {
                    eprintln!("Error evaluating recording rule {}: {:?}", rule.name, e);
                }
            }
        }
        Ok(())
    }
    
    /// 执行单个记录规则
    async fn evaluate_rule(&self, rule: &RecordingRule, state: &Arc<ServerState>) -> crate::Result<()> {
        // 解析表达式
        let label_matchers = parse_label_matchers(&rule.expr);
        
        // 获取当前时间
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;
        
        // 执行查询
        let result = state.memstore.query(&label_matchers, now - 300000, now)?;
        
        // 为每个结果创建新的时间序列
        for ts in result {
            // 构建新的标签
            let mut labels = vec![
                Label::new("__name__", rule.name.clone()),
            ];
            
            // 添加规则中定义的标签
            for (k, v) in &rule.labels {
                labels.push(Label::new(k, v));
            }
            
            // 写入新的时间序列
            for sample in ts.samples {
                state.memstore.write_single(labels.clone(), sample)?;
            }
        }
        
        Ok(())
    }
}

impl Default for RecordingManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_recording_rule_creation() {
        let rule = RecordingRule {
            name: "job:http_requests:rate5m".to_string(),
            expr: "rate(http_requests_total[5m])".to_string(),
            labels: HashMap::new(),
        };
        assert_eq!(rule.name, "job:http_requests:rate5m");
        assert_eq!(rule.expr, "rate(http_requests_total[5m])");
    }

    #[test]
    fn test_recording_rule_with_labels() {
        let mut labels = HashMap::new();
        labels.insert("team".to_string(), "monitoring".to_string());

        let rule = RecordingRule {
            name: "custom:metric".to_string(),
            expr: "up".to_string(),
            labels,
        };
        assert_eq!(rule.labels.len(), 1);
        assert_eq!(rule.labels.get("team").unwrap(), "monitoring");
    }

    #[test]
    fn test_recording_manager_new() {
        let manager = RecordingManager::new();
        assert_eq!(manager.get_rule_count(), 0);
        assert!(manager.get_rules().is_empty());
    }

    #[test]
    fn test_recording_manager_default() {
        let manager = RecordingManager::default();
        assert_eq!(manager.get_rule_count(), 0);
    }

    #[test]
    fn test_add_rule() {
        let mut manager = RecordingManager::new();

        let rule = RecordingRule {
            name: "test:metric".to_string(),
            expr: "up".to_string(),
            labels: HashMap::new(),
        };

        manager.add_rule(rule);
        assert_eq!(manager.get_rule_count(), 1);
    }

    #[test]
    fn test_add_multiple_rules() {
        let mut manager = RecordingManager::new();

        for i in 0..5 {
            let rule = RecordingRule {
                name: format!("rule:{}", i),
                expr: "up".to_string(),
                labels: HashMap::new(),
            };
            manager.add_rule(rule);
        }

        assert_eq!(manager.get_rule_count(), 5);
    }

    #[test]
    fn test_get_rules() {
        let mut manager = RecordingManager::new();

        let rule1 = RecordingRule {
            name: "rule1".to_string(),
            expr: "up".to_string(),
            labels: HashMap::new(),
        };
        let rule2 = RecordingRule {
            name: "rule2".to_string(),
            expr: "down".to_string(),
            labels: HashMap::new(),
        };

        manager.add_rule(rule1);
        manager.add_rule(rule2);

        let rules = manager.get_rules();
        assert_eq!(rules.len(), 2);
        assert_eq!(rules[0].name, "rule1");
        assert_eq!(rules[1].name, "rule2");
    }

    #[test]
    fn test_recording_rule_serialization() {
        let rule = RecordingRule {
            name: "test:metric".to_string(),
            expr: "rate(http_requests[5m])".to_string(),
            labels: HashMap::new(),
        };

        let json = serde_json::to_string(&rule).unwrap();
        let deserialized: RecordingRule = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.name, "test:metric");
        assert_eq!(deserialized.expr, "rate(http_requests[5m])");
    }
}
