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
use crate::model::{Sample, TimeSeries, TimeSeriesId};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, error, warn, debug};

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
pub struct DistributedStorage {
    config: DistributedConfig,
    shard_manager: Arc<ShardManager>,
    coordinator: Option<Arc<Coordinator>>,
    replication_manager: Arc<ReplicationManager>,
    cluster_manager: Arc<ClusterManager>,
    rpc_manager: Arc<super::rpc::ClusterRpcManager>,
}

impl DistributedStorage {
    pub fn new(config: DistributedConfig) -> Result<Self> {
        let shard_manager = Arc::new(ShardManager::new(config.shard_config.clone()));
        let replication_manager = Arc::new(ReplicationManager::new(config.replication_config.clone()));
        let cluster_manager = Arc::new(ClusterManager::new(config.cluster_config.clone()));
        let rpc_manager = Arc::new(super::rpc::ClusterRpcManager::new());
        
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
        })
    }
    
    /// 启动分布式存储
    pub async fn start(&self) -> Result<()> {
        info!("Starting distributed storage node: {}", self.config.node_id);
        
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
        
        info!("Distributed storage node started successfully");
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
        // 路由查询到各个分片
        let shard_queries = self.route_queries(series_ids);
        
        let mut results = Vec::new();
        
        for (shard_id, shard_series_ids) in shard_queries {
            let shard_results = self.query_shard(shard_id, &shard_series_ids, start, end).await?;
            results.extend(shard_results);
        }
        
        Ok(results)
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
        
        for follower in followers {
            if follower != self.config.node_id {
                debug!("Replicating to follower: {}", follower);
                // 异步复制
                let series_clone = series.clone();
                let follower_clone = follower.clone();
                tokio::spawn(async move {
                    if let Err(e) = Self::replicate_to_node(follower_clone, series_clone).await {
                        error!("Replication failed: {}", e);
                    }
                });
            }
        }
        
        Ok(())
    }
    
    /// 复制到指定节点
    async fn replicate_to_node(node_id: String, series: TimeSeries) -> Result<()> {
        // 这里应该实现网络复制逻辑
        debug!("Replicating series {} to node {}", series.id, node_id);
        Ok(())
    }
    
    /// 转发写入到主节点
    async fn forward_write(&self, node_id: String, series: TimeSeries) -> Result<()> {
        // 这里应该实现网络转发逻辑
        debug!("Forwarding write to node: {}", node_id);
        Ok(())
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
    async fn query_local(&self, series_ids: &[TimeSeriesId], start: i64, end: i64) -> Result<Vec<TimeSeries>> {
        // 这里应该调用本地存储引擎查询
        debug!("Querying local storage for {} series", series_ids.len());
        Ok(vec![])
    }
    
    /// 远程查询
    async fn query_remote(&self, node_id: String, series_ids: &[TimeSeriesId], start: i64, end: i64) -> Result<Vec<TimeSeries>> {
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
