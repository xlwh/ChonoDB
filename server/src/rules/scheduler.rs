use std::sync::Arc;
use std::collections::HashMap;
use parking_lot::RwLock;
use tokio::time::{interval, Duration};
use chrono::Utc;
use tracing::{info, warn, error, debug};
use chronodb_storage::model::{PreAggregationRule, RuleStatus, DataLocation};
use chronodb_storage::query::QueryEngine;
use chronodb_storage::distributed::{DistributedPreAggregationCoordinator, TaskStatus as DistributedTaskStatus};
use crate::error::Result;

pub struct PreAggregationScheduler {
    rules: Arc<RwLock<HashMap<String, PreAggregationRule>>>,
    query_engine: Arc<QueryEngine>,
    data_store: Arc<RwLock<HashMap<String, Vec<PreAggregatedData>>>>,
    config: SchedulerConfig,
    distributed_coordinator: Option<Arc<DistributedPreAggregationCoordinator>>,
    node_id: String,
}

#[derive(Debug, Clone)]
pub struct SchedulerConfig {
    pub max_concurrent_tasks: usize,
    pub retry_attempts: u32,
    pub retry_delay_seconds: u64,
    pub task_timeout_seconds: u64,
}

impl Default for SchedulerConfig {
    fn default() -> Self {
        Self {
            max_concurrent_tasks: 10,
            retry_attempts: 3,
            retry_delay_seconds: 60,
            task_timeout_seconds: 300,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PreAggregatedData {
    pub rule_id: String,
    pub timestamp: i64,
    pub value: f64,
    pub labels: Vec<(String, String)>,
}

#[derive(Debug, Clone)]
pub struct TaskStatus {
    pub rule_id: String,
    pub status: TaskState,
    pub last_execution: i64,
    pub next_execution: i64,
    pub error_count: u32,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskState {
    Pending,
    Running,
    Success,
    Failed,
    Disabled,
}

impl PreAggregationScheduler {
    pub fn new(query_engine: Arc<QueryEngine>) -> Self {
        Self {
            rules: Arc::new(RwLock::new(HashMap::new())),
            query_engine,
            data_store: Arc::new(RwLock::new(HashMap::new())),
            config: SchedulerConfig::default(),
            distributed_coordinator: None,
            node_id: uuid::Uuid::new_v4().to_string(),
        }
    }

    pub fn with_config(
        query_engine: Arc<QueryEngine>,
        config: SchedulerConfig,
    ) -> Self {
        Self {
            rules: Arc::new(RwLock::new(HashMap::new())),
            query_engine,
            data_store: Arc::new(RwLock::new(HashMap::new())),
            config,
            distributed_coordinator: None,
            node_id: uuid::Uuid::new_v4().to_string(),
        }
    }

    pub fn with_distributed(
        query_engine: Arc<QueryEngine>,
        config: SchedulerConfig,
        coordinator: Arc<DistributedPreAggregationCoordinator>,
        node_id: String,
    ) -> Self {
        Self {
            rules: Arc::new(RwLock::new(HashMap::new())),
            query_engine,
            data_store: Arc::new(RwLock::new(HashMap::new())),
            config,
            distributed_coordinator: Some(coordinator),
            node_id,
        }
    }

    pub fn add_rule(&self, rule: PreAggregationRule) {
        let mut rules = self.rules.write();
        rules.insert(rule.id.clone(), rule);
    }

    pub fn remove_rule(&self, rule_id: &str) {
        let mut rules = self.rules.write();
        rules.remove(rule_id);
        
        let mut data = self.data_store.write();
        data.remove(rule_id);
    }

    pub async fn execute_task(&self, rule: &PreAggregationRule) -> Result<()> {
        if let Some(coordinator) = &self.distributed_coordinator {
            if let Some(assignment) = coordinator.get_task_assignment(&rule.id).await {
                if assignment.assigned_node != self.node_id {
                    debug!("Skipping task {} - assigned to node {}", rule.id, assignment.assigned_node);
                    return Ok(());
                }
            }
        }

        info!("Executing pre-aggregation task for rule: {}", rule.name);

        if let Some(coordinator) = &self.distributed_coordinator {
            coordinator.update_task_status(&rule.id, DistributedTaskStatus::Running).await?;
        }
        
        let now = Utc::now().timestamp_millis();
        let start = now - (rule.evaluation_interval as i64 * 1000 * 2);
        let end = now;
        let step = rule.evaluation_interval as i64 * 1000;
        
        let result = self.query_engine
            .query(&rule.expr, start, end, step)
            .await?;
        
        let mut data = self.data_store.write();
        let aggregated_data = result.series.iter()
            .flat_map(|series| {
                series.samples.iter().map(|sample| {
                    PreAggregatedData {
                        rule_id: rule.id.clone(),
                        timestamp: sample.timestamp,
                        value: sample.value,
                        labels: series.labels.iter()
                            .map(|l| (l.name.clone(), l.value.clone()))
                            .collect(),
                    }
                })
            })
            .collect();
        
        data.insert(rule.id.clone(), aggregated_data);
        
        if let Some(coordinator) = &self.distributed_coordinator {
            coordinator.update_task_status(&rule.id, DistributedTaskStatus::Completed).await?;
        }
        
        info!("Pre-aggregation task completed for rule: {}", rule.name);
        
        Ok(())
    }

    pub async fn start(&self) {
        let mut ticker = interval(Duration::from_secs(60));
        let mut heartbeat_ticker = interval(Duration::from_secs(10));
        
        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    let rules = self.get_active_rules();
                    
                    for rule in rules {
                        if let Err(e) = self.execute_task(&rule).await {
                            error!("Failed to execute task for rule {}: {:?}", rule.name, e);
                            
                            if let Some(coordinator) = &self.distributed_coordinator {
                                let _ = coordinator.update_task_status(&rule.id, DistributedTaskStatus::Failed).await;
                            }
                        }
                    }
                }
                
                _ = heartbeat_ticker.tick() => {
                    if let Some(coordinator) = &self.distributed_coordinator {
                        let rules = self.get_active_rules();
                        for rule in rules {
                            if let Err(e) = coordinator.heartbeat(&rule.id).await {
                                warn!("Failed to send heartbeat for task {}: {:?}", rule.id, e);
                            }
                        }
                    }
                }
            }
        }
    }

    fn get_active_rules(&self) -> Vec<PreAggregationRule> {
        let rules = self.rules.read();
        rules.values()
            .filter(|r| r.status == RuleStatus::Active)
            .cloned()
            .collect()
    }

    pub fn get_aggregated_data(&self, rule_id: &str) -> Option<Vec<PreAggregatedData>> {
        self.data_store.read().get(rule_id).cloned()
    }

    pub fn get_all_data(&self) -> HashMap<String, Vec<PreAggregatedData>> {
        self.data_store.read().clone()
    }

    pub fn get_task_statuses(&self) -> Vec<TaskStatus> {
        let rules = self.rules.read();
        rules.values()
            .map(|rule| {
                TaskStatus {
                    rule_id: rule.id.clone(),
                    status: match rule.status {
                        RuleStatus::Active => TaskState::Pending,
                        RuleStatus::Inactive => TaskState::Disabled,
                        RuleStatus::Pending => TaskState::Pending,
                        RuleStatus::Failed => TaskState::Failed,
                    },
                    last_execution: rule.last_evaluation,
                    next_execution: rule.last_evaluation + (rule.evaluation_interval as i64 * 1000),
                    error_count: 0,
                    last_error: None,
                }
            })
            .collect()
    }

    pub fn get_stats(&self) -> SchedulerStats {
        let rules = self.rules.read();
        let data = self.data_store.read();
        
        SchedulerStats {
            total_rules: rules.len(),
            active_rules: rules.values().filter(|r| r.status == RuleStatus::Active).count(),
            total_data_points: data.values().map(|v| v.len()).sum(),
            data_size_bytes: data.values()
                .map(|v| v.len() * std::mem::size_of::<PreAggregatedData>())
                .sum(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SchedulerStats {
    pub total_rules: usize,
    pub active_rules: usize,
    pub total_data_points: usize,
    pub data_size_bytes: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chronodb_storage::memstore::MemStore;
    use chronodb_storage::config::StorageConfig;
    use tempfile::tempdir;

    fn create_test_engine() -> Arc<QueryEngine> {
        let temp_dir = tempdir().unwrap();
        let config = StorageConfig {
            data_dir: temp_dir.path().to_string_lossy().to_string(),
            ..Default::default()
        };
        let memstore = Arc::new(MemStore::new(config).unwrap());
        Arc::new(QueryEngine::new(memstore))
    }

    #[test]
    fn test_scheduler_creation() {
        let engine = create_test_engine();
        let scheduler = PreAggregationScheduler::new(engine);
        
        assert_eq!(scheduler.get_stats().total_rules, 0);
    }

    #[test]
    fn test_add_remove_rule() {
        let engine = create_test_engine();
        let scheduler = PreAggregationScheduler::new(engine);
        
        let rule = PreAggregationRule::new(
            "rule-1".to_string(),
            "test_rule".to_string(),
            "up".to_string(),
            HashMap::new(),
            false,
        );
        
        scheduler.add_rule(rule);
        assert_eq!(scheduler.get_stats().total_rules, 1);
        
        scheduler.remove_rule("rule-1");
        assert_eq!(scheduler.get_stats().total_rules, 0);
    }
}
