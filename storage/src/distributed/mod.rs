pub mod shard;
pub mod coordinator;
pub mod replication;
pub mod cluster;
pub mod query_coordinator;

pub use shard::{ShardManager, Shard, ShardConfig, ShardPlacement};
pub use coordinator::{Coordinator, CoordinatorConfig, QueryRouter};
pub use replication::{ReplicationManager, ReplicationConfig, ReplicaPlacement};
pub use cluster::{ClusterManager, ClusterConfig, NodeInfo, NodeStatus};
pub use query_coordinator::{QueryCoordinator, CoordinatorConfig as QueryCoordinatorConfig, ShardManager as QueryShardManager, AggregationType};

use crate::error::Result;
use crate::model::{TimeSeries, TimeSeriesId};
use crate::query::planner::{PlanType, VectorQueryPlan};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{info, debug, warn};

/// 分布式配置
#[derive(Debug, Clone)]
pub struct DistributedConfig {
    /// 节点ID
    pub node_id: String,
    /// 集群名称
    pub cluster_name: String,
    /// 节点地址
    pub node_address: String,
    /// 协调器地址
    pub coordinator_address: String,
    /// 是否作为协调器
    pub is_coordinator: bool,
    /// 分片配置
    pub shard_config: ShardConfig,
    /// 副本配置
    pub replication_config: ReplicationConfig,
    /// 集群配置
    pub cluster_config: ClusterConfig,
}

impl Default for DistributedConfig {
    fn default() -> Self {
        Self {
            node_id: uuid::Uuid::new_v4().to_string(),
            cluster_name: "chronodb-cluster".to_string(),
            node_address: "127.0.0.1:9090".to_string(),
            coordinator_address: "127.0.0.1:9091".to_string(),
            is_coordinator: false,
            shard_config: ShardConfig::default(),
            replication_config: ReplicationConfig::default(),
            cluster_config: ClusterConfig::default(),
        }
    }
}

/// 分布式存储引擎
#[derive(Clone)]
pub struct DistributedStorage {
    config: DistributedConfig,
    shard_manager: Arc<ShardManager>,
    coordinator: Option<Arc<Coordinator>>,
    replication_manager: Arc<ReplicationManager>,
    cluster_manager: Arc<ClusterManager>,
    rpc_manager: Arc<super::rpc::ClusterRpcManager>,
    query_coordinator: Arc<QueryCoordinator>,
}

impl DistributedStorage {
    pub fn new(config: DistributedConfig) -> Result<Self> {
        let shard_manager = Arc::new(ShardManager::new(config.shard_config.clone()));
        let replication_manager = Arc::new(ReplicationManager::new(config.replication_config.clone()));
        let cluster_manager = Arc::new(ClusterManager::new(config.cluster_config.clone()));
        let rpc_manager = Arc::new(super::rpc::ClusterRpcManager::new());
        let query_coordinator = Arc::new(QueryCoordinator::new(
            rpc_manager.clone(),
            Arc::new(tokio::sync::RwLock::new(QueryShardManager::new(128))),
            QueryCoordinatorConfig::default()
        ));
        
        let coordinator = if config.is_coordinator {
            Some(Arc::new(Coordinator::new(CoordinatorConfig::default())))
        } else {
            None
        };
        
        Ok(Self {
            config,
            shard_manager,
            coordinator,
            replication_manager,
            cluster_manager,
            rpc_manager,
            query_coordinator,
        })
    }
    
    /// 启动分布式存储
    pub async fn start(&self) -> Result<()> {
        info!("Starting distributed storage node: {}", self.config.node_id);
        
        // 注册当前节点到集群
        let current_node = crate::distributed::NodeInfo {
            node_id: self.config.node_id.clone(),
            address: self.config.node_address.clone(),
            status: crate::distributed::NodeStatus::Online,
            last_heartbeat: chrono::Utc::now().timestamp_millis(),
            shard_count: 0,
            series_count: 0,
            is_leader: self.config.is_coordinator,
            version: env!("CARGO_PKG_VERSION").to_string(),
        };
        self.cluster_manager.register_node(current_node).await?;
        
        // 启动集群管理
        self.cluster_manager.start().await?;
        
        // 启动分片管理
        self.shard_manager.start().await?;
        
        // 启动副本管理
        self.replication_manager.start(self.rpc_manager.clone()).await?;
        
        // 如果是协调器，启动协调服务
        if let Some(coordinator) = &self.coordinator {
            coordinator.start().await?;
        }
        
        // 注册当前节点的 RPC 客户端
        if let Ok(addr) = self.config.node_address.parse::<std::net::SocketAddr>() {
            self.rpc_manager.register_node(self.config.node_id.clone(), addr).await;
        } else {
            warn!("Invalid node address: {}", self.config.node_address);
        }
        
        // 启动故障检测和转移任务
        self.start_failover_monitor().await?;
        
        info!("Distributed storage node started successfully");
        Ok(())
    }
    
    /// 启动故障检测和转移监控
    async fn start_failover_monitor(&self) -> Result<()> {
        let cluster_manager = self.cluster_manager.clone();
        let shard_manager = self.shard_manager.clone();
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(10));
            
            loop {
                interval.tick().await;
                
                // 检查集群状态
                if let Ok(nodes) = cluster_manager.get_nodes().await {
                    let healthy_nodes: Vec<String> = nodes
                        .iter()
                        .filter(|n| n.status == crate::distributed::NodeStatus::Online)
                        .map(|n| n.node_id.clone())
                        .collect();
                    
                    // 如果有健康节点，确保分片平衡
                    if !healthy_nodes.is_empty() {
                        if let Err(e) = shard_manager.rebalance_shards(&healthy_nodes).await {
                            warn!("Failed to rebalance shards: {}", e);
                        }
                    }
                }
            }
        });
        
        Ok(())
    }
    
    /// 写入数据
    pub async fn write(&self, series: TimeSeries) -> Result<()> {
        // 计算分片
        let shard_id = self.shard_manager.get_shard_for_series(series.id);
        
        // 获取分片的主节点
        let primary_node = self.shard_manager.get_primary_node(shard_id).await?;
        
        if primary_node == self.config.node_id {
            // 本节点是主节点，直接写入
            debug!("Writing to local shard: {}", shard_id);
            self.write_to_local(series.clone()).await?;
            
            // 异步复制到副本
            self.replicate_to_followers(shard_id, series).await?;
        } else {
            // 转发到主节点
            debug!("Forwarding write to primary node: {}", primary_node);
            self.forward_write(primary_node, series).await?;
        }
        
        Ok(())
    }
    
    /// 查询数据
    pub async fn query(&self, series_ids: &[TimeSeriesId], start: i64, end: i64) -> Result<Vec<TimeSeries>> {
        // 创建查询计划
        let plan = crate::query::QueryPlan {
            plan_type: PlanType::VectorQuery(VectorQueryPlan {
                name: None,
                matchers: Vec::new(),
            }),
            start,
            end,
            step: 0,
        };
        
        // 使用查询协调器执行分布式查询
        let result = self.query_coordinator.execute_query(&plan).await?;
        Ok(result.series)
    }
    
    /// 写入本地存储
    async fn write_to_local(&self, series: TimeSeries) -> Result<()> {
        // 这里应该调用本地存储引擎
        // 简化实现
        debug!("Writing series {} to local storage", series.id);
        Ok(())
    }
    
    /// 复制到副本
    async fn replicate_to_followers(&self, shard_id: u64, series: TimeSeries) -> Result<()> {
        let followers = self.shard_manager.get_follower_nodes(shard_id).await?;
        
        if !followers.is_empty() {
            // 使用复制管理器执行复制操作
            self.replication_manager.replicate(shard_id, series, &followers).await?;
        }
        
        Ok(())
    }
    
    /// 复制到指定节点
    async fn replicate_to_node(&self, node_id: String, series: TimeSeries) -> Result<()> {
        // 使用 RPC 客户端进行网络复制
        if let Some(client) = self.rpc_manager.get_client(&node_id).await {
            let response = client.replicate(0, series).await?;
            if response.success {
                debug!("Successfully replicated series to node {}", node_id);
                Ok(())
            } else {
                Err(crate::error::Error::Internal(format!("Replication failed: {}", response.message)))
            }
        } else {
            Err(crate::error::Error::Internal(format!("Node {} not found", node_id)))
        }
    }
    
    /// 转发写入到主节点
    async fn forward_write(&self, node_id: String, series: TimeSeries) -> Result<()> {
        // 使用 RPC 客户端进行网络转发
        if let Some(client) = self.rpc_manager.get_client(&node_id).await {
            let response = client.write(series).await?;
            if response.success {
                debug!("Successfully forwarded write to node {}", node_id);
                Ok(())
            } else {
                Err(crate::error::Error::Internal(format!("Forward write failed: {}", response.message)))
            }
        } else {
            Err(crate::error::Error::Internal(format!("Node {} not found", node_id)))
        }
    }
    
    /// 路由查询到分片
    fn route_queries(&self, series_ids: &[TimeSeriesId]) -> HashMap<u64, Vec<TimeSeriesId>> {
        let mut shard_queries: HashMap<u64, Vec<TimeSeriesId>> = HashMap::new();
        
        for &series_id in series_ids {
            let shard_id = self.shard_manager.get_shard_for_series(series_id);
            shard_queries.entry(shard_id).or_insert_with(Vec::new).push(series_id);
        }
        
        shard_queries
    }
    
    /// 查询分片
    async fn query_shard(&self, shard_id: u64, series_ids: &[TimeSeriesId], start: i64, end: i64) -> Result<Vec<TimeSeries>> {
        // 获取分片的节点
        let nodes = self.shard_manager.get_shard_nodes(shard_id).await?;
        
        // 优先查询主节点
        if let Some(primary) = nodes.first() {
            if *primary == self.config.node_id {
                // 本地查询
                self.query_local(series_ids, start, end).await
            } else {
                // 远程查询
                self.query_remote(primary.clone(), series_ids, start, end).await
            }
        } else {
            Err(crate::error::Error::Internal(format!("No nodes for shard {}", shard_id)))
        }
    }
    
    /// 本地查询
    async fn query_local(&self, series_ids: &[TimeSeriesId], _start: i64, _end: i64) -> Result<Vec<TimeSeries>> {
        // 这里应该调用本地存储引擎查询
        debug!("Querying local storage for {} series", series_ids.len());
        Ok(vec![])
    }
    
    /// 远程查询
    async fn query_remote(&self, node_id: String, _series_ids: &[TimeSeriesId], _start: i64, _end: i64) -> Result<Vec<TimeSeries>> {
        // 这里应该实现网络查询逻辑
        debug!("Querying remote node: {}", node_id);
        Ok(vec![])
    }
    
    /// 获取集群状态
    pub async fn get_cluster_status(&self) -> Result<ClusterStatus> {
        let nodes = self.cluster_manager.get_nodes().await?;
        let shards = self.shard_manager.get_shard_distribution().await?;
        
        Ok(ClusterStatus {
            node_id: self.config.node_id.clone(),
            is_coordinator: self.config.is_coordinator,
            nodes,
            shards,
        })
    }
}

/// 集群状态
#[derive(Debug, Clone)]
pub struct ClusterStatus {
    pub node_id: String,
    pub is_coordinator: bool,
    pub nodes: Vec<NodeInfo>,
    pub shards: HashMap<u64, Vec<String>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_distributed_config() {
        let config = DistributedConfig::default();
        assert!(!config.node_id.is_empty());
        assert_eq!(config.cluster_name, "chronodb-cluster");
    }

    #[tokio::test]
    async fn test_distributed_storage() {
        let config = DistributedConfig {
            is_coordinator: true,
            ..Default::default()
        };
        
        let storage = DistributedStorage::new(config).unwrap();
        assert!(storage.coordinator.is_some());
    }
}
