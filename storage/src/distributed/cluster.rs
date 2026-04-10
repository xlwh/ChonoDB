use crate::error::Result;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::time::interval;
use tracing::{info, debug, warn};

use super::shard::ShardManager;
use super::replication::ReplicationManager;

#[derive(Debug, Clone)]
pub struct ClusterConfig {
    pub cluster_name: String,
    pub heartbeat_interval_ms: u64,
    pub node_timeout_ms: u64,
    pub discovery_addresses: Vec<String>,
    pub enable_auto_discovery: bool,
}

impl Default for ClusterConfig {
    fn default() -> Self {
        Self {
            cluster_name: "chronodb-cluster".to_string(),
            heartbeat_interval_ms: 5000,
            node_timeout_ms: 15000,
            discovery_addresses: Vec::new(),
            enable_auto_discovery: false,
        }
    }
}

impl ClusterConfig {
    pub fn from_yaml_config(yaml_config: &crate::config::ClusterConfigYaml) -> Self {
        Self {
            cluster_name: "chronodb-cluster".to_string(),
            heartbeat_interval_ms: yaml_config.heartbeat_interval_ms,
            node_timeout_ms: yaml_config.node_timeout_ms,
            discovery_addresses: yaml_config.discovery_addresses.clone(),
            enable_auto_discovery: yaml_config.enable_auto_discovery,
        }
    }
}

#[derive(Debug, Clone)]
pub struct NodeInfo {
    pub node_id: String,
    pub address: String,
    pub status: NodeStatus,
    pub last_heartbeat: i64,
    pub shard_count: u64,
    pub series_count: u64,
    pub is_leader: bool,
    pub version: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeStatus {
    Online,
    Offline,
    Degraded,
}

pub struct ClusterManager {
    config: ClusterConfig,
    nodes: Arc<RwLock<HashMap<String, NodeInfo>>>,
    leader_id: Arc<RwLock<Option<String>>>,
    current_node_id: Arc<RwLock<Option<String>>>,
    current_node_address: Arc<RwLock<Option<String>>>,
    discovery_task: Arc<RwLock<Option<tokio::task::JoinHandle<()>>>>,
    heartbeat_task: Arc<RwLock<Option<tokio::task::JoinHandle<()>>>>,
    heartbeat_sender_task: Arc<RwLock<Option<tokio::task::JoinHandle<()>>>>,
    rpc_manager: Option<Arc<crate::rpc::ClusterRpcManager>>,
    shard_manager: Option<Arc<ShardManager>>,
    replication_manager: Option<Arc<ReplicationManager>>,
}

impl ClusterManager {
    pub fn new(config: ClusterConfig) -> Self {
        Self {
            config,
            nodes: Arc::new(RwLock::new(HashMap::new())),
            leader_id: Arc::new(RwLock::new(None)),
            current_node_id: Arc::new(RwLock::new(None)),
            current_node_address: Arc::new(RwLock::new(None)),
            discovery_task: Arc::new(RwLock::new(None)),
            heartbeat_task: Arc::new(RwLock::new(None)),
            heartbeat_sender_task: Arc::new(RwLock::new(None)),
            rpc_manager: None,
            shard_manager: None,
            replication_manager: None,
        }
    }

    pub fn with_rpc_manager(mut self, rpc_manager: Arc<crate::rpc::ClusterRpcManager>) -> Self {
        self.rpc_manager = Some(rpc_manager);
        self
    }

    pub fn with_shard_manager(mut self, shard_manager: Arc<ShardManager>) -> Self {
        self.shard_manager = Some(shard_manager);
        self
    }

    pub fn with_replication_manager(mut self, replication_manager: Arc<ReplicationManager>) -> Self {
        self.replication_manager = Some(replication_manager);
        self
    }

    pub async fn set_current_node(&self, node_id: String, address: String) {
        let mut node_id_write = self.current_node_id.write().await;
        *node_id_write = Some(node_id);

        let mut address_write = self.current_node_address.write().await;
        *address_write = Some(address);
    }

    pub async fn start(&self) -> Result<()> {
        info!("Starting cluster manager for cluster: {}", self.config.cluster_name);

        if self.config.enable_auto_discovery {
            self.start_discovery_task().await?;
        }

        self.start_heartbeat_task().await?;

        if self.rpc_manager.is_some() {
            self.start_heartbeat_sender_task().await?;
        }

        Ok(())
    }

    async fn start_discovery_task(&self) -> Result<()> {
        let discovery_addresses = self.config.discovery_addresses.clone();
        let nodes = self.nodes.clone();

        let handle = tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(30));

            loop {
                interval.tick().await;

                for addr in &discovery_addresses {
                    if let Err(e) = discover_nodes(addr, &nodes).await {
                        warn!("Node discovery failed for {}: {:?}", addr, e);
                    }
                }
            }
        });

        let mut discovery_task_write = self.discovery_task.write().await;
        *discovery_task_write = Some(handle);
        Ok(())
    }

    async fn start_heartbeat_task(&self) -> Result<()> {
        let config = self.config.clone();
        let nodes = self.nodes.clone();
        let leader_id = self.leader_id.clone();
        let shard_manager = self.shard_manager.clone();
        let replication_manager = self.replication_manager.clone();

        let handle = tokio::spawn(async move {
            let mut interval = interval(Duration::from_millis(config.heartbeat_interval_ms));

            loop {
                interval.tick().await;

                let now = chrono::Utc::now().timestamp_millis();
                let mut nodes_write = nodes.write().await;

                let mut failed_nodes: Vec<String> = Vec::new();
                let mut leader_timed_out = false;

                for (node_id, node) in &*nodes_write {
                    if now - node.last_heartbeat > config.node_timeout_ms as i64 {
                        warn!("Node {} timed out", node_id);
                        failed_nodes.push(node_id.clone());

                        if node.is_leader {
                            info!("Leader node {} timed out", node_id);
                            leader_timed_out = true;
                        }
                    }
                }

                if leader_timed_out {
                    *leader_id.write().await = None;
                }

                for failed_node_id in &failed_nodes {
                    if let Some(node) = nodes_write.get_mut(failed_node_id) {
                        node.status = NodeStatus::Offline;
                    }
                }

                let healthy_nodes: Vec<String> = nodes_write
                    .values()
                    .filter(|n| n.status == NodeStatus::Online)
                    .map(|n| n.node_id.clone())
                    .collect();

                for failed_node_id in &failed_nodes {
                    info!("Processing failover for node: {}", failed_node_id);

                    if let Some(ref sm) = shard_manager {
                        match sm.handle_node_failure(failed_node_id, &healthy_nodes).await {
                            Ok(()) => info!("Shard manager handled failure of node {}", failed_node_id),
                            Err(e) => warn!("Shard manager failed to handle node {}: {}", failed_node_id, e),
                        }
                    }

                    if let Some(ref rm) = replication_manager {
                        match rm.handle_node_failure(failed_node_id, &healthy_nodes).await {
                            Ok(()) => info!("Replication manager handled failure of node {}", failed_node_id),
                            Err(e) => warn!("Replication manager failed to handle node {}: {}", failed_node_id, e),
                        }
                    }
                }

                if leader_timed_out && !healthy_nodes.is_empty() {
                    info!("Re-electing leader from {} healthy nodes", healthy_nodes.len());
                }

                for failed_node_id in failed_nodes {
                    nodes_write.remove(&failed_node_id);
                    info!("Removed failed node: {}", failed_node_id);
                }
            }
        });

        let mut heartbeat_task_write = self.heartbeat_task.write().await;
        *heartbeat_task_write = Some(handle);
        Ok(())
    }

    pub async fn register_node(&self, node_info: NodeInfo) -> Result<()> {
        let node_id = node_info.node_id.clone();
        let mut nodes = self.nodes.write().await;
        nodes.insert(node_id.clone(), node_info);
        info!("Node registered: {}", node_id);

        self.check_leader_election().await?;

        Ok(())
    }

    pub async fn update_heartbeat(&self, node_id: &str) -> Result<()> {
        let mut nodes = self.nodes.write().await;
        if let Some(node) = nodes.get_mut(node_id) {
            node.last_heartbeat = chrono::Utc::now().timestamp_millis();
            node.status = NodeStatus::Online;
        }
        Ok(())
    }

    pub async fn get_nodes(&self) -> Result<Vec<NodeInfo>> {
        let nodes = self.nodes.read().await;
        Ok(nodes.values().cloned().collect())
    }

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

    pub async fn remove_node(&self, node_id: &str) -> Result<()> {
        let mut nodes = self.nodes.write().await;
        nodes.remove(node_id);
        info!("Node removed: {}", node_id);

        if let Some(leader) = self.leader_id.read().await.as_ref() {
            if leader == node_id {
                info!("Leader node {} removed, starting re-election", node_id);
                *self.leader_id.write().await = None;
                self.check_leader_election().await?;
            }
        }

        Ok(())
    }

    pub async fn check_leader_election(&self) -> Result<()> {
        let leader = self.leader_id.read().await.clone();
        if leader.is_none() {
            self.elect_leader().await?;
        }
        Ok(())
    }

    async fn elect_leader(&self) -> Result<()> {
        let nodes = self.nodes.read().await;
        let healthy_nodes: Vec<&NodeInfo> = nodes
            .values()
            .filter(|n| n.status == NodeStatus::Online)
            .collect();

        if !healthy_nodes.is_empty() {
            let mut candidates = healthy_nodes.clone();

            candidates.sort_by(|a, b| {
                if a.is_leader && !b.is_leader {
                    return std::cmp::Ordering::Less;
                } else if !a.is_leader && b.is_leader {
                    return std::cmp::Ordering::Greater;
                }

                if a.shard_count < b.shard_count {
                    return std::cmp::Ordering::Less;
                } else if a.shard_count > b.shard_count {
                    return std::cmp::Ordering::Greater;
                }

                a.node_id.cmp(&b.node_id)
            });

            let leader = candidates[0];
            *self.leader_id.write().await = Some(leader.node_id.clone());

            let mut nodes_write = self.nodes.write().await;
            for (nid, node) in &mut *nodes_write {
                node.is_leader = *nid == leader.node_id;
            }

            info!("Elected new leader: {}, shard count: {}", leader.node_id, leader.shard_count);
        }

        Ok(())
    }

    pub async fn handle_node_failure(&self, node_id: &str) -> Result<()> {
        info!("Handling node failure: {}", node_id);

        let mut nodes_write = self.nodes.write().await;
        if let Some(node) = nodes_write.get_mut(node_id) {
            node.status = NodeStatus::Offline;
            info!("Marked node {} as offline", node_id);
        }

        if let Some(leader) = self.leader_id.read().await.as_ref() {
            if leader == node_id {
                info!("Leader node {} failed, starting re-election", node_id);
                *self.leader_id.write().await = None;
                self.elect_leader().await?;
            }
        }

        let healthy_nodes: Vec<String> = nodes_write
            .values()
            .filter(|n| n.status == NodeStatus::Online)
            .map(|n| n.node_id.clone())
            .collect();

        if !healthy_nodes.is_empty() {
            info!("Triggering failover with healthy nodes: {:?}", healthy_nodes);
            self.trigger_failover(node_id, &healthy_nodes).await?;
        }

        nodes_write.remove(node_id);
        info!("Removed failed node: {}", node_id);

        info!("Failover completed for node: {}, healthy nodes remaining: {}",
              node_id, healthy_nodes.len());

        Ok(())
    }

    pub async fn trigger_failover(&self, failed_node_id: &str, healthy_nodes: &[String]) -> Result<()> {
        info!("Triggering failover for node: {}, healthy nodes: {:?}", failed_node_id, healthy_nodes);

        if healthy_nodes.is_empty() {
            warn!("No healthy nodes available for failover");
            return Ok(());
        }

        if let Some(leader) = self.leader_id.read().await.as_ref() {
            if leader == failed_node_id {
                info!("Leader node failed, starting re-election");
                *self.leader_id.write().await = None;
                self.elect_leader().await?;
            }
        }

        if let Some(ref sm) = self.shard_manager {
            match sm.handle_node_failure(failed_node_id, healthy_nodes).await {
                Ok(()) => info!("Shard manager reassigned shards from failed node {}", failed_node_id),
                Err(e) => warn!("Shard manager failed to handle node {}: {}", failed_node_id, e),
            }
        }

        if let Some(ref rm) = self.replication_manager {
            match rm.handle_node_failure(failed_node_id, healthy_nodes).await {
                Ok(()) => info!("Replication manager updated for failed node {}", failed_node_id),
                Err(e) => warn!("Replication manager failed to handle node {}: {}", failed_node_id, e),
            }
        }

        info!("Failover completed: node {} failed, {} healthy nodes available",
              failed_node_id, healthy_nodes.len());

        Ok(())
    }

    pub async fn get_leader(&self) -> Result<Option<NodeInfo>> {
        if let Some(leader_id) = self.leader_id.read().await.as_ref() {
            let nodes = self.nodes.read().await;
            if let Some(leader) = nodes.get(leader_id) {
                return Ok(Some(leader.clone()));
            }
        }
        Ok(None)
    }

    async fn start_heartbeat_sender_task(&self) -> Result<()> {
        let config = self.config.clone();
        let nodes = self.nodes.clone();
        let rpc_manager = self.rpc_manager.clone();
        let current_node_id = self.current_node_id.clone();

        let handle = tokio::spawn(async move {
            let mut interval = interval(Duration::from_millis(config.heartbeat_interval_ms));

            loop {
                interval.tick().await;

                let current_node_id_opt = current_node_id.read().await.clone();

                if let (Some(node_id), Some(rpc_mgr)) = (current_node_id_opt, &rpc_manager) {
                    let clients = rpc_mgr.get_all_clients().await;

                    for (peer_node_id, client) in clients {
                        if peer_node_id != node_id {
                            match client.heartbeat(node_id.clone()).await {
                                Ok(response) if response.success => {
                                    debug!("Heartbeat sent to node: {}", peer_node_id);
                                }
                                Ok(_) => {
                                    warn!("Heartbeat failed for node {}", peer_node_id);
                                }
                                Err(e) => {
                                    warn!("Failed to send heartbeat to node {}: {}", peer_node_id, e);
                                }
                            }
                        }
                    }
                }
            }
        });

        let mut heartbeat_sender_task_write = self.heartbeat_sender_task.write().await;
        *heartbeat_sender_task_write = Some(handle);
        Ok(())
    }

    pub async fn stop(&self) -> Result<()> {
        let mut discovery_task_write = self.discovery_task.write().await;
        if let Some(task) = discovery_task_write.take() {
            task.abort();
        }

        let mut heartbeat_task_write = self.heartbeat_task.write().await;
        if let Some(task) = heartbeat_task_write.take() {
            task.abort();
        }

        let mut heartbeat_sender_task_write = self.heartbeat_sender_task.write().await;
        if let Some(task) = heartbeat_sender_task_write.take() {
            task.abort();
        }

        Ok(())
    }
}

async fn discover_nodes(addr: &str, nodes: &Arc<RwLock<HashMap<String, NodeInfo>>>) -> Result<()> {
    debug!("Discovering nodes from {}", addr);

    if let Ok(socket_addr) = addr.parse::<std::net::SocketAddr>() {
        let client = crate::rpc::RpcClient::new(socket_addr);

        match client.get_cluster_status().await {
            Ok(cluster_status) => {
                info!("Successfully discovered cluster from {}", addr);

                let mut nodes_write = nodes.write().await;
                for node_info in cluster_status.nodes {
                    let node_id_clone = node_info.node_id.clone();
                    let node = NodeInfo {
                        node_id: node_info.node_id,
                        address: node_info.address,
                        status: match node_info.status {
                            crate::rpc::NodeStatus::Online => NodeStatus::Online,
                            crate::rpc::NodeStatus::Offline => NodeStatus::Offline,
                            crate::rpc::NodeStatus::Suspect => NodeStatus::Degraded,
                        },
                        last_heartbeat: node_info.last_heartbeat,
                        shard_count: 0,
                        series_count: 0,
                        is_leader: cluster_status.is_coordinator && cluster_status.node_id == node_id_clone,
                        version: "1.0.0".to_string(),
                    };

                    let node_id = node.node_id.clone();
                    if !nodes_write.contains_key(&node_id) {
                        nodes_write.insert(node_id.clone(), node);
                        info!("Discovered new node: {}", node_id);
                    }
                }
            }
            Err(e) => {
                warn!("Failed to discover nodes from {}: {}", addr, e);
            }
        }
    } else {
        debug!("Address {} is not a socket address, trying other discovery methods", addr);
    }

    Ok(())
}
