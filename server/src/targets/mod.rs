use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::time::Interval;
use tokio::sync::RwLock;
use crate::state::ServerState;
use chronodb_storage::model::{Label, Sample};

pub mod scraper;

pub use scraper::Scraper;

/// 抓取目标
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Target {
    /// 目标 ID
    pub id: String,
    
    /// 目标名称
    pub name: String,
    
    /// 抓取 URL
    pub url: String,
    
    /// 标签
    pub labels: HashMap<String, String>,
    
    /// 健康状态
    pub health: TargetHealth,
    
    /// 最后抓取时间
    pub last_scrape: Option<SystemTime>,
    
    /// 最后错误
    pub last_error: Option<String>,
    
    /// 抓取间隔（秒）
    pub scrape_interval: u64,
    
    /// 抓取超时（秒）
    pub scrape_timeout: u64,
}

/// 目标健康状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum TargetHealth {
    /// 健康
    Up,
    
    /// 不健康
    Down,
    
    /// 未知
    Unknown,
}

/// 目标管理器
pub struct TargetManager {
    targets: HashMap<String, Target>,
    scraper: Scraper,
    state: Option<Arc<ServerState>>,
    intervals: HashMap<String, tokio::task::JoinHandle<()>>,
}

impl TargetManager {
    pub fn new() -> Self {
        Self {
            targets: HashMap::new(),
            scraper: Scraper::new(),
            state: None,
            intervals: HashMap::new(),
        }
    }
    
    /// 设置服务器状态
    pub fn set_state(&mut self, state: Arc<ServerState>) {
        self.state = Some(state);
    }
    
    /// 添加目标
    pub fn add_target(&mut self, target: Target) {
        self.targets.insert(target.id.clone(), target.clone());
        
        // 为目标创建抓取任务
        if let Some(ref state) = self.state {
            // 注意：这里我们不直接传递 self，而是在 start_scraping_task 中处理
            self.start_scraping_task(&target, state.clone());
        }
    }
    
    /// 从配置添加目标
    pub fn add_targets_from_config(&mut self, config_targets: &[TargetConfig]) {
        for config in config_targets {
            let target = Target {
                id: format!("{}-{}", config.name, config.url.replace("/", "-").replace(":", "-")),
                name: config.name.clone(),
                url: config.url.clone(),
                labels: config.labels.clone(),
                health: TargetHealth::Unknown,
                last_scrape: None,
                last_error: None,
                scrape_interval: config.scrape_interval.unwrap_or(60),
                scrape_timeout: config.scrape_timeout.unwrap_or(10),
            };
            
            self.add_target(target);
        }
    }
    
    /// 开始抓取任务
    fn start_scraping_task(&mut self, target: &Target, state: Arc<ServerState>) {
        // 简化实现，暂时不使用 tokio::spawn
        // 实际项目中应该使用更复杂的实现，确保线程安全
        let target_id = target.id.clone();
        self.intervals.insert(target_id, tokio::task::spawn(async move {
            // 这里可以实现抓取逻辑
            // 暂时为空实现
        }));
    }
    
    /// 删除目标
    pub fn remove_target(&mut self, id: &str) -> Option<Target> {
        // 取消抓取任务
        if let Some(handle) = self.intervals.remove(id) {
            handle.abort();
        }
        
        self.targets.remove(id)
    }
    
    /// 获取目标
    pub fn get_target(&self, id: &str) -> Option<&Target> {
        self.targets.get(id)
    }
    
    /// 获取所有目标
    pub fn get_all_targets(&self) -> Vec<&Target> {
        self.targets.values().collect()
    }
    
    /// 更新目标健康状态
    pub fn update_target_health(&mut self, id: &str, health: TargetHealth, error: Option<String>) {
        if let Some(target) = self.targets.get_mut(id) {
            target.health = health;
            target.last_scrape = Some(SystemTime::now());
            target.last_error = error;
        }
    }
    
    /// 获取活跃目标数量
    pub fn get_active_targets_count(&self) -> usize {
        self.targets.values()
            .filter(|t| t.health == TargetHealth::Up)
            .count()
    }
    
    /// 获取目标数量
    pub fn get_targets_count(&self) -> usize {
        self.targets.len()
    }
    
    /// 停止所有抓取任务
    pub fn stop_all_tasks(&mut self) {
        for (_, handle) in self.intervals.drain() {
            handle.abort();
        }
    }
}

impl Default for TargetManager {
    fn default() -> Self {
        Self::new()
    }
}

/// 目标配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TargetConfig {
    /// 目标名称
    pub name: String,
    
    /// 抓取 URL
    pub url: String,
    
    /// 标签
    pub labels: HashMap<String, String>,
    
    /// 抓取间隔（秒）
    pub scrape_interval: Option<u64>,
    
    /// 抓取超时（秒）
    pub scrape_timeout: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_target_manager() {
        let mut manager = TargetManager::new();
        
        let target = Target {
            id: "test-1".to_string(),
            name: "Test Target".to_string(),
            url: "http://localhost:8080/metrics".to_string(),
            labels: HashMap::new(),
            health: TargetHealth::Unknown,
            last_scrape: None,
            last_error: None,
            scrape_interval: 60,
            scrape_timeout: 10,
        };
        
        manager.add_target(target);
        assert_eq!(manager.get_targets_count(), 1);
        
        let target = manager.get_target("test-1").unwrap();
        assert_eq!(target.name, "Test Target");
        
        manager.update_target_health("test-1", TargetHealth::Up, None);
        let target = manager.get_target("test-1").unwrap();
        assert_eq!(target.health, TargetHealth::Up);
        assert!(target.last_scrape.is_some());
    }
}
