use crate::error::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, debug};

/// 集群配置
#[derive(Debug, Clone)]
pub struct ClusterConfig {
    /// 集群名称
    pub cluster_name: String,
    /// 节点心跳间隔（毫秒）
    pub heartbeat_interval_ms: u64,
    /// 节点超时时间（毫秒）
    pub node_timeout_ms: u64,
}

impl Default for ClusterConfig {
    fn default() -> Self {
        Self {
            cluster_name: "chronodb-cluster".to_string(),
            heartbeat_interval_ms: 5000,
            node_timeout_ms: 15000,
        }
    }
}

/// 节点信息
#[derive(Debug, Clone)]
pub struct NodeInfo {
    pub node_id: String,
    pub address: String,
    pub status: NodeStatus,
    pub last_heartbeat: i64,
    pub shard_count: u64,
    pub series_count: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeStatus {
    Online,
    Offline,
    Degraded,
}

/// 集群管理器
pub struct ClusterManager {
    config: ClusterConfig,
    nodes: Arc<RwLock<HashMap<String, NodeInfo>>>,
}

impl ClusterManager {
    pub fn new(config: ClusterConfig) -> Self {
        Self {
            config,
            nodes: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn start(&self) -> Result<()> {
        info!("Starting cluster manager for cluster: {}", self.config.cluster_name);
        Ok(())
    }

    /// 注册节点
    pub async fn register_node(&self, node_info: NodeInfo) -> Result<()> {
        let node_id = node_info.node_id.clone();
        let mut nodes = self.nodes.write().await;
        nodes.insert(node_id.clone(), node_info);
        info!("Node registered: {}", node_id);
        Ok(())
    }

    /// 更新节点心跳
    pub async fn update_heartbeat(&self, node_id: &str) -> Result<()> {
        let mut nodes = self.nodes.write().await;
        if let Some(node) = nodes.get_mut(node_id) {
            node.last_heartbeat = chrono::Utc::now().timestamp_millis();
            node.status = NodeStatus::Online;
        }
        Ok(())
    }

    /// 获取所有节点
    pub async fn get_nodes(&self) -> Result<Vec<NodeInfo>> {
        let nodes = self.nodes.read().await;
        Ok(nodes.values().cloned().collect())
    }

    /// 获取健康节点
    pub async fn get_healthy_nodes(&self) -> Result<Vec<String>> {
        let nodes = self.nodes.read().await;
        let now = chrono::Utc::now().timestamp_millis();
        
        Ok(nodes
            .values()
            .filter(|n| {
                n.status == NodeStatus::Online 
                    && now - n.last_heartbeat < self.config.node_timeout_ms as i64
            })
            .map(|n| n.node_id.clone())
            .collect())
    }

    /// 移除节点
    pub async fn remove_node(&self, node_id: &str) -> Result<()> {
        let mut nodes = self.nodes.write().await;
        nodes.remove(node_id);
        info!("Node removed: {}", node_id);
        Ok(())
    }
}
