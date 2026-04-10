use crate::error::Result;
use crate::config::ChronoDBConfig;
use crate::distributed::DistributedConfig;
use std::path::Path;
use tracing::info;

/// 配置管理器
pub struct ConfigManager {
    config: ChronoDBConfig,
}

impl ConfigManager {
    /// 从文件加载配置
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let config = ChronoDBConfig::from_file(path)?;
        Ok(Self { config })
    }
    
    /// 从YAML字符串加载配置
    pub fn from_yaml(yaml: &str) -> Result<Self> {
        let config = ChronoDBConfig::from_yaml(yaml)?;
        Ok(Self { config })
    }
    
    /// 获取配置
    pub fn config(&self) -> &ChronoDBConfig {
        &self.config
    }
    
    /// 获取分布式配置
    pub fn distributed_config(&self) -> DistributedConfig {
        DistributedConfig::from_yaml_config(&self.config.storage.distributed)
    }
    
    /// 检查配置有效性
    pub fn validate(&self) -> Result<()> {
        // 验证存储配置
        if self.config.storage.data_dir.is_empty() {
            return Err(crate::error::Error::ConfigError("Data directory is required".to_string()));
        }
        
        // 验证分布式配置
        if self.config.storage.mode == "distributed" {
            if self.config.storage.distributed.node_address.is_empty() {
                return Err(crate::error::Error::ConfigError("Node address is required for distributed mode".to_string()));
            }
            
            if self.config.storage.distributed.coordinator_address.is_empty() {
                return Err(crate::error::Error::ConfigError("Coordinator address is required for distributed mode".to_string()));
            }
        }
        
        // 验证降采样配置
        if self.config.downsampling.enabled {
            if self.config.downsampling.levels.is_empty() {
                return Err(crate::error::Error::ConfigError("At least one downsampling level is required".to_string()));
            }
        }
        
        info!("Configuration validation passed");
        Ok(())
    }
    
    /// 保存配置到文件
    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        self.config.save_to_file(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use std::fs::File;
    use std::io::Write;
    
    #[test]
    fn test_config_manager_from_file() {
        let temp_dir = tempdir().unwrap();
        let config_path = temp_dir.path().join("config.yaml");
        
        let yaml = r#"
listen_address: "0.0.0.0:9090"
storage:
  mode: distributed
  data_dir: /var/lib/chronodb
  backend: local
  distributed:
    node_id: node1
    cluster_name: test-cluster
    node_address: 127.0.0.1:9090
    coordinator_address: 127.0.0.1:9091
    is_coordinator: true
    shard:
      count: 16
      strategy: hash
      virtual_nodes: 128
    replication:
      factor: 3
      strategy: asynchronous
      timeout: 5s
    cluster:
      heartbeat_interval_ms: 5000
      node_timeout_ms: 15000
      discovery_addresses:
        - 127.0.0.1:9091
      enable_auto_discovery: true
downsampling:
  enabled: true
  levels:
    - level: L0
      resolution: 10s
      retention: 168h
"#;
        
        let mut file = File::create(&config_path).unwrap();
        file.write_all(yaml.as_bytes()).unwrap();
        
        let manager = ConfigManager::from_file(config_path).unwrap();
        let config = manager.config();
        
        assert_eq!(config.listen_address, "0.0.0.0:9090");
        assert_eq!(config.storage.mode, "distributed");
        assert_eq!(config.storage.distributed.node_id, Some("node1".to_string()));
        assert_eq!(config.storage.distributed.cluster_name, "test-cluster");
        
        let distributed_config = manager.distributed_config();
        assert_eq!(distributed_config.node_id, "node1");
        assert_eq!(distributed_config.cluster_name, "test-cluster");
        assert_eq!(distributed_config.is_coordinator, true);
    }
    
    #[test]
    fn test_config_validation() {
        let yaml = r#"
listen_address: "0.0.0.0:9090"
storage:
  mode: standalone
  data_dir: /var/lib/chronodb
  backend: local
downsampling:
  enabled: true
  levels:
    - level: L0
      resolution: 10s
      retention: 168h
"#;
        
        let manager = ConfigManager::from_yaml(yaml).unwrap();
        assert!(manager.validate().is_ok());
    }
}
