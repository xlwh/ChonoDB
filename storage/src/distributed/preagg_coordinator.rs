use crate::error::Result;
use crate::model::PreAggregationRule;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::time::interval;
use tracing::{debug, info, warn};
use chrono::Utc;

#[derive(Debug, Clone)]
pub struct DistributedPreAggregationConfig {
    pub coordination_interval_ms: u64,
    pub task_timeout_ms: u64,
    pub max_retries: u32,
    pub enable_auto_failover: bool,
}

impl Default for DistributedPreAggregationConfig {
    fn default() -> Self {
        Self {
            coordination_interval_ms: 10000,
            task_timeout_ms: 300000,
            max_retries: 3,
            enable_auto_failover: true,
        }
    }
}

pub struct DistributedPreAggregationCoordinator {
    config: DistributedPreAggregationConfig,
    node_id: String,
    task_assignments: Arc<RwLock<HashMap<String, TaskAssignment>>>,
    node_tasks: Arc<RwLock<HashMap<String, HashSet<String>>>>,
    pending_tasks: Arc<RwLock<Vec<String>>>,
    coordination_task: Arc<RwLock<Option<tokio::task::JoinHandle<()>>>>,
}

#[derive(Debug, Clone)]
pub struct TaskAssignment {
    pub rule_id: String,
    pub assigned_node: String,
    pub assigned_at: i64,
    pub status: TaskStatus,
    pub last_heartbeat: i64,
    pub retry_count: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Timeout,
}

impl DistributedPreAggregationCoordinator {
    pub fn new(node_id: String, config: DistributedPreAggregationConfig) -> Self {
        Self {
            config,
            node_id,
            task_assignments: Arc::new(RwLock::new(HashMap::new())),
            node_tasks: Arc::new(RwLock::new(HashMap::new())),
            pending_tasks: Arc::new(RwLock::new(Vec::new())),
            coordination_task: Arc::new(RwLock::new(None)),
        }
    }

    pub async fn start(&self) -> Result<()> {
        info!("Starting distributed pre-aggregation coordinator on node {}", self.node_id);

        let task_assignments = self.task_assignments.clone();
        let node_tasks = self.node_tasks.clone();
        let pending_tasks = self.pending_tasks.clone();
        let config = self.config.clone();
        let node_id = self.node_id.clone();

        let coordination_handle = tokio::spawn(async move {
            let mut interval = interval(Duration::from_millis(config.coordination_interval_ms));

            loop {
                interval.tick().await;

                let now = Utc::now().timestamp_millis();

                let mut assignments = task_assignments.write().await;
                let mut pending = pending_tasks.write().await;

                for (rule_id, assignment) in assignments.iter_mut() {
                    if assignment.status == TaskStatus::Running {
                        if now - assignment.last_heartbeat > config.task_timeout_ms as i64 {
                            warn!("Task {} timed out on node {}", rule_id, assignment.assigned_node);
                            assignment.status = TaskStatus::Timeout;

                            if config.enable_auto_failover {
                                pending.push(rule_id.clone());
                            }
                        }
                    }
                }

                for rule_id in pending.drain(..) {
                    if let Some(assignment) = assignments.get_mut(&rule_id) {
                        if assignment.retry_count < config.max_retries {
                            assignment.retry_count += 1;
                            assignment.status = TaskStatus::Pending;
                            debug!("Reassigning task {} (retry {})", rule_id, assignment.retry_count);
                        } else {
                            assignment.status = TaskStatus::Failed;
                            warn!("Task {} failed after {} retries", rule_id, config.max_retries);
                        }
                    }
                }
            }
        });

        let mut task = self.coordination_task.write().await;
        *task = Some(coordination_handle);

        Ok(())
    }

    pub async fn assign_task(&self, rule: &PreAggregationRule, available_nodes: &[String]) -> Result<String> {
        let mut assignments = self.task_assignments.write().await;
        let mut node_tasks = self.node_tasks.write().await;

        if let Some(assignment) = assignments.get(&rule.id) {
            if assignment.status == TaskStatus::Running || assignment.status == TaskStatus::Pending {
                return Ok(assignment.assigned_node.clone());
            }
        }

        let selected_node = self.select_node_for_task(&node_tasks, available_nodes)?;

        let assignment = TaskAssignment {
            rule_id: rule.id.clone(),
            assigned_node: selected_node.clone(),
            assigned_at: Utc::now().timestamp_millis(),
            status: TaskStatus::Pending,
            last_heartbeat: Utc::now().timestamp_millis(),
            retry_count: 0,
        };

        assignments.insert(rule.id.clone(), assignment);
        node_tasks.entry(selected_node.clone()).or_insert_with(HashSet::new).insert(rule.id.clone());

        info!("Assigned task {} to node {}", rule.id, selected_node);

        Ok(selected_node)
    }

    fn select_node_for_task(
        &self,
        node_tasks: &HashMap<String, HashSet<String>>,
        available_nodes: &[String],
    ) -> Result<String> {
        if available_nodes.is_empty() {
            return Err(crate::error::Error::Internal(
                "No available nodes for task assignment".to_string(),
            ));
        }

        let mut min_tasks = usize::MAX;
        let mut selected_node = available_nodes[0].clone();

        for node_id in available_nodes {
            let task_count = node_tasks.get(node_id).map(|tasks| tasks.len()).unwrap_or(0);

            if task_count < min_tasks {
                min_tasks = task_count;
                selected_node = node_id.clone();
            }
        }

        Ok(selected_node)
    }

    pub async fn update_task_status(&self, rule_id: &str, status: TaskStatus) -> Result<()> {
        let mut assignments = self.task_assignments.write().await;

        if let Some(assignment) = assignments.get_mut(rule_id) {
            assignment.status = status;
            assignment.last_heartbeat = Utc::now().timestamp_millis();

            debug!("Updated task {} status to {:?}", rule_id, status);
        }

        Ok(())
    }

    pub async fn heartbeat(&self, rule_id: &str) -> Result<()> {
        let mut assignments = self.task_assignments.write().await;

        if let Some(assignment) = assignments.get_mut(rule_id) {
            assignment.last_heartbeat = Utc::now().timestamp_millis();
        }

        Ok(())
    }

    pub async fn get_task_assignment(&self, rule_id: &str) -> Option<TaskAssignment> {
        self.task_assignments.read().await.get(rule_id).cloned()
    }

    pub async fn get_node_tasks(&self, node_id: &str) -> Vec<String> {
        self.node_tasks
            .read()
            .await
            .get(node_id)
            .map(|tasks| tasks.iter().cloned().collect())
            .unwrap_or_default()
    }

    pub async fn remove_task(&self, rule_id: &str) -> Result<()> {
        let mut assignments = self.task_assignments.write().await;
        let mut node_tasks = self.node_tasks.write().await;

        if let Some(assignment) = assignments.remove(rule_id) {
            if let Some(tasks) = node_tasks.get_mut(&assignment.assigned_node) {
                tasks.remove(rule_id);
            }
        }

        info!("Removed task {}", rule_id);

        Ok(())
    }

    pub async fn handle_node_failure(&self, failed_node: &str) -> Result<Vec<String>> {
        warn!("Handling failure of node {}", failed_node);

        let mut assignments = self.task_assignments.write().await;
        let mut node_tasks = self.node_tasks.write().await;
        let mut affected_rules = Vec::new();

        if let Some(tasks) = node_tasks.remove(failed_node) {
            for rule_id in tasks {
                if let Some(assignment) = assignments.get_mut(&rule_id) {
                    assignment.status = TaskStatus::Failed;
                    affected_rules.push(rule_id);
                }
            }
        }

        info!("Node {} failure affected {} tasks", failed_node, affected_rules.len());

        Ok(affected_rules)
    }

    pub async fn get_coordination_stats(&self) -> CoordinationStats {
        let assignments = self.task_assignments.read().await;
        let node_tasks = self.node_tasks.read().await;

        CoordinationStats {
            total_tasks: assignments.len(),
            running_tasks: assignments.values().filter(|a| a.status == TaskStatus::Running).count(),
            pending_tasks: assignments.values().filter(|a| a.status == TaskStatus::Pending).count(),
            failed_tasks: assignments.values().filter(|a| a.status == TaskStatus::Failed).count(),
            active_nodes: node_tasks.len(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct CoordinationStats {
    pub total_tasks: usize,
    pub running_tasks: usize,
    pub pending_tasks: usize,
    pub failed_tasks: usize,
    pub active_nodes: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_task_assignment() {
        let coordinator = DistributedPreAggregationCoordinator::new(
            "node-1".to_string(),
            DistributedPreAggregationConfig::default(),
        );

        let rule = PreAggregationRule::new(
            "rule-1".to_string(),
            "test_rule".to_string(),
            "up".to_string(),
            HashMap::new(),
            false,
        );

        let available_nodes = vec!["node-1".to_string(), "node-2".to_string()];

        let assigned_node = coordinator.assign_task(&rule, &available_nodes).await.unwrap();
        assert!(!assigned_node.is_empty());

        let assignment = coordinator.get_task_assignment("rule-1").await;
        assert!(assignment.is_some());
    }

    #[tokio::test]
    async fn test_task_status_update() {
        let coordinator = DistributedPreAggregationCoordinator::new(
            "node-1".to_string(),
            DistributedPreAggregationConfig::default(),
        );

        let rule = PreAggregationRule::new(
            "rule-1".to_string(),
            "test_rule".to_string(),
            "up".to_string(),
            HashMap::new(),
            false,
        );

        let available_nodes = vec!["node-1".to_string()];
        coordinator.assign_task(&rule, &available_nodes).await.unwrap();

        coordinator.update_task_status("rule-1", TaskStatus::Running).await.unwrap();

        let assignment = coordinator.get_task_assignment("rule-1").await.unwrap();
        assert_eq!(assignment.status, TaskStatus::Running);
    }
}
