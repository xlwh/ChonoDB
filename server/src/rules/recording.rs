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
