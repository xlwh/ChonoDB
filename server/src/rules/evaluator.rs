use super::{AlertRule, RecordingRule};

/// 规则评估器
pub struct RuleEvaluator;

impl RuleEvaluator {
    pub fn new() -> Self {
        Self
    }

    /// 评估记录规则
    pub async fn evaluate_recording(&self, rule: &RecordingRule) -> crate::Result<()> {
        // TODO: 实现记录规则评估
        Ok(())
    }

    /// 评估告警规则
    pub async fn evaluate_alerting(&self, rule: &AlertRule) -> crate::Result<()> {
        // TODO: 实现告警规则评估
        Ok(())
    }
}

impl Default for RuleEvaluator {
    fn default() -> Self {
        Self::new()
    }
}
