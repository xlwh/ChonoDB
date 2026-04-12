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
    pub fn from_yaml_config(yaml_config: &crate::config::ClusterConfigYaml, cluster_name: String) -> Self {
        Self {
            cluster_name,
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
    Degraded,
    Offline,
    Suspect,
}

impl NodeStatus {
    pub fn is_available(&self) -> bool {
        matches!(self, NodeStatus::Online | NodeStatus::Degraded)
    }
    
    pub fn is_healthy(&self) -> bool {
        matches!(self, NodeStatus::Online)
    }
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

            // 用于跟踪连续超时次数
            let mut timeout_counts: HashMap<String, u32> = HashMap::new();
            // 降级超时阈值（毫秒）
            let degraded_timeout_ms = config.node_timeout_ms / 2;

            loop {
                interval.tick().await;

                let now = chrono::Utc::now().timestamp_millis();
                let mut nodes_write = nodes.write().await;

                let mut failed_nodes: Vec<String> = Vec::new();
                let mut degraded_nodes: Vec<String> = Vec::new();
                let mut recovered_nodes: Vec<String> = Vec::new();
                let mut leader_timed_out = false;

                for (node_id, node) in &mut *nodes_write {
                    let time_since_heartbeat = now - node.last_heartbeat;
                    
                    // 获取当前超时次数，默认为0
                    let timeout_count = timeout_counts.entry(node_id.clone()).or_insert(0);

                    match node.status {
                        NodeStatus::Online => {
                            if time_since_heartbeat > config.node_timeout_ms as i64 {
                                warn!("Node {} timed out ({}ms)", node_id, time_since_heartbeat);
                                *timeout_count += 1;
                                
                                // 连续超时3次才标记为离线
                                if *timeout_count >= 3 {
                                    failed_nodes.push(node_id.clone());
                                    node.status = NodeStatus::Offline;
                                    
                                    if node.is_leader {
                                        info!("Leader node {} timed out after {} attempts", node_id, timeout_count);
                                        leader_timed_out = true;
                                    }
                                } else {
                                    // 首次超时标记为可疑状态
                                    node.status = NodeStatus::Suspect;
                                    warn!("Node {} marked as suspect (attempt {})", node_id, timeout_count);
                                }
                            } else if time_since_heartbeat > degraded_timeout_ms as i64 {
                                // 延迟较高但未超时，标记为降级状态
                                node.status = NodeStatus::Degraded;
                                degraded_nodes.push(node_id.clone());
                                debug!("Node {} marked as degraded (latency: {}ms)", node_id, time_since_heartbeat);
                            }
                        }
                        NodeStatus::Degraded => {
                            if time_since_heartbeat > config.node_timeout_ms as i64 {
                                *timeout_count += 1;
                                if *timeout_count >= 3 {
                                    failed_nodes.push(node_id.clone());
                                    node.status = NodeStatus::Offline;
                                } else {
                                    node.status = NodeStatus::Suspect;
                                }
                            } else if time_since_heartbeat <= degraded_timeout_ms as i64 {
                                // 恢复正常
                                node.status = NodeStatus::Online;
                                *timeout_count = 0;
                                recovered_nodes.push(node_id.clone());
                                info!("Node {} recovered from degraded state", node_id);
                            }
                        }
                        NodeStatus::Suspect => {
                            if time_since_heartbeat > config.node_timeout_ms as i64 {
                                *timeout_count += 1;
                                if *timeout_count >= 3 {
                                    failed_nodes.push(node_id.clone());
                                    node.status = NodeStatus::Offline;
                                }
                            } else {
                                // 恢复正常
                                node.status = NodeStatus::Online;
                                *timeout_count = 0;
                                recovered_nodes.push(node_id.clone());
                                info!("Node {} recovered from suspect state", node_id);
                            }
                        }
                        NodeStatus::Offline => {
                            // 离线节点保持离线状态
                        }
                    }
                }

                if leader_timed_out {
                    *leader_id.write().await = None;
                }

                // 获取所有可用节点（Online + Degraded）
                let available_nodes: Vec<String> = nodes_write
                    .values()
                    .filter(|n| n.status.is_available())
                    .map(|n| n.node_id.clone())
                    .collect();

                // 获取健康节点（仅 Online）
                let healthy_nodes: Vec<String> = nodes_write
                    .values()
                    .filter(|n| n.status.is_healthy())
                    .map(|n| n.node_id.clone())
                    .collect();

                // 处理降级节点（不触发故障转移，但记录日志）
                for degraded_node in &degraded_nodes {
                    warn!("Node {} is in degraded state, consider investigating", degraded_node);
                }

                // 处理故障节点
                for failed_node_id in &failed_nodes {
                    info!("Processing failover for node: {}", failed_node_id);

                    if let Some(ref sm) = shard_manager {
                        match sm.handle_node_failure(failed_node_id, &available_nodes).await {
                            Ok(()) => info!("Shard manager handled failure of node {}", failed_node_id),
                            Err(e) => warn!("Shard manager failed to handle node {}: {}", failed_node_id, e),
                        }
                    }

                    if let Some(ref rm) = replication_manager {
                        match rm.handle_node_failure(failed_node_id, &available_nodes).await {
                            Ok(()) => info!("Replication manager handled failure of node {}", failed_node_id),
                            Err(e) => warn!("Replication manager failed to handle node {}: {}", failed_node_id, e),
                        }
                    }
                }

                // 处理恢复节点（重新加入集群）
                for recovered_node in &recovered_nodes {
                    info!("Node {} recovered, re-adding to cluster", recovered_node);
                    if let Some(ref sm) = shard_manager {
                        if let Err(e) = sm.rebalance_shards(&available_nodes).await {
                            warn!("Failed to rebalance shards after node recovery: {}", e);
                        }
                    }
                }

                // 重新选举 leader（如果 leader 超时且有健康节点）
                if leader_timed_out && !healthy_nodes.is_empty() {
                    info!("Re-electing leader from {} healthy nodes", healthy_nodes.len());
                }

                // 从超时计数中移除恢复的节点
                for recovered_node in &recovered_nodes {
                    timeout_counts.remove(recovered_node);
                }

                // 从超时计数中移除故障节点（稍后会从nodes中移除）
                for failed_node_id in &failed_nodes {
                    timeout_counts.remove(failed_node_id);
                }

                // 移除故障节点
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
        {
            let mut nodes = self.nodes.write().await;
            nodes.insert(node_id.clone(), node_info);
        }
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
        {
            let mut nodes = self.nodes.write().await;
            nodes.remove(node_id);
        }
        info!("Node removed: {}", node_id);

        let needs_relection = {
            let leader = self.leader_id.read().await;
            leader.as_ref().map(|l| l == node_id).unwrap_or(false)
        };

        if needs_relection {
            info!("Leader node {} removed, starting re-election", node_id);
            *self.leader_id.write().await = None;
            self.check_leader_election().await?;
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
        let leader_node_id: Option<String>;
        let mut leader_shard_count: u64 = 0;
        
        {
            let nodes = self.nodes.read().await;
            let healthy_nodes: Vec<&NodeInfo> = nodes
                .values()
                .filter(|n| n.status.is_healthy())
                .collect();

            if healthy_nodes.is_empty() {
                info!("No healthy nodes available for leader election");
                return Ok(());
            }

            // 改进的 leader 选举算法：
            // 1. 优先选择已有 leader 标记的节点（如果仍然健康）
            // 2. 选择分片数量最少的节点（负载均衡）
            // 3. 考虑节点版本（新版本优先）
            // 4. 最后按节点ID排序（确定性）
            let mut candidates = healthy_nodes.clone();

            candidates.sort_by(|a, b| {
                // 1. 当前 leader 优先
                if a.is_leader && !b.is_leader {
                    return std::cmp::Ordering::Less;
                } else if !a.is_leader && b.is_leader {
                    return std::cmp::Ordering::Greater;
                }

                // 2. 分片数量最少优先（负载均衡）
                if a.shard_count < b.shard_count {
                    return std::cmp::Ordering::Less;
                } else if a.shard_count > b.shard_count {
                    return std::cmp::Ordering::Greater;
                }

                // 3. 版本号较新优先
                let version_cmp = compare_versions(&a.version, &b.version);
                if version_cmp != std::cmp::Ordering::Equal {
                    return version_cmp.reverse(); // 新版本优先
                }

                // 4. 节点ID排序（确定性）
                a.node_id.cmp(&b.node_id)
            });

            leader_node_id = Some(candidates[0].node_id.clone());
            leader_shard_count = candidates[0].shard_count;
        }

        if let Some(leader) = leader_node_id {
            *self.leader_id.write().await = Some(leader.clone());

            let mut nodes_write = self.nodes.write().await;
            for (nid, node) in &mut *nodes_write {
                node.is_leader = *nid == leader;
            }

            info!("Elected new leader: {}, shard count: {}", leader, leader_shard_count);
        }

        Ok(())
    }

    /// 触发主从切换
    pub async fn trigger_failover_switch(&self, failed_node_id: &str) -> Result<()> {
        info!("Triggering failover switch for failed node: {}", failed_node_id);

        let current_leader = self.get_leader().await?;
        
        if current_leader.as_ref().map(|l| l.node_id == failed_node_id).unwrap_or(false) {
            info!("Leader node {} failed, initiating leader re-election", failed_node_id);
            
            *self.leader_id.write().await = None;
            
            self.elect_leader().await?;
            
            if let Some(new_leader) = self.get_leader().await? {
                info!("Successfully elected new leader: {}", new_leader.node_id);
                
                if let Some(ref rpc_manager) = self.rpc_manager {
                    let clients = rpc_manager.get_all_clients().await;
                    for (node_id, _client) in clients {
                        if node_id != new_leader.node_id {
                            debug!("Will notify node {} of new leader {}", node_id, new_leader.node_id);
                        }
                    }
                }
            } else {
                warn!("Failed to elect new leader after failover");
            }
        }

        Ok(())
    }
}

/// 比较版本号
fn compare_versions(v1: &str, v2: &str) -> std::cmp::Ordering {
    let parts1: Vec<u32> = v1.split('.').filter_map(|s| s.parse().ok()).collect();
    let parts2: Vec<u32> = v2.split('.').filter_map(|s| s.parse().ok()).collect();
    
    for (a, b) in parts1.iter().zip(parts2.iter()) {
        match a.cmp(b) {
            std::cmp::Ordering::Equal => continue,
            other => return other,
        }
    }
    
    parts1.len().cmp(&parts2.len())
}

impl ClusterManager {
    pub async fn handle_node_failure(&self, node_id: &str) -> Result<()> {
        info!("Handling node failure: {}", node_id);

        let is_leader_failure;
        let healthy_nodes: Vec<String>;
        
        {
            let mut nodes_write = self.nodes.write().await;
            if let Some(node) = nodes_write.get_mut(node_id) {
                node.status = NodeStatus::Offline;
                info!("Marked node {} as offline", node_id);
            }

            healthy_nodes = nodes_write
                .values()
                .filter(|n| n.status.is_healthy())
                .map(|n| n.node_id.clone())
                .collect();

            nodes_write.remove(node_id);
            info!("Removed failed node: {}", node_id);
        }

        {
            let leader = self.leader_id.read().await;
            is_leader_failure = leader.as_ref().map(|l| l == node_id).unwrap_or(false);
        }

        if is_leader_failure {
            info!("Leader node {} failed, starting re-election", node_id);
            *self.leader_id.write().await = None;
            self.elect_leader().await?;
        }

        if !healthy_nodes.is_empty() {
            info!("Triggering failover with healthy nodes: {:?}", healthy_nodes);
            self.trigger_failover(node_id, &healthy_nodes).await?;
        }

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

        let needs_relection = {
            let leader = self.leader_id.read().await;
            leader.as_ref().map(|l| l == failed_node_id).unwrap_or(false)
        };

        if needs_relection {
            info!("Leader node failed, starting re-election");
            *self.leader_id.write().await = None;
            self.elect_leader().await?;
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
        perform_discovery(client, addr, nodes).await;
    } else {
        debug!("Address {} is not a socket address, trying DNS resolution", addr);
        
        if let Ok(socket_addrs) = tokio::net::lookup_host(addr).await {
            for socket_addr in socket_addrs {
                let client = crate::rpc::RpcClient::new(socket_addr);
                perform_discovery(client, &socket_addr.to_string(), nodes).await;
            }
        } else {
            warn!("Failed to resolve address: {}", addr);
        }
    }

    Ok(())
}

async fn perform_discovery(client: crate::rpc::RpcClient, addr: &str, nodes: &Arc<RwLock<HashMap<String, NodeInfo>>>) {
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cluster_config_default() {
        let config = ClusterConfig::default();
        assert_eq!(config.cluster_name, "chronodb-cluster");
        assert_eq!(config.heartbeat_interval_ms, 5000);
        assert_eq!(config.node_timeout_ms, 15000);
        assert!(config.discovery_addresses.is_empty());
        assert!(!config.enable_auto_discovery);
    }

    #[test]
    fn test_node_status_equality() {
        assert_eq!(NodeStatus::Online, NodeStatus::Online);
        assert_eq!(NodeStatus::Offline, NodeStatus::Offline);
        assert_eq!(NodeStatus::Degraded, NodeStatus::Degraded);
        assert_ne!(NodeStatus::Online, NodeStatus::Offline);
    }

    #[test]
    fn test_node_info_creation() {
        let node = NodeInfo {
            node_id: "node-1".to_string(),
            address: "127.0.0.1:9090".to_string(),
            status: NodeStatus::Online,
            last_heartbeat: 1000,
            shard_count: 5,
            series_count: 100,
            is_leader: true,
            version: "1.0.0".to_string(),
        };
        assert_eq!(node.node_id, "node-1");
        assert_eq!(node.status, NodeStatus::Online);
        assert!(node.is_leader);
    }

    #[tokio::test]
    async fn test_cluster_manager_new() {
        let config = ClusterConfig::default();
        let manager = ClusterManager::new(config);
        let nodes = manager.get_nodes().await.unwrap();
        assert!(nodes.is_empty());
    }

    #[tokio::test]
    async fn test_register_node() {
        let config = ClusterConfig::default();
        let manager = ClusterManager::new(config);

        let node = NodeInfo {
            node_id: "node-1".to_string(),
            address: "127.0.0.1:9090".to_string(),
            status: NodeStatus::Online,
            last_heartbeat: chrono::Utc::now().timestamp_millis(),
            shard_count: 0,
            series_count: 0,
            is_leader: false,
            version: "1.0.0".to_string(),
        };

        manager.register_node(node).await.unwrap();

        let nodes = manager.get_nodes().await.unwrap();
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].node_id, "node-1");
    }

    #[tokio::test]
    async fn test_leader_election() {
        let config = ClusterConfig::default();
        let manager = ClusterManager::new(config);

        let node1 = NodeInfo {
            node_id: "node-1".to_string(),
            address: "127.0.0.1:9090".to_string(),
            status: NodeStatus::Online,
            last_heartbeat: chrono::Utc::now().timestamp_millis(),
            shard_count: 2,
            series_count: 0,
            is_leader: false,
            version: "1.0.0".to_string(),
        };

        let node2 = NodeInfo {
            node_id: "node-2".to_string(),
            address: "127.0.0.1:9091".to_string(),
            status: NodeStatus::Online,
            last_heartbeat: chrono::Utc::now().timestamp_millis(),
            shard_count: 5,
            series_count: 0,
            is_leader: false,
            version: "1.0.0".to_string(),
        };

        manager.register_node(node1).await.unwrap();
        manager.register_node(node2).await.unwrap();

        let leader = manager.get_leader().await.unwrap();
        assert!(leader.is_some());
        let leader = leader.unwrap();
        assert!(leader.is_leader);
    }

    #[tokio::test]
    async fn test_remove_node() {
        let config = ClusterConfig::default();
        let manager = ClusterManager::new(config);

        let node = NodeInfo {
            node_id: "node-1".to_string(),
            address: "127.0.0.1:9090".to_string(),
            status: NodeStatus::Online,
            last_heartbeat: chrono::Utc::now().timestamp_millis(),
            shard_count: 0,
            series_count: 0,
            is_leader: false,
            version: "1.0.0".to_string(),
        };

        manager.register_node(node).await.unwrap();
        assert_eq!(manager.get_nodes().await.unwrap().len(), 1);

        manager.remove_node("node-1").await.unwrap();
        assert_eq!(manager.get_nodes().await.unwrap().len(), 0);
    }

    #[tokio::test]
    async fn test_update_heartbeat() {
        let config = ClusterConfig::default();
        let manager = ClusterManager::new(config);

        let node = NodeInfo {
            node_id: "node-1".to_string(),
            address: "127.0.0.1:9090".to_string(),
            status: NodeStatus::Offline,
            last_heartbeat: 0,
            shard_count: 0,
            series_count: 0,
            is_leader: false,
            version: "1.0.0".to_string(),
        };

        manager.register_node(node).await.unwrap();

        manager.update_heartbeat("node-1").await.unwrap();

        let nodes = manager.get_nodes().await.unwrap();
        assert_eq!(nodes[0].status, NodeStatus::Online);
        assert!(nodes[0].last_heartbeat > 0);
    }

    #[tokio::test]
    async fn test_get_healthy_nodes() {
        let config = ClusterConfig::default();
        let manager = ClusterManager::new(config);

        let node = NodeInfo {
            node_id: "node-1".to_string(),
            address: "127.0.0.1:9090".to_string(),
            status: NodeStatus::Online,
            last_heartbeat: chrono::Utc::now().timestamp_millis(),
            shard_count: 0,
            series_count: 0,
            is_leader: false,
            version: "1.0.0".to_string(),
        };

        manager.register_node(node).await.unwrap();

        let healthy = manager.get_healthy_nodes().await.unwrap();
        assert_eq!(healthy.len(), 1);
        assert_eq!(healthy[0], "node-1");
    }

    #[tokio::test]
    async fn test_stop_without_start() {
        let config = ClusterConfig::default();
        let manager = ClusterManager::new(config);
        let result = manager.stop().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_set_current_node() {
        let config = ClusterConfig::default();
        let manager = ClusterManager::new(config);
        manager.set_current_node("node-0".to_string(), "127.0.0.1:9090".to_string()).await;
    }
}
