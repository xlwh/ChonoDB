use crate::error::Result;
use crate::query::planner::QueryPlan;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

/// 协调器配置
#[derive(Debug, Clone)]
pub struct CoordinatorConfig {
    /// 协调器地址
    pub address: String,
    /// 心跳间隔（毫秒）
    pub heartbeat_interval_ms: u64,
    /// 节点超时时间（毫秒）
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

/// 协调器
pub struct Coordinator {
    config: CoordinatorConfig,
    nodes: Arc<RwLock<HashMap<String, NodeStatus>>>,
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
        }
    }

    pub async fn start(&self) -> Result<()> {
        info!("Starting coordinator on {}", self.config.address);
        // 启动协调服务
        Ok(())
    }

    pub async fn register_node(&self, node_id: String, address: String) -> Result<()> {
        let mut nodes = self.nodes.write().await;
        nodes.insert(node_id.clone(), NodeStatus {
            node_id,
            address,
            last_heartbeat: chrono::Utc::now().timestamp_millis(),
            is_healthy: true,
        });
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

/// 查询路由器
pub struct QueryRouter;

impl QueryRouter {
    pub fn new() -> Self {
        Self
    }

    pub fn route(&self, plan: &QueryPlan, nodes: &[String]) -> Result<HashMap<String, QueryPlan>> {
        let mut routes = HashMap::new();
        
        // 简化实现：将查询路由到所有节点
        for node in nodes {
            routes.insert(node.clone(), plan.clone());
        }
        
        Ok(routes)
    }
}

impl Default for QueryRouter {
    fn default() -> Self {
        Self::new()
    }
}
