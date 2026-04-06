use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, SystemTime};

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
}

impl AlertManager {
    pub fn new() -> Self {
        Self {
            alerts: Vec::new(),
        }
    }
    
    /// 添加告警
    pub fn add_alert(&mut self, alert: Alert) {
        self.alerts.push(alert);
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
