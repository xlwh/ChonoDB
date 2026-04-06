use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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
}

impl RecordingManager {
    pub fn new() -> Self {
        Self {
            rules: Vec::new(),
        }
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
}

impl Default for RecordingManager {
    fn default() -> Self {
        Self::new()
    }
}
