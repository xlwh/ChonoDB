use serde::{Deserialize, Serialize};
use thiserror::Error;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::time::interval;

#[derive(Debug, Error)]
pub enum ServiceDiscoveryError {
    #[error("Kubernetes error: {0}")]
    KubernetesError(String),
    
    #[error("Consul error: {0}")]
    ConsulError(String),
    
    #[error("DNS error: {0}")]
    DnsError(String),
    
    #[error("Config error: {0}")]
    ConfigError(String),
    
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Target {
    pub url: String,
    pub labels: std::collections::HashMap<String, String>,
    pub last_scrape: Option<i64>,
    pub health: TargetHealth,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TargetHealth {
    Healthy,
    Unhealthy,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ServiceDiscoveryType {
    Kubernetes(KubernetesConfig),
    Consul(ConsulConfig),
    Dns(DnsConfig),
    Static(StaticConfig),
    Etcd(EtcdConfig),
    Zookeeper(ZookeeperConfig),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KubernetesConfig {
    pub namespace: String,
    pub selector: String,
    pub port: Option<u16>,
    pub interval: Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsulConfig {
    pub address: String,
    pub datacenter: Option<String>,
    pub service_name: String,
    pub interval: Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DnsConfig {
    pub name: String,
    pub port: u16,
    pub interval: Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StaticConfig {
    pub targets: Vec<String>,
    pub labels: std::collections::HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EtcdConfig {
    pub endpoints: Vec<String>,
    pub key_prefix: String,
    pub interval: Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZookeeperConfig {
    pub hosts: Vec<String>,
    pub path: String,
    pub interval: Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceDiscoveryConfig {
    pub discovery_type: ServiceDiscoveryType,
    pub scrape_interval: Duration,
    pub scrape_timeout: Duration,
}

#[async_trait::async_trait]
pub trait ServiceDiscoverer: Send + Sync {
    async fn discover(&self) -> Result<Vec<Target>, ServiceDiscoveryError>;
}

#[derive(Debug, Clone)]
pub struct KubernetesDiscoverer {
    config: KubernetesConfig,
}

impl KubernetesDiscoverer {
    pub fn new(config: KubernetesConfig) -> Self {
        Self { config }
    }
}

#[async_trait::async_trait]
impl ServiceDiscoverer for KubernetesDiscoverer {
    async fn discover(&self) -> Result<Vec<Target>, ServiceDiscoveryError> {
        // 模拟 Kubernetes 服务发现
        // 在实际实现中，这里应该：
        // 1. 调用 Kubernetes API
        // 2. 根据 selector 选择 Pods
        // 3. 构建目标列表
        
        Ok(vec![
            Target {
                url: "http://pod1:9100/metrics".to_string(),
                labels: std::collections::HashMap::from([
                    ("app".to_string(), "prometheus".to_string()),
                    ("namespace".to_string(), self.config.namespace.clone()),
                ]),
                last_scrape: None,
                health: TargetHealth::Unknown,
            },
            Target {
                url: "http://pod2:9100/metrics".to_string(),
                labels: std::collections::HashMap::from([
                    ("app".to_string(), "prometheus".to_string()),
                    ("namespace".to_string(), self.config.namespace.clone()),
                ]),
                last_scrape: None,
                health: TargetHealth::Unknown,
            },
        ])
    }
}

#[derive(Debug, Clone)]
pub struct ConsulDiscoverer {
    config: ConsulConfig,
}

impl ConsulDiscoverer {
    pub fn new(config: ConsulConfig) -> Self {
        Self { config }
    }
}

#[async_trait::async_trait]
impl ServiceDiscoverer for ConsulDiscoverer {
    async fn discover(&self) -> Result<Vec<Target>, ServiceDiscoveryError> {
        // 模拟 Consul 服务发现
        // 在实际实现中，这里应该：
        // 1. 调用 Consul API
        // 2. 根据 service_name 查找服务
        // 3. 构建目标列表
        
        Ok(vec![
            Target {
                url: format!("http://{}:9100/metrics", "consul-service-1"),
                labels: std::collections::HashMap::from([
                    ("service".to_string(), self.config.service_name.clone()),
                    ("datacenter".to_string(), self.config.datacenter.clone().unwrap_or("dc1".to_string())),
                ]),
                last_scrape: None,
                health: TargetHealth::Unknown,
            },
        ])
    }
}

#[derive(Debug, Clone)]
pub struct DnsDiscoverer {
    config: DnsConfig,
}

impl DnsDiscoverer {
    pub fn new(config: DnsConfig) -> Self {
        Self { config }
    }
}

#[async_trait::async_trait]
impl ServiceDiscoverer for DnsDiscoverer {
    async fn discover(&self) -> Result<Vec<Target>, ServiceDiscoveryError> {
        // 模拟 DNS 服务发现
        // 在实际实现中，这里应该：
        // 1. 执行 DNS 查找
        // 2. 解析 A/AAAA 记录
        // 3. 构建目标列表
        
        Ok(vec![
            Target {
                url: format!("http://{}:{}/metrics", "dns-service-1", self.config.port),
                labels: std::collections::HashMap::from([
                    ("dns_name".to_string(), self.config.name.clone()),
                ]),
                last_scrape: None,
                health: TargetHealth::Unknown,
            },
            Target {
                url: format!("http://{}:{}/metrics", "dns-service-2", self.config.port),
                labels: std::collections::HashMap::from([
                    ("dns_name".to_string(), self.config.name.clone()),
                ]),
                last_scrape: None,
                health: TargetHealth::Unknown,
            },
        ])
    }
}

#[derive(Debug, Clone)]
pub struct StaticDiscoverer {
    config: StaticConfig,
}

impl StaticDiscoverer {
    pub fn new(config: StaticConfig) -> Self {
        Self { config }
    }
}

#[async_trait::async_trait]
impl ServiceDiscoverer for StaticDiscoverer {
    async fn discover(&self) -> Result<Vec<Target>, ServiceDiscoveryError> {
        // 静态目标发现
        let mut targets = Vec::new();
        
        for target in &self.config.targets {
            targets.push(Target {
                url: target.clone(),
                labels: self.config.labels.clone(),
                last_scrape: None,
                health: TargetHealth::Unknown,
            });
        }
        
        Ok(targets)
    }
}

#[derive(Debug, Clone)]
pub struct EtcdDiscoverer {
    config: EtcdConfig,
}

impl EtcdDiscoverer {
    pub fn new(config: EtcdConfig) -> Self {
        Self { config }
    }
}

#[async_trait::async_trait]
impl ServiceDiscoverer for EtcdDiscoverer {
    async fn discover(&self) -> Result<Vec<Target>, ServiceDiscoveryError> {
        // 模拟 Etcd 服务发现
        // 在实际实现中，这里应该：
        // 1. 连接到 Etcd 集群
        // 2. 读取 key_prefix 下的所有键值
        // 3. 解析服务信息并构建目标列表
        
        Ok(vec![
            Target {
                url: "http://etcd-service-1:9100/metrics".to_string(),
                labels: std::collections::HashMap::from([
                    ("etcd_prefix".to_string(), self.config.key_prefix.clone()),
                    ("endpoint".to_string(), self.config.endpoints[0].clone()),
                ]),
                last_scrape: None,
                health: TargetHealth::Unknown,
            },
            Target {
                url: "http://etcd-service-2:9100/metrics".to_string(),
                labels: std::collections::HashMap::from([
                    ("etcd_prefix".to_string(), self.config.key_prefix.clone()),
                    ("endpoint".to_string(), self.config.endpoints[0].clone()),
                ]),
                last_scrape: None,
                health: TargetHealth::Unknown,
            },
        ])
    }
}

#[derive(Debug, Clone)]
pub struct ZookeeperDiscoverer {
    config: ZookeeperConfig,
}

impl ZookeeperDiscoverer {
    pub fn new(config: ZookeeperConfig) -> Self {
        Self { config }
    }
}

#[async_trait::async_trait]
impl ServiceDiscoverer for ZookeeperDiscoverer {
    async fn discover(&self) -> Result<Vec<Target>, ServiceDiscoveryError> {
        // 模拟 Zookeeper 服务发现
        // 在实际实现中，这里应该：
        // 1. 连接到 Zookeeper 集群
        // 2. 读取 path 下的所有子节点
        // 3. 解析服务信息并构建目标列表
        
        Ok(vec![
            Target {
                url: "http://zookeeper-service-1:9100/metrics".to_string(),
                labels: std::collections::HashMap::from([
                    ("zookeeper_path".to_string(), self.config.path.clone()),
                    ("host".to_string(), self.config.hosts[0].clone()),
                ]),
                last_scrape: None,
                health: TargetHealth::Unknown,
            },
            Target {
                url: "http://zookeeper-service-2:9100/metrics".to_string(),
                labels: std::collections::HashMap::from([
                    ("zookeeper_path".to_string(), self.config.path.clone()),
                    ("host".to_string(), self.config.hosts[0].clone()),
                ]),
                last_scrape: None,
                health: TargetHealth::Unknown,
            },
        ])
    }
}

#[derive(Clone)]
pub struct ServiceDiscoveryManager {
    discoverers: Vec<Arc<dyn ServiceDiscoverer>>,
    targets: Arc<RwLock<Vec<Target>>>,
}

impl ServiceDiscoveryManager {
    pub fn new() -> Self {
        Self {
            discoverers: Vec::new(),
            targets: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub fn add_discoverer(&mut self, discoverer: Arc<dyn ServiceDiscoverer>) {
        self.discoverers.push(discoverer);
    }

    pub async fn start(&self) {
        let targets = Arc::clone(&self.targets);
        let discoverers = self.discoverers.clone();
        
        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(60));
            
            loop {
                interval.tick().await;
                ServiceDiscoveryManager::run_discovery(&targets, &discoverers).await;
            }
        });
    }

    async fn run_discovery(targets: &Arc<RwLock<Vec<Target>>>, discoverers: &[Arc<dyn ServiceDiscoverer>]) {
        let mut new_targets = Vec::new();
        
        for discoverer in discoverers {
            match discoverer.discover().await {
                Ok(discovered) => {
                    new_targets.extend(discovered);
                }
                Err(e) => {
                    eprintln!("Service discovery error: {}", e);
                }
            }
        }
        
        let mut targets_write = targets.write().await;
        *targets_write = new_targets;
    }

    pub async fn get_targets(&self) -> Vec<Target> {
        let targets = self.targets.read().await;
        targets.clone()
    }

    pub async fn get_healthy_targets(&self) -> Vec<Target> {
        let targets = self.targets.read().await;
        targets
            .iter()
            .filter(|t| t.health == TargetHealth::Healthy)
            .cloned()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn test_kubernetes_discovery() {
        let config = KubernetesConfig {
            namespace: "default".to_string(),
            selector: "app=prometheus".to_string(),
            port: Some(9100),
            interval: Duration::from_secs(60),
        };
        
        let discoverer = KubernetesDiscoverer::new(config);
        let targets = discoverer.discover().await;
        assert!(targets.is_ok());
        assert!(!targets.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_consul_discovery() {
        let config = ConsulConfig {
            address: "http://localhost:8500".to_string(),
            datacenter: Some("dc1".to_string()),
            service_name: "prometheus".to_string(),
            interval: Duration::from_secs(60),
        };
        
        let discoverer = ConsulDiscoverer::new(config);
        let targets = discoverer.discover().await;
        assert!(targets.is_ok());
        assert!(!targets.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_dns_discovery() {
        let config = DnsConfig {
            name: "prometheus.service.consul".to_string(),
            port: 9100,
            interval: Duration::from_secs(60),
        };
        
        let discoverer = DnsDiscoverer::new(config);
        let targets = discoverer.discover().await;
        assert!(targets.is_ok());
        assert!(!targets.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_static_discovery() {
        let config = StaticConfig {
            targets: vec![
                "http://localhost:9100/metrics".to_string(),
                "http://localhost:9101/metrics".to_string(),
            ],
            labels: std::collections::HashMap::from([
                ("job".to_string(), "prometheus".to_string()),
            ]),
        };
        
        let discoverer = StaticDiscoverer::new(config);
        let targets = discoverer.discover().await;
        assert!(targets.is_ok());
        assert_eq!(targets.unwrap().len(), 2);
    }

    #[tokio::test]
    async fn test_service_discovery_manager() {
        let mut manager = ServiceDiscoveryManager::new();
        
        // 添加静态发现器
        let static_config = StaticConfig {
            targets: vec!["http://localhost:9100/metrics".to_string()],
            labels: std::collections::HashMap::new(),
        };
        let static_discoverer = Arc::new(StaticDiscoverer::new(static_config));
        manager.add_discoverer(static_discoverer);
        
        // 启动服务发现
        manager.start().await;
        
        // 等待一段时间让服务发现运行
        tokio::time::sleep(Duration::from_secs(1)).await;
        
        // 获取目标
        let targets = manager.get_targets().await;
        assert!(!targets.is_empty());
    }

    #[tokio::test]
    async fn test_etcd_discovery() {
        let config = EtcdConfig {
            endpoints: vec!["http://localhost:2379".to_string()],
            key_prefix: "/services".to_string(),
            interval: Duration::from_secs(60),
        };
        
        let discoverer = EtcdDiscoverer::new(config);
        let targets = discoverer.discover().await;
        assert!(targets.is_ok());
        assert!(!targets.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_zookeeper_discovery() {
        let config = ZookeeperConfig {
            hosts: vec!["localhost:2181".to_string()],
            path: "/services".to_string(),
            interval: Duration::from_secs(60),
        };
        
        let discoverer = ZookeeperDiscoverer::new(config);
        let targets = discoverer.discover().await;
        assert!(targets.is_ok());
        assert!(!targets.unwrap().is_empty());
    }
}