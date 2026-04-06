use serde::{Deserialize, Serialize};
use std::time::{Duration, SystemTime};

pub mod alerting;
pub mod recording;
pub mod evaluator;

pub use alerting::{AlertRule, AlertState, Alert, AlertManager};
pub use recording::{RecordingRule, RecordingManager};
pub use evaluator::RuleEvaluator;

/// 规则文件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleFile {
    /// 规则组
    pub groups: Vec<RuleGroup>,
}

/// 规则组
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleGroup {
    /// 组名称
    pub name: String,
    
    /// 评估间隔
    pub interval: Option<Duration>,
    
    /// 限制（并发评估数量）
    pub limit: Option<usize>,
    
    /// 规则
    pub rules: Vec<Rule>,
}

/// 规则类型
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Rule {
    /// 记录规则
    #[serde(rename = "record")]
    Recording(RecordingRule),
    
    /// 告警规则
    #[serde(rename = "alert")]
    Alerting(AlertRule),
}

/// 规则管理器
pub struct RuleManager {
    /// 规则组
    groups: Vec<RuleGroup>,
    
    /// 规则文件路径
    rule_files: Vec<std::path::PathBuf>,
    
    /// 最后加载时间
    last_load: Option<SystemTime>,
}

impl RuleManager {
    pub fn new() -> Self {
        Self {
            groups: Vec::new(),
            rule_files: Vec::new(),
            last_load: None,
        }
    }
    
    /// 从文件加载规则
    pub fn load_from_file(&mut self, path: &std::path::Path) -> crate::Result<()> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| crate::error::ServerError::Rule(format!("Failed to read rule file: {}", e)))?;
        
        let rule_file: RuleFile = serde_yaml::from_str(&content)
            .map_err(|e| crate::error::ServerError::Rule(format!("Failed to parse rule file: {}", e)))?;
        
        self.groups.extend(rule_file.groups);
        self.rule_files.push(path.to_path_buf());
        self.last_load = Some(SystemTime::now());
        
        Ok(())
    }
    
    /// 添加规则组
    pub fn add_group(&mut self, group: RuleGroup) {
        self.groups.push(group);
    }
    
    /// 获取所有规则组
    pub fn get_groups(&self) -> &[RuleGroup] {
        &self.groups
    }
    
    /// 获取所有规则
    pub fn get_all_rules(&self) -> Vec<&Rule> {
        self.groups.iter()
            .flat_map(|g| &g.rules)
            .collect()
    }
    
    /// 获取记录规则
    pub fn get_recording_rules(&self) -> Vec<&RecordingRule> {
        self.groups.iter()
            .flat_map(|g| &g.rules)
            .filter_map(|r| match r {
                Rule::Recording(r) => Some(r),
                _ => None,
            })
            .collect()
    }
    
    /// 获取告警规则
    pub fn get_alerting_rules(&self) -> Vec<&AlertRule> {
        self.groups.iter()
            .flat_map(|g| &g.rules)
            .filter_map(|r| match r {
                Rule::Alerting(r) => Some(r),
                _ => None,
            })
            .collect()
    }
    
    /// 获取规则数量
    pub fn get_rule_count(&self) -> usize {
        self.groups.iter()
            .map(|g| g.rules.len())
            .sum()
    }
    
    /// 获取记录规则数量
    pub fn get_recording_rule_count(&self) -> usize {
        self.get_recording_rules().len()
    }
    
    /// 获取告警规则数量
    pub fn get_alerting_rule_count(&self) -> usize {
        self.get_alerting_rules().len()
    }
}

impl Default for RuleManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alerting::AlertCondition;
    use std::collections::HashMap;

    #[test]
    fn test_rule_manager() {
        let mut manager = RuleManager::new();
        
        let group = RuleGroup {
            name: "test".to_string(),
            interval: None,
            limit: None,
            rules: vec![
                Rule::Recording(RecordingRule {
                    name: "job:http_requests:rate5m".to_string(),
                    expr: "sum(rate(http_requests_total[5m])) by (job)".to_string(),
                    labels: HashMap::new(),
                }),
                Rule::Alerting(AlertRule {
                    name: "HighErrorRate".to_string(),
                    expr: "rate(http_requests_total{status=~\"5..\"}[5m]) > 0.1".to_string(),
                    condition: AlertCondition::Gt(0.1),
                    duration: Duration::from_secs(300),
                    labels: HashMap::new(),
                    annotations: HashMap::new(),
                }),
            ],
        };
        
        manager.add_group(group);
        
        assert_eq!(manager.get_rule_count(), 2);
        assert_eq!(manager.get_recording_rule_count(), 1);
        assert_eq!(manager.get_alerting_rule_count(), 1);
    }
}
