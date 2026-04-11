use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

/// 告警级别
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AlertSeverity {
    Critical,
    Warning,
    Info,
}

/// 告警规则模板
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertRuleTemplate {
    pub name: String,
    pub description: String,
    pub severity: AlertSeverity,
    pub expr: String,
    pub condition: String,
    pub duration: Duration,
    pub labels: HashMap<String, String>,
    pub annotations: HashMap<String, String>,
    pub recommended_actions: Vec<String>,
}

/// 告警规则模板管理器
#[derive(Debug, Clone)]
pub struct AlertRuleTemplateManager {
    templates: HashMap<String, AlertRuleTemplate>,
}

impl AlertRuleTemplateManager {
    pub fn new() -> Self {
        Self {
            templates: HashMap::new(),
        }
    }

    pub fn add_template(&mut self, template: AlertRuleTemplate) {
        self.templates.insert(template.name.clone(), template);
    }

    pub fn get_template(&self, name: &str) -> Option<&AlertRuleTemplate> {
        self.templates.get(name)
    }

    pub fn get_all_templates(&self) -> Vec<&AlertRuleTemplate> {
        self.templates.values().collect()
    }

    pub fn get_templates_by_severity(&self, severity: AlertSeverity) -> Vec<&AlertRuleTemplate> {
        self.templates
            .values()
            .filter(|t| t.severity == severity)
            .collect()
    }

    pub fn remove_template(&mut self, name: &str) -> Option<AlertRuleTemplate> {
        self.templates.remove(name)
    }

    pub fn template_count(&self) -> usize {
        self.templates.len()
    }

    /// 从模板创建告警规则
    pub fn create_alert_rule(&self, template_name: &str, custom_labels: Option<HashMap<String, String>>) -> Option<crate::rules::AlertRule> {
        if let Some(template) = self.get_template(template_name) {
            let mut labels = template.labels.clone();
            if let Some(custom) = custom_labels {
                labels.extend(custom);
            }

            Some(crate::rules::AlertRule {
                name: template.name.clone(),
                expr: template.expr.clone(),
                condition: crate::rules::AlertCondition::from_str(&template.condition).unwrap_or(crate::rules::AlertCondition::Gt(0.0)),
                duration: template.duration,
                labels,
                annotations: template.annotations.clone(),
            })
        } else {
            None
        }
    }
}

impl Default for AlertRuleTemplateManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_alert_rule_template_manager() {
        let mut manager = AlertRuleTemplateManager::new();

        let template = AlertRuleTemplate {
            name: "HighErrorRate".to_string(),
            description: "High HTTP error rate".to_string(),
            severity: AlertSeverity::Critical,
            expr: "rate(http_requests_total{status=~\"5..\"}[5m]) > 0.1".to_string(),
            condition: "gt 0.1".to_string(),
            duration: Duration::from_secs(300),
            labels: HashMap::new(),
            annotations: HashMap::new(),
            recommended_actions: vec!["Check server logs", "Investigate network issues"].to_vec(),
        };

        manager.add_template(template);

        assert_eq!(manager.template_count(), 1);
        assert!(manager.get_template("HighErrorRate").is_some());
        assert_eq!(manager.get_templates_by_severity(AlertSeverity::Critical).len(), 1);

        manager.remove_template("HighErrorRate");
        assert_eq!(manager.template_count(), 0);
        assert!(manager.get_template("HighErrorRate").is_none());
    }
}