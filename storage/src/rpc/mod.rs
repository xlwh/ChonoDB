use crate::error::{Error, Result};
use crate::model::{TimeSeries, TimeSeriesId};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{RwLock, Semaphore};
use tokio::time;
use tracing::{debug, error, info, warn};

/// RPC请求类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RpcRequest {
    /// 写入请求
    Write(WriteRequest),
    /// 查询请求
    Query(QueryRequest),
    /// 复制请求
    Replicate(ReplicateRequest),
    /// 心跳请求
    Heartbeat(HeartbeatRequest),
    /// 获取集群状态
    GetClusterStatus,
}

/// RPC响应类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RpcResponse {
    /// 写入响应
    Write(WriteResponse),
    /// 查询响应
    Query(QueryResponse),
    /// 复制响应
    Replicate(ReplicateResponse),
    /// 心跳响应
    Heartbeat(HeartbeatResponse),
    /// 集群状态响应
    ClusterStatus(ClusterStatusResponse),
    /// 错误响应
    Error(String),
}

/// 写入请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WriteRequest {
    pub series: TimeSeries,
}

/// 写入响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WriteResponse {
    pub success: bool,
    pub message: String,
}

/// 查询请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryRequest {
    pub series_ids: Vec<TimeSeriesId>,
    pub start: i64,
    pub end: i64,
}

/// 查询响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryResponse {
    pub series: Vec<TimeSeries>,
    pub success: bool,
    pub message: String,
}

/// 复制请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplicateRequest {
    pub shard_id: u64,
    pub series: TimeSeries,
}

/// 复制响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplicateResponse {
    pub success: bool,
    pub message: String,
}

/// 心跳请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeartbeatRequest {
    pub node_id: String,
    pub timestamp: i64,
}

/// 心跳响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeartbeatResponse {
    pub success: bool,
    pub timestamp: i64,
}

/// 集群状态响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterStatusResponse {
    pub node_id: String,
    pub is_coordinator: bool,
    pub nodes: Vec<NodeInfo>,
    pub shards: HashMap<u64, Vec<String>>,
}

/// 节点信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeInfo {
    pub node_id: String,
    pub address: String,
    pub status: NodeStatus,
    pub last_heartbeat: i64,
}

/// 节点状态
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum NodeStatus {
    Online,
    Offline,
    Suspect,
}

/// RPC服务器
pub struct RpcServer {
    addr: SocketAddr,
    handler: Arc<dyn RpcHandler>,
}

impl RpcServer {
    pub fn new(addr: SocketAddr, handler: Arc<dyn RpcHandler>) -> Self {
        Self { addr, handler }
    }

    pub async fn run(&self) -> Result<()> {
        let listener = TcpListener::bind(self.addr).await?;
        info!("RPC server listening on {}", self.addr);

        loop {
            let (stream, peer_addr) = listener.accept().await?;
            debug!("New RPC connection from {}", peer_addr);

            let handler = Arc::clone(&self.handler);
            tokio::spawn(async move {
                if let Err(e) = Self::handle_connection(stream, handler).await {
                    error!("Error handling RPC connection from {}: {}", peer_addr, e);
                }
            });
        }
    }

    async fn handle_connection(
        stream: TcpStream,
        handler: Arc<dyn RpcHandler>,
    ) -> Result<()> {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let (mut reader, mut writer) = stream.into_split();

        // Read request length
        let mut len_buf = [0u8; 4];
        reader.read_exact(&mut len_buf).await?;
        let len = u32::from_le_bytes(len_buf) as usize;

        // Read request data
        let mut data = vec![0u8; len];
        reader.read_exact(&mut data).await?;

        // Deserialize request
        let request: RpcRequest = match bincode::deserialize(&data) {
            Ok(req) => req,
            Err(e) => {
                error!("Failed to deserialize RPC request: {}", e);
                let response = RpcResponse::Error(format!("Deserialization error: {}", e));
                let response_data = bincode::serialize(&response)?;
                let response_len = response_data.len() as u32;
                writer.write_all(&response_len.to_le_bytes()).await?;
                writer.write_all(&response_data).await?;
                return Ok(());
            }
        };

        // Handle request
        let response = handler.handle(request).await;

        // Serialize and send response
        let response_data = bincode::serialize(&response)?;
        let response_len = response_data.len() as u32;
        writer.write_all(&response_len.to_le_bytes()).await?;
        writer.write_all(&response_data).await?;

        Ok(())
    }
}

/// RPC客户端配置
#[derive(Debug, Clone)]
pub struct RpcClientConfig {
    pub max_connections: usize,
    pub connection_timeout_ms: u64,
    pub request_timeout_ms: u64,
    pub max_retries: usize,
    pub retry_delay_ms: u64,
}

impl Default for RpcClientConfig {
    fn default() -> Self {
        Self {
            max_connections: 10,
            connection_timeout_ms: 5000,
            request_timeout_ms: 30000,
            max_retries: 3,
            retry_delay_ms: 100,
        }
    }
}

/// RPC客户端连接池
struct ConnectionPool {
    addr: SocketAddr,
    config: RpcClientConfig,
    connections: RwLock<Vec<TcpStream>>,
    semaphore: Semaphore,
}

impl ConnectionPool {
    fn new(addr: SocketAddr, config: RpcClientConfig) -> Self {
        let max_connections = config.max_connections;
        Self {
            addr,
            config,
            connections: RwLock::new(Vec::new()),
            semaphore: Semaphore::new(max_connections),
        }
    }

    async fn get_connection(&self) -> Result<TcpStream> {
        let _permit = self.semaphore.acquire().await.map_err(|e| {
            Error::Internal(format!("Failed to acquire connection semaphore: {}", e))
        })?;

        let mut connections = self.connections.write().await;
        
        if let Some(stream) = connections.pop() {
            if stream.peer_addr().is_ok() {
                return Ok(stream);
            }
        }

        let timeout = Duration::from_millis(self.config.connection_timeout_ms);
        let stream = time::timeout(timeout, TcpStream::connect(self.addr))
            .await
            .map_err(|_| Error::Internal("Connection timeout".to_string()))??;

        Ok(stream)
    }

    async fn return_connection(&self, stream: TcpStream) {
        if let Ok(_) = stream.peer_addr() {
            if let Ok(mut connections) = self.connections.try_write() {
                if connections.len() < self.config.max_connections {
                    connections.push(stream);
                    return;
                }
            }
        }
    }
}

/// RPC客户端
pub struct RpcClient {
    pool: Arc<ConnectionPool>,
    config: RpcClientConfig,
}

impl RpcClient {
    pub fn new(addr: SocketAddr) -> Self {
        let config = RpcClientConfig::default();
        Self {
            pool: Arc::new(ConnectionPool::new(addr, config.clone())),
            config,
        }
    }

    pub fn with_config(addr: SocketAddr, config: RpcClientConfig) -> Self {
        Self {
            pool: Arc::new(ConnectionPool::new(addr, config.clone())),
            config,
        }
    }

    pub async fn call(&self, request: RpcRequest) -> Result<RpcResponse> {
        let mut last_error: Option<Error> = None;
        
        for attempt in 0..=self.config.max_retries {
            match self.try_call(&request).await {
                Ok(response) => return Ok(response),
                Err(e) => {
                    last_error = Some(e);
                    if attempt < self.config.max_retries {
                        warn!("RPC call attempt {} failed: {}, retrying...", attempt + 1, last_error.as_ref().unwrap());
                        time::sleep(Duration::from_millis(self.config.retry_delay_ms * (attempt as u64 + 1))).await;
                    }
                }
            }
        }
        
        Err(last_error.unwrap_or_else(|| Error::Internal("RPC call failed after retries".to_string())))
    }

    async fn try_call(&self, request: &RpcRequest) -> Result<RpcResponse> {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let stream = self.pool.get_connection().await?;
        let (mut reader, mut writer) = stream.into_split();

        let request_data = bincode::serialize(request)?;
        let request_len = request_data.len() as u32;

        let timeout = Duration::from_millis(self.config.request_timeout_ms);
        let result = time::timeout(timeout, async {
            writer.write_all(&request_len.to_le_bytes()).await?;
            writer.write_all(&request_data).await?;

            let mut len_buf = [0u8; 4];
            reader.read_exact(&mut len_buf).await?;
            let len = u32::from_le_bytes(len_buf) as usize;

            let mut data = vec![0u8; len];
            reader.read_exact(&mut data).await?;

            let response: RpcResponse = bincode::deserialize(&data)?;

            Ok(response)
        }).await;

        match result {
            Ok(Ok(response)) => {
                self.pool.return_connection(reader.reunite(writer).unwrap()).await;
                Ok(response)
            }
            Ok(Err(e)) => {
                Err(e)
            }
            Err(_) => {
                Err(Error::Internal("Request timeout".to_string()))
            }
        }
    }

    pub async fn write(&self, series: TimeSeries) -> Result<WriteResponse> {
        let request = RpcRequest::Write(WriteRequest { series });
        match self.call(request).await? {
            RpcResponse::Write(response) => Ok(response),
            RpcResponse::Error(e) => Err(Error::Internal(e)),
            _ => Err(Error::Internal("Unexpected response type".to_string())),
        }
    }

    pub async fn query(
        &self,
        series_ids: Vec<TimeSeriesId>,
        start: i64,
        end: i64,
    ) -> Result<QueryResponse> {
        let request = RpcRequest::Query(QueryRequest {
            series_ids,
            start,
            end,
        });
        match self.call(request).await? {
            RpcResponse::Query(response) => Ok(response),
            RpcResponse::Error(e) => Err(Error::Internal(e)),
            _ => Err(Error::Internal("Unexpected response type".to_string())),
        }
    }

    pub async fn replicate(&self, shard_id: u64, series: TimeSeries) -> Result<ReplicateResponse> {
        let request = RpcRequest::Replicate(ReplicateRequest { shard_id, series });
        match self.call(request).await? {
            RpcResponse::Replicate(response) => Ok(response),
            RpcResponse::Error(e) => Err(Error::Internal(e)),
            _ => Err(Error::Internal("Unexpected response type".to_string())),
        }
    }

    pub async fn heartbeat(&self, node_id: String) -> Result<HeartbeatResponse> {
        let request = RpcRequest::Heartbeat(HeartbeatRequest {
            node_id,
            timestamp: chrono::Utc::now().timestamp_millis(),
        });
        match self.call(request).await? {
            RpcResponse::Heartbeat(response) => Ok(response),
            RpcResponse::Error(e) => Err(Error::Internal(e)),
            _ => Err(Error::Internal("Unexpected response type".to_string())),
        }
    }

    pub async fn get_cluster_status(&self) -> Result<ClusterStatusResponse> {
        let request = RpcRequest::GetClusterStatus;
        match self.call(request).await? {
            RpcResponse::ClusterStatus(response) => Ok(response),
            RpcResponse::Error(e) => Err(Error::Internal(e)),
            _ => Err(Error::Internal("Unexpected response type".to_string())),
        }
    }
}

/// RPC处理器trait
#[async_trait]
pub trait RpcHandler: Send + Sync {
    async fn handle(&self, request: RpcRequest) -> RpcResponse;
}

/// 集群RPC管理器
pub struct ClusterRpcManager {
    clients: RwLock<HashMap<String, Arc<RpcClient>>>,
    node_addresses: RwLock<HashMap<String, SocketAddr>>,
}

impl ClusterRpcManager {
    pub fn new() -> Self {
        Self {
            clients: RwLock::new(HashMap::new()),
            node_addresses: RwLock::new(HashMap::new()),
        }
    }

    pub async fn register_node(&self, node_id: String, addr: SocketAddr) {
        let node_id_clone = node_id.clone();
        let mut clients = self.clients.write().await;
        let mut addresses = self.node_addresses.write().await;

        clients.insert(node_id_clone.clone(), Arc::new(RpcClient::new(addr)));
        addresses.insert(node_id_clone, addr);

        info!("Registered node {} at {}", node_id, addr);
    }

    pub async fn unregister_node(&self, node_id: &str) {
        let mut clients = self.clients.write().await;
        let mut addresses = self.node_addresses.write().await;

        clients.remove(node_id);
        addresses.remove(node_id);

        info!("Unregistered node {}", node_id);
    }

    pub async fn get_client(&self, node_id: &str) -> Option<Arc<RpcClient>> {
        let clients = self.clients.read().await;
        clients.get(node_id).cloned()
    }

    pub async fn get_all_clients(&self) -> Vec<(String, Arc<RpcClient>)> {
        let clients = self.clients.read().await;
        clients
            .iter()
            .map(|(k, v)| (k.clone(), Arc::clone(v)))
            .collect()
    }

    pub async fn broadcast_write(&self, series: TimeSeries) -> Vec<(String, Result<WriteResponse>)> {
        let clients = self.get_all_clients().await;
        let mut results = Vec::new();

        for (node_id, client) in clients {
            let result = client.write(series.clone()).await;
            results.push((node_id, result));
        }

        results
    }

    pub async fn broadcast_query(
        &self,
        series_ids: Vec<TimeSeriesId>,
        start: i64,
        end: i64,
    ) -> Vec<(String, Result<QueryResponse>)> {
        let clients = self.get_all_clients().await;
        let mut results = Vec::new();

        for (node_id, client) in clients {
            let result = client.query(series_ids.clone(), start, end).await;
            results.push((node_id, result));
        }

        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_status() {
        assert_ne!(NodeStatus::Online, NodeStatus::Offline);
        assert_eq!(NodeStatus::Online, NodeStatus::Online);
    }

    #[tokio::test]
    async fn test_rpc_client_new() {
        let addr = "127.0.0.1:9090".parse().unwrap();
        let client = RpcClient::new(addr);
        // Just test that it compiles and runs
    }
}
