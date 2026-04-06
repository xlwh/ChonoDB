use crate::error::{Error, Result};
use crate::model::{Labels, TimeSeriesId};
use etcd_client::{Client, ConnectOptions, GetOptions, PutOptions, WatchOptions, WatchStream};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{debug, info};

/// etcd元数据存储配置
#[derive(Debug, Clone)]
pub struct EtcdConfig {
    /// etcd端点列表
    pub endpoints: Vec<String>,
    /// 连接超时（秒）
    pub connect_timeout_secs: u64,
    /// 是否启用认证
    pub auth_enabled: bool,
    /// 用户名
    pub username: Option<String>,
    /// 密码
    pub password: Option<String>,
    /// 键前缀
    pub key_prefix: String,
}

impl Default for EtcdConfig {
    fn default() -> Self {
        Self {
            endpoints: vec!["localhost:2379".to_string()],
            connect_timeout_secs: 5,
            auth_enabled: false,
            username: None,
            password: None,
            key_prefix: "/chronodb".to_string(),
        }
    }
}

/// etcd元数据存储
pub struct EtcdMetadataStore {
    client: Arc<RwLock<Client>>,
    config: EtcdConfig,
}

/// 系列元数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeriesMetadata {
    pub series_id: TimeSeriesId,
    pub labels: Labels,
    pub created_at: i64,
    pub updated_at: i64,
    pub last_sample_time: i64,
    pub sample_count: u64,
}

/// 分片元数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShardMetadata {
    pub shard_id: u64,
    pub node_ids: Vec<String>,
    pub series_count: u64,
    pub created_at: i64,
}

/// 节点元数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeMetadata {
    pub node_id: String,
    pub address: String,
    pub status: NodeStatus,
    pub shards: Vec<u64>,
    pub last_heartbeat: i64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum NodeStatus {
    Online,
    Offline,
    Suspect,
}

impl EtcdMetadataStore {
    pub async fn new(config: EtcdConfig) -> Result<Self> {
        let connect_options = ConnectOptions::new()
            .with_timeout(Duration::from_secs(config.connect_timeout_secs));

        let client = Client::connect(&config.endpoints, Some(connect_options))
            .await
            .map_err(|e| Error::Internal(format!("Failed to connect to etcd: {}", e)))?;

        info!("Connected to etcd at {:?}", config.endpoints);

        Ok(Self {
            client: Arc::new(RwLock::new(client)),
            config,
        })
    }

    /// 存储系列元数据
    pub async fn put_series_metadata(&self, metadata: &SeriesMetadata) -> Result<()> {
        let key = format!("{}/series/{}", self.config.key_prefix, metadata.series_id);
        let value = serde_json::to_vec(metadata)
            .map_err(|e| Error::Internal(format!("Failed to serialize metadata: {}", e)))?;

        let mut client = self.client.write().await;
        client
            .put(key, value, None)
            .await
            .map_err(|e| Error::Internal(format!("Failed to put series metadata: {}", e)))?;

        debug!("Stored series metadata for series {}", metadata.series_id);
        Ok(())
    }

    /// 获取系列元数据
    pub async fn get_series_metadata(&self, series_id: TimeSeriesId) -> Result<Option<SeriesMetadata>> {
        let key = format!("{}/series/{}", self.config.key_prefix, series_id);

        let mut client = self.client.write().await;
        let response = client
            .get(key, None)
            .await
            .map_err(|e| Error::Internal(format!("Failed to get series metadata: {}", e)))?;

        if let Some(kv) = response.kvs().first() {
            let metadata: SeriesMetadata = serde_json::from_slice(kv.value())
                .map_err(|e| Error::Internal(format!("Failed to deserialize metadata: {}", e)))?;
            Ok(Some(metadata))
        } else {
            Ok(None)
        }
    }

    /// 删除系列元数据
    pub async fn delete_series_metadata(&self, series_id: TimeSeriesId) -> Result<()> {
        let key = format!("{}/series/{}", self.config.key_prefix, series_id);

        let mut client = self.client.write().await;
        client
            .delete(key, None)
            .await
            .map_err(|e| Error::Internal(format!("Failed to delete series metadata: {}", e)))?;

        debug!("Deleted series metadata for series {}", series_id);
        Ok(())
    }

    /// 列出所有系列
    pub async fn list_series(&self) -> Result<Vec<SeriesMetadata>> {
        let prefix = format!("{}/series/", self.config.key_prefix);

        let mut client = self.client.write().await;
        let response = client
            .get(prefix.clone(), Some(GetOptions::new().with_prefix()))
            .await
            .map_err(|e| Error::Internal(format!("Failed to list series: {}", e)))?;

        let mut series = Vec::new();
        for kv in response.kvs() {
            if let Ok(metadata) = serde_json::from_slice::<SeriesMetadata>(kv.value()) {
                series.push(metadata);
            }
        }

        Ok(series)
    }

    /// 存储分片元数据
    pub async fn put_shard_metadata(&self, metadata: &ShardMetadata) -> Result<()> {
        let key = format!("{}/shards/{}", self.config.key_prefix, metadata.shard_id);
        let value = serde_json::to_vec(metadata)
            .map_err(|e| Error::Internal(format!("Failed to serialize shard metadata: {}", e)))?;

        let mut client = self.client.write().await;
        client
            .put(key, value, None)
            .await
            .map_err(|e| Error::Internal(format!("Failed to put shard metadata: {}", e)))?;

        debug!("Stored shard metadata for shard {}", metadata.shard_id);
        Ok(())
    }

    /// 获取分片元数据
    pub async fn get_shard_metadata(&self, shard_id: u64) -> Result<Option<ShardMetadata>> {
        let key = format!("{}/shards/{}", self.config.key_prefix, shard_id);

        let mut client = self.client.write().await;
        let response = client
            .get(key, None)
            .await
            .map_err(|e| Error::Internal(format!("Failed to get shard metadata: {}", e)))?;

        if let Some(kv) = response.kvs().first() {
            let metadata: ShardMetadata = serde_json::from_slice(kv.value())
                .map_err(|e| Error::Internal(format!("Failed to deserialize shard metadata: {}", e)))?;
            Ok(Some(metadata))
        } else {
            Ok(None)
        }
    }

    /// 列出所有分片
    pub async fn list_shards(&self) -> Result<Vec<ShardMetadata>> {
        let prefix = format!("{}/shards/", self.config.key_prefix);

        let mut client = self.client.write().await;
        let response = client
            .get(prefix.clone(), Some(GetOptions::new().with_prefix()))
            .await
            .map_err(|e| Error::Internal(format!("Failed to list shards: {}", e)))?;

        let mut shards = Vec::new();
        for kv in response.kvs() {
            if let Ok(metadata) = serde_json::from_slice::<ShardMetadata>(kv.value()) {
                shards.push(metadata);
            }
        }

        Ok(shards)
    }

    /// 存储节点元数据
    pub async fn put_node_metadata(&self, metadata: &NodeMetadata) -> Result<()> {
        let key = format!("{}/nodes/{}", self.config.key_prefix, metadata.node_id);
        let value = serde_json::to_vec(metadata)
            .map_err(|e| Error::Internal(format!("Failed to serialize node metadata: {}", e)))?;

        let mut client = self.client.write().await;
        client
            .put(key, value, None)
            .await
            .map_err(|e| Error::Internal(format!("Failed to put node metadata: {}", e)))?;

        debug!("Stored node metadata for node {}", metadata.node_id);
        Ok(())
    }

    /// 获取节点元数据
    pub async fn get_node_metadata(&self, node_id: &str) -> Result<Option<NodeMetadata>> {
        let key = format!("{}/nodes/{}", self.config.key_prefix, node_id);

        let mut client = self.client.write().await;
        let response = client
            .get(key, None)
            .await
            .map_err(|e| Error::Internal(format!("Failed to get node metadata: {}", e)))?;

        if let Some(kv) = response.kvs().first() {
            let metadata: NodeMetadata = serde_json::from_slice(kv.value())
                .map_err(|e| Error::Internal(format!("Failed to deserialize node metadata: {}", e)))?;
            Ok(Some(metadata))
        } else {
            Ok(None)
        }
    }

    /// 列出所有节点
    pub async fn list_nodes(&self) -> Result<Vec<NodeMetadata>> {
        let prefix = format!("{}/nodes/", self.config.key_prefix);

        let mut client = self.client.write().await;
        let response = client
            .get(prefix.clone(), Some(GetOptions::new().with_prefix()))
            .await
            .map_err(|e| Error::Internal(format!("Failed to list nodes: {}", e)))?;

        let mut nodes = Vec::new();
        for kv in response.kvs() {
            if let Ok(metadata) = serde_json::from_slice::<NodeMetadata>(kv.value()) {
                nodes.push(metadata);
            }
        }

        Ok(nodes)
    }

    /// 删除节点元数据
    pub async fn delete_node_metadata(&self, node_id: &str) -> Result<()> {
        let key = format!("{}/nodes/{}", self.config.key_prefix, node_id);

        let mut client = self.client.write().await;
        client
            .delete(key, None)
            .await
            .map_err(|e| Error::Internal(format!("Failed to delete node metadata: {}", e)))?;

        debug!("Deleted node metadata for node {}", node_id);
        Ok(())
    }

    /// 存储集群配置
    pub async fn put_cluster_config(&self, key: &str, value: &str) -> Result<()> {
        let full_key = format!("{}/config/{}", self.config.key_prefix, key);

        let mut client = self.client.write().await;
        client
            .put(full_key, value.as_bytes().to_vec(), None)
            .await
            .map_err(|e| Error::Internal(format!("Failed to put cluster config: {}", e)))?;

        Ok(())
    }

    /// 获取集群配置
    pub async fn get_cluster_config(&self, key: &str) -> Result<Option<String>> {
        let full_key = format!("{}/config/{}", self.config.key_prefix, key);

        let mut client = self.client.write().await;
        let response = client
            .get(full_key, None)
            .await
            .map_err(|e| Error::Internal(format!("Failed to get cluster config: {}", e)))?;

        if let Some(kv) = response.kvs().first() {
            Ok(Some(String::from_utf8_lossy(kv.value()).to_string()))
        } else {
            Ok(None)
        }
    }

    /// 创建租约（用于心跳）
    pub async fn create_lease(&self, ttl: i64) -> Result<i64> {
        let mut client = self.client.write().await;
        let response = client
            .lease_grant(ttl, None)
            .await
            .map_err(|e| Error::Internal(format!("Failed to create lease: {}", e)))?;

        Ok(response.id())
    }

    /// 续约租约
    pub async fn keep_alive_lease(&self, lease_id: i64) -> Result<()> {
        let mut client = self.client.write().await;
        let (mut keeper, _) = client
            .lease_keep_alive(lease_id)
            .await
            .map_err(|e| Error::Internal(format!("Failed to keep alive lease: {}", e)))?;

        keeper
            .keep_alive()
            .await
            .map_err(|e| Error::Internal(format!("Failed to send keep alive: {}", e)))?;

        Ok(())
    }

    /// 使用租约存储键值（自动过期）
    pub async fn put_with_lease(&self, key: &str, value: &str, lease_id: i64) -> Result<()> {
        let full_key = format!("{}/{}", self.config.key_prefix, key);
        let options = PutOptions::new().with_lease(lease_id);

        let mut client = self.client.write().await;
        client
            .put(full_key, value.as_bytes().to_vec(), Some(options))
            .await
            .map_err(|e| Error::Internal(format!("Failed to put with lease: {}", e)))?;

        Ok(())
    }

    /// 监视键的变化
    pub async fn watch(&self, key: &str) -> Result<WatchStream> {
        let full_key = format!("{}/{}", self.config.key_prefix, key);

        let mut client = self.client.write().await;
        let (_, stream) = client
            .watch(full_key, Some(WatchOptions::new().with_prefix()))
            .await
            .map_err(|e| Error::Internal(format!("Failed to watch key: {}", e)))?;

        Ok(stream)
    }

    /// 关闭连接
    pub async fn close(&self) -> Result<()> {
        info!("Closing etcd connection");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_etcd_config_default() {
        let config = EtcdConfig::default();
        assert_eq!(config.endpoints, vec!["localhost:2379"]);
        assert_eq!(config.connect_timeout_secs, 5);
        assert!(!config.auth_enabled);
    }

    #[test]
    fn test_node_status() {
        assert_ne!(NodeStatus::Online, NodeStatus::Offline);
        assert_eq!(NodeStatus::Online, NodeStatus::Online);
    }
}
