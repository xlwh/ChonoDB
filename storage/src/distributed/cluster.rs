use crate::error::Result;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::time::interval;
use tracing::{info, debug, warn};

/// 集群配置
#[derive(Debug, Clone)]
pub struct ClusterConfig {
    /// 集群名称
    pub cluster_name: String,
    /// 节点心跳间隔（毫秒）
    pub heartbeat_interval_ms: u64,
    /// 节点超时时间（毫秒）
    pub node_timeout_ms: u64,
    /// 节点发现地址
    pub discovery_addresses: Vec<String>,
    /// 是否启用自动节点发现
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

/// 节点信息
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

/// 集群管理器
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
        }
    }

    /// 设置RPC管理器
    pub fn with_rpc_manager(mut self, rpc_manager: Arc<crate::rpc::ClusterRpcManager>) -> Self {
        self.rpc_manager = Some(rpc_manager);
        self
    }

    /// 设置当前节点信息
    pub async fn set_current_node(&self, node_id: String, address: String) {
        let mut node_id_write = self.current_node_id.write().await;
        *node_id_write = Some(node_id);
        
        let mut address_write = self.current_node_address.write().await;
        *address_write = Some(address);
    }

    pub async fn start(&self) -> Result<()> {
        info!("Starting cluster manager for cluster: {}", self.config.cluster_name);
        
        // 启动节点发现任务
        if self.config.enable_auto_discovery {
            self.start_discovery_task().await?;
        }
        
        // 启动心跳检测任务
        self.start_heartbeat_task().await?;
        
        // 启动心跳发送任务
        if self.rpc_manager.is_some() {
            self.start_heartbeat_sender_task().await?;
        }
        
        Ok(())
    }

    /// 启动节点发现任务
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

    /// 启动心跳检测任务
    async fn start_heartbeat_task(&self) -> Result<()> {
        let config = self.config.clone();
        let nodes = self.nodes.clone();
        let leader_id = self.leader_id.clone();
        
        let handle = tokio::spawn(async move {
            let mut interval = interval(Duration::from_millis(config.heartbeat_interval_ms));
            
            loop {
                interval.tick().await;
                
                let now = chrono::Utc::now().timestamp_millis();
                let mut nodes_write = nodes.write().await;
                
                // 检查节点心跳
                let mut to_remove: Vec<String> = Vec::new();
                let mut to_mark_offline: Vec<String> = Vec::new();
                let mut leader_timed_out = false;
                
                for (node_id, node) in &*nodes_write {
                    if now - node.last_heartbeat > config.node_timeout_ms as i64 {
                        warn!("Node {} timed out, marking as offline", node_id);
                        to_mark_offline.push(node_id.clone());
                        
                        // 如果超时节点是领导者，重新选举
                        if node.is_leader {
                            info!("Leader node {} timed out, starting re-election", node_id);
                            leader_timed_out = true;
                            // 这里应该触发领导者选举
                        }
                        
                        // 添加到移除列表
                        to_remove.push(node_id.clone());
                    }
                }
                
                // 标记节点为离线
                for node_id in &to_mark_offline {
                    nodes_write.get_mut(node_id).unwrap().status = NodeStatus::Offline;
                }
                
                // 处理领导者超时
                if leader_timed_out {
                    *leader_id.write().await = None;
                }
                
                // 移除离线节点
                for node_id in to_remove {
                    nodes_write.remove(&node_id);
                    info!("Removed offline node: {}", node_id);
                    
                    // 触发故障转移（简化实现，不调用 self 方法）
                    let healthy_nodes: Vec<String> = nodes_write
                        .values()
                        .filter(|n| n.status == NodeStatus::Online)
                        .map(|n| n.node_id.clone())
                        .collect();
                    
                    if !healthy_nodes.is_empty() {
                        info!("Healthy nodes after failover: {:?}", healthy_nodes);
                    }
                }
            }
        });
        
        let mut heartbeat_task_write = self.heartbeat_task.write().await;
        *heartbeat_task_write = Some(handle);
        Ok(())
    }

    /// 注册节点
    pub async fn register_node(&self, node_info: NodeInfo) -> Result<()> {
        let node_id = node_info.node_id.clone();
        let mut nodes = self.nodes.write().await;
        nodes.insert(node_id.clone(), node_info);
        info!("Node registered: {}", node_id);
        
        // 检查是否需要选举领导者
        self.check_leader_election().await?;
        
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
        
        // 检查是否需要重新选举领导者
        if let Some(leader) = self.leader_id.read().await.as_ref() {
            if leader == node_id {
                info!("Leader node {} removed, starting re-election", node_id);
                *self.leader_id.write().await = None;
                self.check_leader_election().await?;
            }
        }
        
        Ok(())
    }

    /// 检查领导者选举
    pub async fn check_leader_election(&self) -> Result<()> {
        let leader = self.leader_id.read().await.clone();
        if leader.is_none() {
            // 执行领导者选举
            self.elect_leader().await?;
        }
        Ok(())
    }

    /// 执行领导者选举
    async fn elect_leader(&self) -> Result<()> {
        let nodes = self.nodes.read().await;
        let healthy_nodes: Vec<&NodeInfo> = nodes
            .values()
            .filter(|n| n.status == NodeStatus::Online)
            .collect();
        
        if !healthy_nodes.is_empty() {
            // 简单的领导者选举：选择第一个健康节点
            let leader = healthy_nodes[0];
            *self.leader_id.write().await = Some(leader.node_id.clone());
            
            // 更新节点状态
            let mut nodes_write = self.nodes.write().await;
            for (node_id, node) in &mut *nodes_write {
                node.is_leader = *node_id == leader.node_id;
            }
            
            info!("Elected new leader: {}", leader.node_id);
        }
        
        Ok(())
    }
    
    /// 处理节点故障
    pub async fn handle_node_failure(&self, node_id: &str) -> Result<()> {
        info!("Handling node failure: {}", node_id);
        
        // 1. 标记节点为离线
        let mut nodes_write = self.nodes.write().await;
        if let Some(node) = nodes_write.get_mut(node_id) {
            node.status = NodeStatus::Offline;
            info!("Marked node {} as offline", node_id);
        }
        
        // 2. 如果故障节点是领导者，重新选举
        if let Some(leader) = self.leader_id.read().await.as_ref() {
            if leader == node_id {
                info!("Leader node {} failed, starting re-election", node_id);
                *self.leader_id.write().await = None;
                self.elect_leader().await?;
            }
        }
        
        // 3. 移除故障节点
        nodes_write.remove(node_id);
        info!("Removed failed node: {}", node_id);
        
        Ok(())
    }
    
    /// 触发故障转移
    pub async fn trigger_failover(&self, failed_node_id: &str, healthy_nodes: &[String]) -> Result<()> {
        info!("Triggering failover for node: {}, healthy nodes: {:?}", failed_node_id, healthy_nodes);
        
        // 这里可以添加更多的故障转移逻辑
        // 例如，通知分片管理器重新分配分片
        
        Ok(())
    }

    /// 获取领导者节点
    pub async fn get_leader(&self) -> Result<Option<NodeInfo>> {
        if let Some(leader_id) = self.leader_id.read().await.as_ref() {
            let nodes = self.nodes.read().await;
            if let Some(leader) = nodes.get(leader_id) {
                return Ok(Some(leader.clone()));
            }
        }
        Ok(None)
    }

    /// 启动心跳发送任务
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
                                Ok(response) => {
                                    warn!("Heartbeat failed for node {}: {}", peer_node_id, "unknown error");
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

    /// 停止集群管理器
    pub async fn stop(&self) -> Result<()> {
        // 停止发现任务
        let mut discovery_task_write = self.discovery_task.write().await;
        if let Some(task) = discovery_task_write.take() {
            task.abort();
        }
        
        // 停止心跳检测任务
        let mut heartbeat_task_write = self.heartbeat_task.write().await;
        if let Some(task) = heartbeat_task_write.take() {
            task.abort();
        }

        // 停止心跳发送任务
        let mut heartbeat_sender_task_write = self.heartbeat_sender_task.write().await;
        if let Some(task) = heartbeat_sender_task_write.take() {
            task.abort();
        }

        Ok(())
    }
}

/// 发现节点
async fn discover_nodes(addr: &str, nodes: &Arc<RwLock<HashMap<String, NodeInfo>>>) -> Result<()> {
    debug!("Discovering nodes from {}", addr);
    
    // 尝试解析地址为SocketAddr
    if let Ok(socket_addr) = addr.parse::<std::net::SocketAddr>() {
        // 尝试通过RPC获取节点信息
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
                        shard_count: 0, // 后续可以从集群状态中获取
                        series_count: 0, // 后续可以从集群状态中获取
                        is_leader: cluster_status.is_coordinator && cluster_status.node_id == node_id_clone,
                        version: "1.0.0".to_string(), // 后续可以从集群状态中获取
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
        // 如果不是SocketAddr，尝试使用其他发现机制（如DNS、etcd等）
        debug!("Address {} is not a socket address, trying other discovery methods", addr);
    }
    
    Ok(())
}
