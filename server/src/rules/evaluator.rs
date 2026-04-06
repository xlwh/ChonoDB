use super::{AlertRule, RecordingRule, RuleManager, AlertManager};
use crate::state::ServerState;
use crate::api::handlers::parse_label_matchers;
use chronodb_storage::model::{Label, Sample, TimeSeries};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// 规则评估器
pub struct RuleEvaluator {
    state: Arc<ServerState>,
    rule_manager: Arc<tokio::sync::RwLock<RuleManager>>,
    alert_manager: Arc<tokio::sync::RwLock<AlertManager>>,
}

impl RuleEvaluator {
    pub fn new(state: Arc<ServerState>, rule_manager: Arc<tokio::sync::RwLock<RuleManager>>, alert_manager: Arc<tokio::sync::RwLock<AlertManager>>) -> Self {
        Self {
            state,
            rule_manager,
            alert_manager,
        }
    }

    /// 评估记录规则
    pub async fn evaluate_recording(&self, rule: &RecordingRule) -> crate::Result<()> {
        // 解析表达式
        let label_matchers = parse_label_matchers(&rule.expr);
        
        // 获取当前时间
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;
        
        // 执行查询
        let result = self.state.memstore.query(&label_matchers, now - 300000, now)?;
        
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
                self.state.memstore.write_single(labels.clone(), sample)?;
            }
        }
        
        Ok(())
    }

    /// 评估告警规则
    pub async fn evaluate_alerting(&self, rule: &AlertRule) -> crate::Result<()> {
        // 解析表达式
        let label_matchers = parse_label_matchers(&rule.expr);
        
        // 获取当前时间
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;
        
        // 执行查询
        let result = self.state.memstore.query(&label_matchers, now - 300000, now)?;
        
        // 检查告警条件
        for ts in result {
            for sample in ts.samples {
                // 根据告警条件检查值
                let is_firing = match rule.condition {
                    super::alerting::AlertCondition::Gt(threshold) => sample.value > threshold,
                    super::alerting::AlertCondition::Lt(threshold) => sample.value < threshold,
                    super::alerting::AlertCondition::Gte(threshold) => sample.value >= threshold,
                    super::alerting::AlertCondition::Lte(threshold) => sample.value <= threshold,
                };
                
                if is_firing {
                    // 创建告警
                    let mut alert_manager = self.alert_manager.write().await;
                    alert_manager.create_alert(
                        rule.name.clone(),
                        ts.labels,
                        sample.value,
                        now,
                        rule.annotations.clone(),
                    );
                }
            }
        }
        
        Ok(())
    }

    /// 评估所有规则
    pub async fn evaluate_all(&self) -> crate::Result<()> {
        let rule_manager = self.rule_manager.read().await;
        
        // 评估记录规则
        for rule in rule_manager.get_recording_rules() {
            if let Err(e) = self.evaluate_recording(rule).await {
                eprintln!("Error evaluating recording rule {}: {:?}", rule.name, e);
            }
        }
        
        // 评估告警规则
        for rule in rule_manager.get_alerting_rules() {
            if let Err(e) = self.evaluate_alerting(rule).await {
                eprintln!("Error evaluating alerting rule {}: {:?}", rule.name, e);
            }
        }
        
        Ok(())
    }
}

impl Default for RuleEvaluator {
    fn default() -> Self {
        panic!("RuleEvaluator requires ServerState, RuleManager, and AlertManager");
    }
}
