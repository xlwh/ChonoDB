use crate::error::Result;
use crate::model::TimeSeriesId;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, debug};

/// 分片配置
#[derive(Debug, Clone)]
pub struct ShardConfig {
    /// 分片数量
    pub shard_count: u64,
    /// 副本因子
    pub replication_factor: u32,
    /// 是否启用虚拟节点
    pub enable_virtual_nodes: bool,
    /// 每个物理节点的虚拟节点数
    pub virtual_nodes_per_physical: u32,
}

impl Default for ShardConfig {
    fn default() -> Self {
        Self {
            shard_count: 64,
            replication_factor: 3,
            enable_virtual_nodes: true,
            virtual_nodes_per_physical: 128,
        }
    }
}

/// 分片
#[derive(Debug, Clone)]
pub struct Shard {
    pub id: u64,
    pub primary_node: String,
    pub follower_nodes: Vec<String>,
    pub status: ShardStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShardStatus {
    Healthy,
    Degraded,
    Unavailable,
}

/// 分片放置策略
#[derive(Debug, Clone)]
pub struct ShardPlacement {
    pub shard_id: u64,
    pub node_id: String,
    pub is_primary: bool,
}

/// 分片管理器
pub struct ShardManager {
    config: ShardConfig,
    shards: Arc<RwLock<HashMap<u64, Shard>>>,
    node_shards: Arc<RwLock<HashMap<String, Vec<u64>>>>,
    virtual_nodes: Arc<RwLock<Vec<(u64, String)>>>, // (hash, node_id)
}

impl ShardManager {
    pub fn new(config: ShardConfig) -> Self {
        Self {
            config,
            shards: Arc::new(RwLock::new(HashMap::new())),
            node_shards: Arc::new(RwLock::new(HashMap::new())),
            virtual_nodes: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// 启动分片管理器
    pub async fn start(&self) -> Result<()> {
        info!("Starting shard manager with {} shards", self.config.shard_count);
        
        // 初始化分片
        let mut shards = self.shards.write().await;
        for i in 0..self.config.shard_count {
            shards.insert(i, Shard {
                id: i,
                primary_node: String::new(),
                follower_nodes: Vec::new(),
                status: ShardStatus::Unavailable,
            });
        }
        
        info!("Shard manager started");
        Ok(())
    }

    /// 根据系列ID获取分片ID
    pub fn get_shard_for_series(&self, series_id: TimeSeriesId) -> u64 {
        // 使用一致性哈希
        let hash = self.hash_series_id(series_id);
        hash % self.config.shard_count
    }

    /// 计算系列ID的哈希值
    fn hash_series_id(&self, series_id: TimeSeriesId) -> u64 {
        // 使用简单的哈希函数，实际应该使用更复杂的哈希算法
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        
        let mut hasher = DefaultHasher::new();
        series_id.hash(&mut hasher);
        hasher.finish()
    }

    /// 获取分片的主节点
    pub async fn get_primary_node(&self, shard_id: u64) -> Result<String> {
        let shards = self.shards.read().await;
        
        if let Some(shard) = shards.get(&shard_id) {
            if !shard.primary_node.is_empty() {
                Ok(shard.primary_node.clone())
            } else {
                Err(crate::error::Error::Internal(format!("No primary node for shard {}", shard_id)))
            }
        } else {
            Err(crate::error::Error::Internal(format!("Shard {} not found", shard_id)))
        }
    }

    /// 获取分片的副本节点
    pub async fn get_follower_nodes(&self, shard_id: u64) -> Result<Vec<String>> {
        let shards = self.shards.read().await;
        
        if let Some(shard) = shards.get(&shard_id) {
            Ok(shard.follower_nodes.clone())
        } else {
            Err(crate::error::Error::Internal(format!("Shard {} not found", shard_id)))
        }
    }

    /// 获取分片的所有节点
    pub async fn get_shard_nodes(&self, shard_id: u64) -> Result<Vec<String>> {
        let shards = self.shards.read().await;
        
        if let Some(shard) = shards.get(&shard_id) {
            let mut nodes = vec![shard.primary_node.clone()];
            nodes.extend(shard.follower_nodes.clone());
            Ok(nodes)
        } else {
            Err(crate::error::Error::Internal(format!("Shard {} not found", shard_id)))
        }
    }

    /// 分配分片到节点
    pub async fn assign_shard(&self, shard_id: u64, primary_node: String, follower_nodes: Vec<String>) -> Result<()> {
        let mut shards = self.shards.write().await;
        
        if let Some(shard) = shards.get_mut(&shard_id) {
            shard.primary_node = primary_node.clone();
            shard.follower_nodes = follower_nodes.clone();
            shard.status = ShardStatus::Healthy;
            
            debug!("Assigned shard {} to primary: {}, followers: {:?}", 
                shard_id, primary_node, follower_nodes);
        }
        
        // 更新节点-分片映射
        let mut node_shards = self.node_shards.write().await;
        node_shards.entry(primary_node).or_insert_with(Vec::new).push(shard_id);
        for follower in follower_nodes {
            node_shards.entry(follower).or_insert_with(Vec::new).push(shard_id);
        }
        
        Ok(())
    }

    /// 重新平衡分片
    pub async fn rebalance_shards(&self, nodes: &[String]) -> Result<()> {
        info!("Rebalancing shards across {} nodes", nodes.len());
        
        if nodes.is_empty() {
            return Err(crate::error::Error::Internal("No nodes available for rebalancing".to_string()));
        }
        
        let mut shards = self.shards.write().await;
        let replication_factor = self.config.replication_factor as usize;
        
        for shard_id in 0..self.config.shard_count {
            // 计算主节点
            let primary_index = (shard_id as usize) % nodes.len();
            let primary_node = nodes[primary_index].clone();
            
            // 计算副本节点
            let mut follower_nodes = Vec::new();
            for i in 1..replication_factor.min(nodes.len()) {
                let follower_index = (primary_index + i) % nodes.len();
                follower_nodes.push(nodes[follower_index].clone());
            }
            
            if let Some(shard) = shards.get_mut(&shard_id) {
                shard.primary_node = primary_node;
                shard.follower_nodes = follower_nodes;
                shard.status = ShardStatus::Healthy;
            }
        }
        
        info!("Shard rebalancing completed");
        Ok(())
    }

    /// 获取分片分布
    pub async fn get_shard_distribution(&self) -> Result<HashMap<u64, Vec<String>>> {
        let shards = self.shards.read().await;
        let mut distribution = HashMap::new();
        
        for (shard_id, shard) in shards.iter() {
            let mut nodes = vec![shard.primary_node.clone()];
            nodes.extend(shard.follower_nodes.clone());
            distribution.insert(*shard_id, nodes);
        }
        
        Ok(distribution)
    }

    /// 获取节点负责的分片
    pub async fn get_node_shards(&self, node_id: &str) -> Result<Vec<u64>> {
        let node_shards = self.node_shards.read().await;
        
        Ok(node_shards.get(node_id).cloned().unwrap_or_default())
    }
    
    /// 处理节点故障
    pub async fn handle_node_failure(&self, failed_node_id: &str, healthy_nodes: &[String]) -> Result<()> {
        info!("Handling node failure in shard manager: {}, healthy nodes: {:?}", failed_node_id, healthy_nodes);
        
        if healthy_nodes.is_empty() {
            return Err(crate::error::Error::Internal("No healthy nodes available for failover".to_string()));
        }
        
        // 1. 获取故障节点负责的分片
        let failed_shards = self.get_node_shards(failed_node_id).await?;
        info!("Failed node {} was responsible for {} shards", failed_node_id, failed_shards.len());
        
        // 2. 重新分配这些分片
        for shard_id in failed_shards {
            let mut shards_write = self.shards.write().await;
            if let Some(shard) = shards_write.get_mut(&shard_id) {
                // 检查故障节点是否是主节点
                if shard.primary_node == failed_node_id {
                    // 从健康节点中选择新的主节点
                    let new_primary = healthy_nodes[0].clone();
                    
                    // 重新计算副本节点
                    let replication_factor = self.config.replication_factor as usize;
                    let mut new_followers = Vec::new();
                    for i in 1..replication_factor.min(healthy_nodes.len()) {
                        new_followers.push(healthy_nodes[i].clone());
                    }
                    
                    // 更新分片信息
                    shard.primary_node = new_primary.clone();
                    shard.follower_nodes = new_followers.clone();
                    shard.status = ShardStatus::Healthy;
                    
                    info!("Reassigned shard {}: new primary {}, new followers: {:?}", 
                          shard_id, new_primary, new_followers);
                } else {
                    // 故障节点是副本节点，从副本列表中移除
                    shard.follower_nodes.retain(|node| node != failed_node_id);
                    
                    // 如果需要，添加新的副本节点
                    if shard.follower_nodes.len() < (self.config.replication_factor as usize - 1) {
                        for node in healthy_nodes {
                            if node != &shard.primary_node && !shard.follower_nodes.contains(node) {
                                shard.follower_nodes.push(node.clone());
                                if shard.follower_nodes.len() >= (self.config.replication_factor as usize - 1) {
                                    break;
                                }
                            }
                        }
                    }
                    
                    info!("Updated shard {} followers: {:?}", shard_id, shard.follower_nodes);
                }
            }
        }
        
        // 3. 更新节点-分片映射
        let mut node_shards_write = self.node_shards.write().await;
        node_shards_write.remove(failed_node_id);
        
        // 重新计算所有节点的分片映射
        node_shards_write.clear();
        let shards_read = self.shards.read().await;
        for (shard_id, shard) in &*shards_read {
            node_shards_write.entry(shard.primary_node.clone()).or_insert_with(Vec::new).push(*shard_id);
            for follower in &shard.follower_nodes {
                node_shards_write.entry(follower.clone()).or_insert_with(Vec::new).push(*shard_id);
            }
        }
        
        info!("Completed failover for node: {}", failed_node_id);
        Ok(())
    }

    /// 添加虚拟节点
    pub async fn add_virtual_nodes(&self, node_id: String) -> Result<()> {
        if !self.config.enable_virtual_nodes {
            return Ok(());
        }
        
        let mut virtual_nodes = self.virtual_nodes.write().await;
        
        for i in 0..self.config.virtual_nodes_per_physical {
            let hash = self.hash_virtual_node(&node_id, i);
            virtual_nodes.push((hash, node_id.clone()));
        }
        
        // 按哈希值排序
        virtual_nodes.sort_by_key(|(hash, _)| *hash);
        
        debug!("Added {} virtual nodes for {}", self.config.virtual_nodes_per_physical, node_id);
        Ok(())
    }

    /// 计算虚拟节点的哈希值
    fn hash_virtual_node(&self, node_id: &str, index: u32) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        
        let mut hasher = DefaultHasher::new();
        node_id.hash(&mut hasher);
        index.hash(&mut hasher);
        hasher.finish()
    }

    /// 使用一致性哈希获取节点
    pub async fn get_node_by_hash(&self, hash: u64) -> Option<String> {
        let virtual_nodes = self.virtual_nodes.read().await;
        
        if virtual_nodes.is_empty() {
            return None;
        }
        
        // 二分查找第一个大于等于hash的虚拟节点
        let idx = match virtual_nodes.binary_search_by_key(&hash, |(h, _)| *h) {
            Ok(i) => i,
            Err(i) => i % virtual_nodes.len(),
        };
        
        Some(virtual_nodes[idx].1.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shard_config() {
        let config = ShardConfig::default();
        assert_eq!(config.shard_count, 64);
        assert_eq!(config.replication_factor, 3);
    }

    #[tokio::test]
    async fn test_shard_manager() {
        let config = ShardConfig::default();
        let manager = ShardManager::new(config);
        
        manager.start().await.unwrap();
        
        // 测试分片分配
        manager.assign_shard(0, "node1".to_string(), vec!["node2".to_string(), "node3".to_string()]).await.unwrap();
        
        let primary = manager.get_primary_node(0).await.unwrap();
        assert_eq!(primary, "node1");
        
        let followers = manager.get_follower_nodes(0).await.unwrap();
        assert_eq!(followers.len(), 2);
    }

    #[tokio::test]
    async fn test_rebalance_shards() {
        let config = ShardConfig::default();
        let manager = ShardManager::new(config);
        
        manager.start().await.unwrap();
        
        let nodes = vec!["node1".to_string(), "node2".to_string(), "node3".to_string()];
        manager.rebalance_shards(&nodes).await.unwrap();
        
        let distribution = manager.get_shard_distribution().await.unwrap();
        assert!(!distribution.is_empty());
    }

    #[test]
    fn test_get_shard_for_series() {
        let config = ShardConfig::default();
        let manager = ShardManager::new(config);
        
        let shard_id1 = manager.get_shard_for_series(1);
        let shard_id2 = manager.get_shard_for_series(2);
        
        assert!(shard_id1 < 64);
        assert!(shard_id2 < 64);
    }
}
