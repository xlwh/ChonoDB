use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use chronodb_storage::model::Labels;

/// 告警规则
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertRule {
    /// 规则名称
    pub name: String,
    
    /// PromQL 表达式
    pub expr: String,
    
    /// 告警条件
    #[serde(skip)]
    pub condition: AlertCondition,
    
    /// 持续时间
    #[serde(rename = "for")]
    #[serde(with = "humantime_serde")]
    pub duration: Duration,
    
    /// 标签
    pub labels: HashMap<String, String>,
    
    /// 注释
    pub annotations: HashMap<String, String>,
}

/// 告警条件
#[derive(Debug, Clone)]
pub enum AlertCondition {
    /// 大于
    Gt(f64),
    /// 大于等于
    Gte(f64),
    /// 小于
    Lt(f64),
    /// 小于等于
    Lte(f64),
    /// 等于
    Eq(f64),
    /// 不等于
    Ne(f64),
}

impl Default for AlertCondition {
    fn default() -> Self {
        AlertCondition::Ne(0.0)
    }
}

impl AlertCondition {
    /// 从字符串解析告警条件
    pub fn from_str(s: &str) -> Result<Self, String> {
        let parts: Vec<&str> = s.trim().split_whitespace().collect();
        if parts.len() != 2 {
            return Err("Invalid condition format. Expected format: '<operator> <value>'".to_string());
        }

        let operator = parts[0];
        let value_str = parts[1];

        let value = value_str.parse::<f64>().map_err(|e| format!("Invalid value: {}", e))?;

        match operator {
            "gt" => Ok(AlertCondition::Gt(value)),
            "gte" => Ok(AlertCondition::Gte(value)),
            "lt" => Ok(AlertCondition::Lt(value)),
            "lte" => Ok(AlertCondition::Lte(value)),
            "eq" => Ok(AlertCondition::Eq(value)),
            "ne" => Ok(AlertCondition::Ne(value)),
            _ => Err(format!("Invalid operator: {}", operator)),
        }
    }
}

/// 告警状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum AlertState {
    /// 正常
    Inactive,
    /// 待触发
    Pending,
    /// 已触发
    Firing,
}

/// 告警实例
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alert {
    /// 告警名称
    pub name: String,
    
    /// 状态
    pub state: AlertState,
    
    /// 标签
    pub labels: HashMap<String, String>,
    
    /// 注释
    pub annotations: HashMap<String, String>,
    
    /// 激活时间
    pub active_at: Option<SystemTime>,
    
    /// 最后评估时间
    pub last_evaluation: Option<SystemTime>,
    
    /// 值
    pub value: f64,
}

/// 告警管理器
pub struct AlertManager {
    alerts: Vec<Alert>,
    alert_notifiers: Vec<Box<dyn AlertNotifier>>,
}

/// 告警通知接口
pub trait AlertNotifier: Send + Sync {
    fn notify(&self, alert: &Alert, state_change: bool) -> Result<(), String>;
}

impl AlertManager {
    pub fn new() -> Self {
        Self {
            alerts: Vec::new(),
            alert_notifiers: Vec::new(),
        }
    }
    
    /// 添加告警
    pub fn add_alert(&mut self, alert: Alert) {
        self.alerts.push(alert);
    }
    
    /// 创建或更新告警
    pub fn create_alert(&mut self, name: String, labels: Labels, value: f64, _timestamp: i64, annotations: HashMap<String, String>) {
        let now = SystemTime::now();
        let labels_map: HashMap<String, String> = labels.into_iter().map(|label| (label.name, label.value)).collect();
        
        // 查找是否已存在相同标签的告警
        if let Some(existing_alert) = self.alerts.iter_mut().find(|a| {
            a.name == name && a.labels == labels_map
        }) {
            // 更新现有告警
            let _old_state = existing_alert.state.clone();
            existing_alert.value = value;
            existing_alert.last_evaluation = Some(now);
            existing_alert.annotations = annotations;
            
            // 检查是否需要状态转换
            if existing_alert.state == AlertState::Inactive {
                existing_alert.state = AlertState::Pending;
                existing_alert.active_at = Some(now);
                
                // 通知状态变化
                for notifier in &self.alert_notifiers {
                    let _ = notifier.notify(existing_alert, true);
                }
            } else if existing_alert.state == AlertState::Pending {
                // 检查是否达到持续时间
                if let Some(active_at) = existing_alert.active_at {
                    let duration_since_active = now.duration_since(active_at).unwrap();
                    if duration_since_active >= Duration::from_secs(300) { // 使用300秒作为默认持续时间
                        existing_alert.state = AlertState::Firing;
                        
                        // 通知状态变化
                        for notifier in &self.alert_notifiers {
                            let _ = notifier.notify(existing_alert, true);
                        }
                    }
                }
            }
        } else {
            // 创建新告警
            let new_alert = Alert {
                name,
                state: AlertState::Pending,
                labels: labels_map,
                annotations,
                active_at: Some(now),
                last_evaluation: Some(now),
                value,
            };
            
            self.alerts.push(new_alert);
            
            // 通知新告警
            let alert_ref = &self.alerts[self.alerts.len() - 1];
            for notifier in &self.alert_notifiers {
                let _ = notifier.notify(alert_ref, true);
            }
        }
    }
    
    /// 清理过期告警
    pub fn clean_expired_alerts(&mut self) {
        let now = SystemTime::now();
        let mut to_remove = Vec::new();
        
        for (i, alert) in self.alerts.iter().enumerate() {
            if let Some(last_eval) = alert.last_evaluation {
                if now.duration_since(last_eval).unwrap() > Duration::from_secs(3600) { // 1小时过期
                    to_remove.push(i);
                }
            }
        }
        
        // 反向删除，避免索引变化
        for i in to_remove.iter().rev() {
            let alert = &self.alerts[*i];
            
            // 通知告警解除
            for notifier in &self.alert_notifiers {
                let _ = notifier.notify(alert, true);
            }
            
            self.alerts.remove(*i);
        }
    }
    
    /// 获取所有告警
    pub fn get_alerts(&self) -> &[Alert] {
        &self.alerts
    }
    
    /// 获取活跃告警
    pub fn get_active_alerts(&self) -> Vec<&Alert> {
        self.alerts.iter()
            .filter(|a| a.state == AlertState::Firing)
            .collect()
    }
    
    /// 获取告警数量
    pub fn get_alert_count(&self) -> usize {
        self.alerts.len()
    }
    
    /// 获取活跃告警数量
    pub fn get_active_alert_count(&self) -> usize {
        self.alerts.iter()
            .filter(|a| a.state == AlertState::Firing)
            .count()
    }
    
    /// 添加告警通知器
    pub fn add_notifier(&mut self, notifier: Box<dyn AlertNotifier>) {
        self.alert_notifiers.push(notifier);
    }
}

impl Default for AlertManager {
    fn default() -> Self {
        Self::new()
    }
}

// 序列化支持
mod humantime_serde {
    use serde::{Deserialize, Deserializer, Serializer};
    use std::time::Duration;

    pub fn serialize<S>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&format!("{}s", duration.as_secs()))
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        humantime::parse_duration(&s).map_err(serde::de::Error::custom)
    }
}

// 示例通知器实现
struct ConsoleNotifier;

impl AlertNotifier for ConsoleNotifier {
    fn notify(&self, alert: &Alert, state_change: bool) -> Result<(), String> {
        if state_change {
            println!("[ALERT] {} - {}: {} (value: {})
Labels: {:?}
Annotations: {:?}",
                     alert.name,
                     match alert.state {
                         AlertState::Inactive => "RESOLVED",
                         AlertState::Pending => "PENDING",
                         AlertState::Firing => "FIRING",
                     },
                     alert.active_at.map(|t| {
                         let duration = t.duration_since(UNIX_EPOCH).unwrap();
                         format!("{}s ago", duration.as_secs())
                     }).unwrap_or("N/A".to_string()),
                     alert.value,
                     alert.labels,
                     alert.annotations);
        }
        Ok(())
    }
}

impl AlertManager {
    /// 添加控制台通知器
    pub fn add_console_notifier(&mut self) {
        self.add_notifier(Box::new(ConsoleNotifier));
    }

    /// 添加邮件通知器
    pub fn add_email_notifier(&mut self, config: EmailConfig) {
        self.add_notifier(Box::new(EmailNotifier::new(config)));
    }

    /// 添加 Webhook 通知器
    pub fn add_webhook_notifier(&mut self, config: WebhookConfig) {
        self.add_notifier(Box::new(WebhookNotifier::new(config)));
    }
}

/// 邮件通知配置
#[derive(Debug, Clone)]
pub struct EmailConfig {
    pub smtp_server: String,
    pub smtp_port: u16,
    pub username: String,
    pub password: String,
    pub from_address: String,
    pub to_addresses: Vec<String>,
}

/// 邮件通知器
pub struct EmailNotifier {
    config: EmailConfig,
}

impl EmailNotifier {
    pub fn new(config: EmailConfig) -> Self {
        Self { config }
    }
}

impl AlertNotifier for EmailNotifier {
    fn notify(&self, alert: &Alert, state_change: bool) -> Result<(), String> {
        if !state_change {
            return Ok(());
        }

        let subject = format!(
            "[{}] Alert: {}",
            match alert.state {
                AlertState::Inactive => "RESOLVED",
                AlertState::Pending => "PENDING",
                AlertState::Firing => "FIRING",
            },
            alert.name
        );

        let body = format!(
            "Alert: {}\nState: {:?}\nValue: {}\nLabels: {:?}\nAnnotations: {:?}",
            alert.name, alert.state, alert.value, alert.labels, alert.annotations
        );

        println!(
            "[EMAIL] Sending email to {:?}\nSubject: {}\nBody:\n{}",
            self.config.to_addresses, subject, body
        );

        Ok(())
    }
}

/// Webhook 通知配置
#[derive(Debug, Clone)]
pub struct WebhookConfig {
    pub url: String,
    pub headers: HashMap<String, String>,
    pub timeout_secs: u64,
}

/// Webhook 通知器
pub struct WebhookNotifier {
    config: WebhookConfig,
}

impl WebhookNotifier {
    pub fn new(config: WebhookConfig) -> Self {
        Self { config }
    }
}

impl AlertNotifier for WebhookNotifier {
    fn notify(&self, alert: &Alert, state_change: bool) -> Result<(), String> {
        if !state_change {
            return Ok(());
        }

        let payload = serde_json::json!({
            "name": alert.name,
            "state": format!("{:?}", alert.state),
            "value": alert.value,
            "labels": alert.labels,
            "annotations": alert.annotations,
            "active_at": alert.active_at.map(|t| {
                t.duration_since(UNIX_EPOCH).unwrap().as_secs()
            }),
        });

        println!(
            "[WEBHOOK] Sending to {}\nPayload: {}",
            self.config.url,
            serde_json::to_string_pretty(&payload).unwrap()
        );

        Ok(())
    }
}
