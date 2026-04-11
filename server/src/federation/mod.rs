use serde::{Deserialize, Serialize};
use thiserror::Error;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Error)]
pub enum FederationError {
    #[error("HTTP error: {0}")]
    HttpError(#[from] reqwest::Error),
    
    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),
    
    #[error("Invalid response: {0}")]
    InvalidResponse(String),
    
    #[error("Node error: {0}")]
    NodeError(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FederationNode {
    pub url: String,
    pub name: String,
    pub weight: f64,
    pub is_active: bool,
    pub last_heartbeat: Option<i64>,
    pub region: String,
    pub cloud_provider: CloudProvider,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum CloudProvider {
    AWS,
    Azure,
    GCP,
    OnPremise,
    Other(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FederatedQuery {
    pub query: String,
    pub start: i64,
    pub end: i64,
    pub step: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FederatedResult {
    pub node_name: String,
    pub result: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FederationConfig {
    pub nodes: Vec<FederationNode>,
    pub timeout_ms: u64,
    pub retry_count: u32,
    pub heartbeat_interval_ms: u64,
}

#[derive(Debug, Clone)]
pub struct FederationManager {
    config: FederationConfig,
    nodes: Arc<RwLock<Vec<FederationNode>>>,
    client: reqwest::Client,
}

impl FederationManager {
    pub fn new(config: FederationConfig) -> Self {
        let nodes = config.nodes.clone();
        Self {
            config,
            nodes: Arc::new(RwLock::new(nodes)),
            client: reqwest::Client::new(),
        }
    }

    pub async fn start(&self) {
        let nodes = Arc::clone(&self.nodes);
        let heartbeat_interval = self.config.heartbeat_interval_ms;
        let client = self.client.clone();
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(heartbeat_interval));
            
            loop {
                interval.tick().await;
                Self::check_heartbeats(&nodes, &client).await;
            }
        });
    }

    async fn check_heartbeats(nodes: &Arc<RwLock<Vec<FederationNode>>>, client: &reqwest::Client) {
        let mut nodes_write = nodes.write().await;
        
        for node in &mut *nodes_write {
            let response = client
                .get(format!("{}/api/v1/status", node.url))
                .timeout(tokio::time::Duration::from_secs(5))
                .send()
                .await;
            
            match response {
                Ok(res) if res.status().is_success() => {
                    node.is_active = true;
                    node.last_heartbeat = Some(tokio::time::Instant::now().elapsed().as_millis() as i64);
                }
                _ => {
                    node.is_active = false;
                }
            }
        }
    }

    pub async fn federate(&self, query: FederatedQuery) -> Result<Vec<FederatedResult>, FederationError> {
        let nodes = self.nodes.read().await;
        let active_nodes: Vec<FederationNode> = nodes.iter().filter(|n| n.is_active).cloned().collect();
        
        if active_nodes.is_empty() {
            return Err(FederationError::NodeError("No active federation nodes".to_string()));
        }
        
        let mut handles = Vec::new();
        
        for node in active_nodes {
            let query_clone = query.clone();
            let node_clone = node;
            let client = self.client.clone();
            
            let handle = tokio::spawn(async move {
                let response = client
                    .post(format!("{}/api/v1/query_range", node_clone.url))
                    .json(&serde_json::json!({
                        "query": query_clone.query,
                        "start": query_clone.start,
                        "end": query_clone.end,
                        "step": query_clone.step.unwrap_or(15)
                    }))
                    .timeout(tokio::time::Duration::from_millis(10000))
                    .send()
                    .await;
                
                match response {
                    Ok(res) if res.status().is_success() => {
                        let result: serde_json::Value = res.json().await.unwrap();
                        Ok(FederatedResult {
                            node_name: node_clone.name,
                            result,
                        })
                    }
                    _ => {
                        Err(FederationError::NodeError(format!("Failed to query node: {}", node_clone.name)))
                    }
                }
            });
            
            handles.push(handle);
        }
        
        let mut results = Vec::new();
        
        for handle in handles {
            match handle.await {
                Ok(Ok(result)) => {
                    results.push(result);
                }
                Ok(Err(e)) => {
                    eprintln!("Federation error: {}", e);
                }
                Err(e) => {
                    eprintln!("Task error: {}", e);
                }
            }
        }
        
        if results.is_empty() {
            return Err(FederationError::NodeError("No successful federation results".to_string()));
        }
        
        Ok(results)
    }

    pub async fn federate_by_region(&self, query: FederatedQuery, region: &str) -> Result<Vec<FederatedResult>, FederationError> {
        let nodes = self.nodes.read().await;
        let active_nodes: Vec<FederationNode> = nodes
            .iter()
            .filter(|n| n.is_active && n.region == region)
            .cloned()
            .collect();
        
        if active_nodes.is_empty() {
            return Err(FederationError::NodeError(format!("No active federation nodes in region: {}", region)));
        }
        
        self.execute_federated_query(query, active_nodes).await
    }

    pub async fn federate_by_cloud_provider(&self, query: FederatedQuery, cloud_provider: &CloudProvider) -> Result<Vec<FederatedResult>, FederationError> {
        let nodes = self.nodes.read().await;
        let active_nodes: Vec<FederationNode> = nodes
            .iter()
            .filter(|n| n.is_active && &n.cloud_provider == cloud_provider)
            .cloned()
            .collect();
        
        if active_nodes.is_empty() {
            return Err(FederationError::NodeError(format!("No active federation nodes for cloud provider: {:?}", cloud_provider)));
        }
        
        self.execute_federated_query(query, active_nodes).await
    }

    pub async fn federate_across_regions(&self, query: FederatedQuery, regions: &[String]) -> Result<Vec<FederatedResult>, FederationError> {
        let nodes = self.nodes.read().await;
        let active_nodes: Vec<FederationNode> = nodes
            .iter()
            .filter(|n| n.is_active && regions.contains(&n.region))
            .cloned()
            .collect();
        
        if active_nodes.is_empty() {
            return Err(FederationError::NodeError("No active federation nodes in specified regions".to_string()));
        }
        
        self.execute_federated_query(query, active_nodes).await
    }

    pub async fn federate_across_clouds(&self, query: FederatedQuery) -> Result<Vec<FederatedResult>, FederationError> {
        let nodes = self.nodes.read().await;
        let active_nodes: Vec<FederationNode> = nodes.iter().filter(|n| n.is_active).cloned().collect();
        
        if active_nodes.is_empty() {
            return Err(FederationError::NodeError("No active federation nodes".to_string()));
        }
        
        self.execute_federated_query(query, active_nodes).await
    }

    async fn execute_federated_query(&self, query: FederatedQuery, nodes: Vec<FederationNode>) -> Result<Vec<FederatedResult>, FederationError> {
        let mut handles = Vec::new();
        
        for node in nodes {
            let query_clone = query.clone();
            let node_clone = node;
            let client = self.client.clone();
            
            let handle = tokio::spawn(async move {
                let response = client
                    .post(format!("{}/api/v1/query_range", node_clone.url))
                    .json(&serde_json::json!({
                        "query": query_clone.query,
                        "start": query_clone.start,
                        "end": query_clone.end,
                        "step": query_clone.step.unwrap_or(15)
                    }))
                    .timeout(tokio::time::Duration::from_millis(10000))
                    .send()
                    .await;
                
                match response {
                    Ok(res) if res.status().is_success() => {
                        let result: serde_json::Value = res.json().await.unwrap();
                        Ok(FederatedResult {
                            node_name: node_clone.name,
                            result,
                        })
                    }
                    _ => {
                        Err(FederationError::NodeError(format!("Failed to query node: {}", node_clone.name)))
                    }
                }
            });
            
            handles.push(handle);
        }
        
        let mut results = Vec::new();
        
        for handle in handles {
            match handle.await {
                Ok(Ok(result)) => {
                    results.push(result);
                }
                Ok(Err(e)) => {
                    eprintln!("Federation error: {}", e);
                }
                Err(e) => {
                    eprintln!("Task error: {}", e);
                }
            }
        }
        
        if results.is_empty() {
            return Err(FederationError::NodeError("No successful federation results".to_string()));
        }
        
        Ok(results)
    }

    pub async fn get_nodes(&self) -> Vec<FederationNode> {
        let nodes = self.nodes.read().await;
        nodes.clone()
    }

    pub async fn get_nodes_by_region(&self, region: &str) -> Vec<FederationNode> {
        let nodes = self.nodes.read().await;
        nodes
            .iter()
            .filter(|n| n.region == region)
            .cloned()
            .collect()
    }

    pub async fn get_nodes_by_cloud_provider(&self, cloud_provider: &CloudProvider) -> Vec<FederationNode> {
        let nodes = self.nodes.read().await;
        nodes
            .iter()
            .filter(|n| &n.cloud_provider == cloud_provider)
            .cloned()
            .collect()
    }

    pub async fn get_active_nodes(&self) -> Vec<FederationNode> {
        let nodes = self.nodes.read().await;
        nodes
            .iter()
            .filter(|n| n.is_active)
            .cloned()
            .collect()
    }

    pub async fn add_node(&self, node: FederationNode) {
        let mut nodes = self.nodes.write().await;
        nodes.push(node);
    }

    pub async fn remove_node(&self, node_name: &str) {
        let mut nodes = self.nodes.write().await;
        nodes.retain(|n| n.name != node_name);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn test_federation_manager() {
        let config = FederationConfig {
            nodes: vec![
                FederationNode {
                    url: "http://localhost:9090".to_string(),
                    name: "node1".to_string(),
                    weight: 1.0,
                    is_active: true,
                    last_heartbeat: Some(0),
                    region: "us-east-1".to_string(),
                    cloud_provider: CloudProvider::AWS,
                },
            ],
            timeout_ms: 10000,
            retry_count: 3,
            heartbeat_interval_ms: 60000,
        };
        
        let manager = FederationManager::new(config);
        
        // 启动心跳检查
        manager.start().await;
        
        // 等待一段时间让心跳检查运行
        tokio::time::sleep(Duration::from_secs(1)).await;
        
        // 获取节点列表
        let nodes = manager.get_nodes().await;
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].name, "node1");
        assert_eq!(nodes[0].region, "us-east-1");
        assert_eq!(nodes[0].cloud_provider, CloudProvider::AWS);
    }

    #[tokio::test]
    async fn test_add_remove_node() {
        let config = FederationConfig {
            nodes: vec![],
            timeout_ms: 10000,
            retry_count: 3,
            heartbeat_interval_ms: 60000,
        };
        
        let manager = FederationManager::new(config);
        
        // 添加节点
        let node = FederationNode {
            url: "http://localhost:9090".to_string(),
            name: "node1".to_string(),
            weight: 1.0,
            is_active: true,
            last_heartbeat: Some(0),
            region: "us-west-2".to_string(),
            cloud_provider: CloudProvider::Azure,
        };
        
        manager.add_node(node).await;
        
        // 检查节点是否添加成功
        let nodes = manager.get_nodes().await;
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].name, "node1");
        assert_eq!(nodes[0].region, "us-west-2");
        assert_eq!(nodes[0].cloud_provider, CloudProvider::Azure);
        
        // 删除节点
        manager.remove_node("node1").await;
        
        // 检查节点是否删除成功
        let nodes = manager.get_nodes().await;
        assert_eq!(nodes.len(), 0);
    }

    #[tokio::test]
    async fn test_get_nodes_by_region() {
        let config = FederationConfig {
            nodes: vec![
                FederationNode {
                    url: "http://localhost:9090".to_string(),
                    name: "node1".to_string(),
                    weight: 1.0,
                    is_active: true,
                    last_heartbeat: Some(0),
                    region: "us-east-1".to_string(),
                    cloud_provider: CloudProvider::AWS,
                },
                FederationNode {
                    url: "http://localhost:9091".to_string(),
                    name: "node2".to_string(),
                    weight: 1.0,
                    is_active: true,
                    last_heartbeat: Some(0),
                    region: "us-west-2".to_string(),
                    cloud_provider: CloudProvider::Azure,
                },
                FederationNode {
                    url: "http://localhost:9092".to_string(),
                    name: "node3".to_string(),
                    weight: 1.0,
                    is_active: true,
                    last_heartbeat: Some(0),
                    region: "us-east-1".to_string(),
                    cloud_provider: CloudProvider::GCP,
                },
            ],
            timeout_ms: 10000,
            retry_count: 3,
            heartbeat_interval_ms: 60000,
        };
        
        let manager = FederationManager::new(config);
        
        // 按区域获取节点
        let us_east_nodes = manager.get_nodes_by_region("us-east-1").await;
        assert_eq!(us_east_nodes.len(), 2);
        assert_eq!(us_east_nodes[0].name, "node1");
        assert_eq!(us_east_nodes[1].name, "node3");
        
        let us_west_nodes = manager.get_nodes_by_region("us-west-2").await;
        assert_eq!(us_west_nodes.len(), 1);
        assert_eq!(us_west_nodes[0].name, "node2");
    }

    #[tokio::test]
    async fn test_get_nodes_by_cloud_provider() {
        let config = FederationConfig {
            nodes: vec![
                FederationNode {
                    url: "http://localhost:9090".to_string(),
                    name: "node1".to_string(),
                    weight: 1.0,
                    is_active: true,
                    last_heartbeat: Some(0),
                    region: "us-east-1".to_string(),
                    cloud_provider: CloudProvider::AWS,
                },
                FederationNode {
                    url: "http://localhost:9091".to_string(),
                    name: "node2".to_string(),
                    weight: 1.0,
                    is_active: true,
                    last_heartbeat: Some(0),
                    region: "us-west-2".to_string(),
                    cloud_provider: CloudProvider::Azure,
                },
                FederationNode {
                    url: "http://localhost:9092".to_string(),
                    name: "node3".to_string(),
                    weight: 1.0,
                    is_active: true,
                    last_heartbeat: Some(0),
                    region: "us-east-1".to_string(),
                    cloud_provider: CloudProvider::AWS,
                },
            ],
            timeout_ms: 10000,
            retry_count: 3,
            heartbeat_interval_ms: 60000,
        };
        
        let manager = FederationManager::new(config);
        
        // 按云服务提供商获取节点
        let aws_nodes = manager.get_nodes_by_cloud_provider(&CloudProvider::AWS).await;
        assert_eq!(aws_nodes.len(), 2);
        assert_eq!(aws_nodes[0].name, "node1");
        assert_eq!(aws_nodes[1].name, "node3");
        
        let azure_nodes = manager.get_nodes_by_cloud_provider(&CloudProvider::Azure).await;
        assert_eq!(azure_nodes.len(), 1);
        assert_eq!(azure_nodes[0].name, "node2");
    }
}
