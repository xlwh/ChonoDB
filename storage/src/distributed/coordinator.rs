use crate::error::Result;
use crate::model::TimeSeriesId;
use crate::query::planner::{PlanType, QueryPlan};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::time::interval;
use tracing::{debug, info};

use super::shard::ShardManager;

#[derive(Debug, Clone)]
pub struct CoordinatorConfig {
    pub address: String,
    pub heartbeat_interval_ms: u64,
    pub node_timeout_ms: u64,
}

impl Default for CoordinatorConfig {
    fn default() -> Self {
        Self {
            address: "127.0.0.1:9091".to_string(),
            heartbeat_interval_ms: 5000,
            node_timeout_ms: 15000,
        }
    }
}

pub struct Coordinator {
    config: CoordinatorConfig,
    nodes: Arc<RwLock<HashMap<String, NodeStatus>>>,
    health_check_task: Arc<RwLock<Option<tokio::task::JoinHandle<()>>>>,
}

#[derive(Debug, Clone)]
pub struct NodeStatus {
    pub node_id: String,
    pub address: String,
    pub last_heartbeat: i64,
    pub is_healthy: bool,
}

impl Coordinator {
    pub fn new(config: CoordinatorConfig) -> Self {
        Self {
            config,
            nodes: Arc::new(RwLock::new(HashMap::new())),
            health_check_task: Arc::new(RwLock::new(None)),
        }
    }

    pub async fn start(&self) -> Result<()> {
        info!("Starting coordinator on {}", self.config.address);

        let nodes = self.nodes.clone();
        let config = self.config.clone();

        let health_check_handle = tokio::spawn(async move {
            let mut interval = interval(Duration::from_millis(config.heartbeat_interval_ms));

            loop {
                interval.tick().await;

                let now = chrono::Utc::now().timestamp_millis();
                let mut nodes_write = nodes.write().await;

                for (_, node) in nodes_write.iter_mut() {
                    if now - node.last_heartbeat > config.node_timeout_ms as i64 {
                        if node.is_healthy {
                            info!("Node {} marked as unhealthy", node.node_id);
                            node.is_healthy = false;
                        }
                    }
                }
            }
        });

        let mut task = self.health_check_task.write().await;
        *task = Some(health_check_handle);

        Ok(())
    }

    pub async fn stop(&self) -> Result<()> {
        let mut task = self.health_check_task.write().await;
        if let Some(handle) = task.take() {
            handle.abort();
        }
        Ok(())
    }

    pub async fn register_node(&self, node_id: String, address: String) -> Result<()> {
        let mut nodes = self.nodes.write().await;
        nodes.insert(
            node_id.clone(),
            NodeStatus {
                node_id,
                address,
                last_heartbeat: chrono::Utc::now().timestamp_millis(),
                is_healthy: true,
            },
        );
        Ok(())
    }

    pub async fn update_heartbeat(&self, node_id: &str) -> Result<()> {
        let mut nodes = self.nodes.write().await;
        if let Some(node) = nodes.get_mut(node_id) {
            node.last_heartbeat = chrono::Utc::now().timestamp_millis();
            node.is_healthy = true;
        }
        Ok(())
    }

    pub async fn get_healthy_nodes(&self) -> Result<Vec<String>> {
        let nodes = self.nodes.read().await;
        let now = chrono::Utc::now().timestamp_millis();

        Ok(nodes
            .values()
            .filter(|n| n.is_healthy && now - n.last_heartbeat < self.config.node_timeout_ms as i64)
            .map(|n| n.node_id.clone())
            .collect())
    }
}

/// 查询路由器 - 基于分片信息智能路由查询
pub struct QueryRouter {
    shard_manager: Arc<RwLock<ShardManager>>,
}

impl QueryRouter {
    pub fn new(shard_manager: Arc<RwLock<ShardManager>>) -> Self {
        Self { shard_manager }
    }

    /// 根据查询计划和 series IDs 路由查询到目标节点
    pub async fn route_by_series(
        &self,
        series_ids: &[TimeSeriesId],
    ) -> Result<HashMap<String, Vec<TimeSeriesId>>> {
        let shard_manager = self.shard_manager.read().await;
        let distribution = shard_manager.get_shard_distribution().await?;

        let mut shard_to_primary: HashMap<u64, String> = HashMap::new();
        for (shard_id, nodes) in &distribution {
            if let Some(primary) = nodes.first() {
                if !primary.is_empty() {
                    shard_to_primary.insert(*shard_id, primary.clone());
                }
            }
        }

        let mut node_series: HashMap<String, Vec<TimeSeriesId>> = HashMap::new();
        for &series_id in series_ids {
            let shard_id = shard_manager.get_shard_for_series(series_id);
            if let Some(node_id) = shard_to_primary.get(&shard_id) {
                node_series.entry(node_id.clone()).or_default().push(series_id);
            }
        }

        debug!(
            "Routed {} series to {} nodes",
            series_ids.len(),
            node_series.len()
        );

        Ok(node_series)
    }

    /// 从查询计划中提取 matchers
    pub fn extract_matchers(plan: &QueryPlan) -> Vec<(String, String)> {
        Self::extract_matchers_from_plan(&plan.plan_type)
    }

    fn extract_matchers_from_plan(plan_type: &PlanType) -> Vec<(String, String)> {
        match plan_type {
            PlanType::VectorQuery(vq) => vq.matchers.clone(),
            PlanType::MatrixQuery(mq) => mq.vector_plan.matchers.clone(),
            PlanType::Call(call) => {
                for arg in &call.args {
                    let matchers = Self::extract_matchers_from_plan(&arg.plan_type);
                    if !matchers.is_empty() {
                        return matchers;
                    }
                }
                Vec::new()
            }
            PlanType::BinaryExpr(bin) => {
                let lhs = Self::extract_matchers_from_plan(&bin.lhs.plan_type);
                if !lhs.is_empty() {
                    return lhs;
                }
                Self::extract_matchers_from_plan(&bin.rhs.plan_type)
            }
            PlanType::UnaryExpr(unary) => {
                Self::extract_matchers_from_plan(&unary.expr.plan_type)
            }
            PlanType::Aggregation(agg) => {
                Self::extract_matchers_from_plan(&agg.expr.plan_type)
            }
        }
    }

    /// 路由查询到目标节点（兼容旧接口）
    pub fn route(&self, plan: &QueryPlan, nodes: &[String]) -> Result<HashMap<String, QueryPlan>> {
        let mut routes = HashMap::new();
        for node in nodes {
            routes.insert(node.clone(), plan.clone());
        }
        Ok(routes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_coordinator_config_default() {
        let config = CoordinatorConfig::default();
        assert_eq!(config.address, "127.0.0.1:9091");
        assert_eq!(config.heartbeat_interval_ms, 5000);
        assert_eq!(config.node_timeout_ms, 15000);
    }

    #[tokio::test]
    async fn test_coordinator_new() {
        let config = CoordinatorConfig::default();
        let coordinator = Coordinator::new(config);
        let healthy = coordinator.get_healthy_nodes().await.unwrap();
        assert!(healthy.is_empty());
    }

    #[tokio::test]
    async fn test_register_and_get_healthy_nodes() {
        let config = CoordinatorConfig::default();
        let coordinator = Coordinator::new(config);

        coordinator.register_node("node-1".to_string(), "127.0.0.1:9090".to_string()).await.unwrap();

        let healthy = coordinator.get_healthy_nodes().await.unwrap();
        assert_eq!(healthy.len(), 1);
        assert_eq!(healthy[0], "node-1");
    }

    #[tokio::test]
    async fn test_register_multiple_nodes() {
        let config = CoordinatorConfig::default();
        let coordinator = Coordinator::new(config);

        coordinator.register_node("node-1".to_string(), "127.0.0.1:9090".to_string()).await.unwrap();
        coordinator.register_node("node-2".to_string(), "127.0.0.1:9091".to_string()).await.unwrap();

        let healthy = coordinator.get_healthy_nodes().await.unwrap();
        assert_eq!(healthy.len(), 2);
    }

    #[tokio::test]
    async fn test_update_heartbeat() {
        let config = CoordinatorConfig::default();
        let coordinator = Coordinator::new(config);

        coordinator.register_node("node-1".to_string(), "127.0.0.1:9090".to_string()).await.unwrap();
        coordinator.update_heartbeat("node-1").await.unwrap();

        let healthy = coordinator.get_healthy_nodes().await.unwrap();
        assert_eq!(healthy.len(), 1);
    }

    #[tokio::test]
    async fn test_update_heartbeat_nonexistent() {
        let config = CoordinatorConfig::default();
        let coordinator = Coordinator::new(config);

        let result = coordinator.update_heartbeat("nonexistent").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_stop_without_start() {
        let config = CoordinatorConfig::default();
        let coordinator = Coordinator::new(config);
        let result = coordinator.stop().await;
        assert!(result.is_ok());
    }

    #[test]
    fn test_node_status_creation() {
        let status = NodeStatus {
            node_id: "node-1".to_string(),
            address: "127.0.0.1:9090".to_string(),
            last_heartbeat: 1000,
            is_healthy: true,
        };
        assert_eq!(status.node_id, "node-1");
        assert!(status.is_healthy);
    }
}
