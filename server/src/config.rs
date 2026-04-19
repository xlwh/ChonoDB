use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    /// 服务器监听地址
    pub listen_address: String,
    
    /// 服务器端口
    pub port: u16,
    
    /// 数据目录
    pub data_dir: PathBuf,
    
    /// 存储配置
    pub storage: StorageConfig,
    
    /// 查询配置
    pub query: QueryConfig,
    
    /// 规则配置
    pub rules: RulesConfig,
    
    /// 目标配置
    pub targets: TargetsConfig,
    
    /// 内存配置
    pub memory: MemoryConfig,
    
    /// 压缩配置
    pub compression: CompressionConfig,
    
    /// 日志配置
    pub log: LogConfig,
    
    /// 认证配置
    pub auth: AuthConfig,

    /// TLS 配置
    pub tls: TlsConfig,

    /// 降采样配置
    pub downsampling: DownsamplingConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    /// 存储模式: standalone | distributed
    pub mode: String,
    
    /// 存储后端: local | s3 | gcs
    pub backend: String,
    
    /// 本地存储路径
    pub local_path: Option<PathBuf>,
    
    /// 最大磁盘使用率
    pub max_disk_usage: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryConfig {
    /// 最大并发查询数
    pub max_concurrent: usize,
    
    /// 查询超时（秒）
    pub timeout: u64,
    
    /// 最大返回样本数
    pub max_samples: usize,
    
    /// 启用向量化执行
    pub enable_vectorized: bool,
    
    /// 启用查询并行化
    pub enable_parallel: bool,
    
    /// 启用自动降采样
    pub enable_auto_downsampling: bool,
    
    /// 降采样精度选择策略: auto | conservative | aggressive
    pub downsample_policy: String,
    
    /// 查询缓存大小
    pub query_cache_size: String,
    
    /// 启用查询结果缓存
    pub enable_query_cache: bool,
    
    /// 查询缓存TTL（秒）
    pub query_cache_ttl: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RulesConfig {
    /// 规则文件路径
    pub rule_files: Vec<PathBuf>,

    /// 规则评估间隔（秒）
    pub evaluation_interval: u64,
    
    /// 告警发送间隔（秒）
    pub alert_send_interval: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TargetsConfig {
    /// 目标配置文件路径
    pub config_file: Option<PathBuf>,
    
    /// 抓取间隔（秒）
    pub scrape_interval: u64,
    
    /// 抓取超时（秒）
    pub scrape_timeout: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryConfig {
    /// MemStore 大小
    pub memstore_size: String,
    
    /// WAL 大小
    pub wal_size: String,
    
    /// 查询缓存大小
    pub query_cache_size: String,
    
    /// 最大内存使用率
    pub max_memory_usage: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressionConfig {
    /// 时间列压缩
    pub time_column: ColumnCompressionConfig,
    
    /// 值列压缩
    pub value_column: ValueColumnCompressionConfig,
    
    /// 标签列压缩
    pub label_column: ColumnCompressionConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnCompressionConfig {
    /// 压缩算法
    pub algorithm: String,
    
    /// 压缩级别
    pub level: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValueColumnCompressionConfig {
    /// 压缩算法
    pub algorithm: String,
    
    /// 压缩级别
    pub level: i32,
    
    /// 使用预测编码
    pub use_prediction: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogConfig {
    /// 日志级别
    pub level: String,
    
    /// 日志格式: json | text
    pub format: String,
    
    /// 日志输出路径
    pub output: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    /// 是否启用认证
    pub enabled: bool,

    /// 认证类型: basic | bearer | api_key
    pub auth_type: String,

    /// API密钥列表（用于api_key认证）
    pub api_keys: Vec<String>,

    /// 用户名（用于basic认证）
    pub username: Option<String>,

    /// 密码（用于basic认证）
    pub password: Option<String>,

    /// JWT密钥（用于bearer认证）
    pub jwt_secret: Option<String>,

    /// 允许的IP白名单
    pub allowed_ips: Vec<String>,

    /// 是否启用IP白名单检查
    pub enable_ip_whitelist: bool,
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            auth_type: "basic".to_string(),
            api_keys: vec![],
            username: None,
            password: None,
            jwt_secret: None,
            allowed_ips: vec![],
            enable_ip_whitelist: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TlsConfig {
    /// 是否启用 TLS
    pub enabled: bool,

    /// 证书文件路径
    pub cert_file: Option<String>,

    /// 私钥文件路径
    pub key_file: Option<String>,

    /// 客户端证书 CA 文件路径（用于双向 TLS）
    pub client_ca_file: Option<String>,

    /// 是否要求客户端证书
    pub require_client_cert: bool,
}

impl Default for TlsConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            cert_file: None,
            key_file: None,
            client_ca_file: None,
            require_client_cert: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownsamplingConfig {
    /// 是否启用降采样
    pub enabled: bool,

    /// 降采样间隔（秒）
    pub interval: u64,

    /// 并发数
    pub concurrency: usize,

    /// 超时时间（秒）
    pub timeout: u64,

    /// 降采样级别配置
    pub levels: Vec<DownsamplingLevelConfig>,
}

impl Default for DownsamplingConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            interval: 900, // 15分钟
            concurrency: 4,
            timeout: 3600, // 1小时
            levels: vec![
                DownsamplingLevelConfig {
                    level: "L1".to_string(),
                    enabled: true,
                    functions: vec!["min", "max", "avg", "sum", "count", "last"]
                        .into_iter()
                        .map(String::from)
                        .collect(),
                },
                DownsamplingLevelConfig {
                    level: "L2".to_string(),
                    enabled: true,
                    functions: vec!["min", "max", "avg", "sum", "count", "last"]
                        .into_iter()
                        .map(String::from)
                        .collect(),
                },
                DownsamplingLevelConfig {
                    level: "L3".to_string(),
                    enabled: true,
                    functions: vec!["min", "max", "avg", "sum", "count", "last"]
                        .into_iter()
                        .map(String::from)
                        .collect(),
                },
                DownsamplingLevelConfig {
                    level: "L4".to_string(),
                    enabled: true,
                    functions: vec!["min", "max", "avg", "sum", "count", "last"]
                        .into_iter()
                        .map(String::from)
                        .collect(),
                },
            ],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownsamplingLevelConfig {
    /// 级别名称: L1 | L2 | L3 | L4
    pub level: String,

    /// 是否启用
    pub enabled: bool,

    /// 支持的聚合函数
    pub functions: Vec<String>,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            listen_address: "0.0.0.0".to_string(),
            port: 9090,
            data_dir: PathBuf::from("/var/lib/chronodb"),
            storage: StorageConfig {
                mode: "standalone".to_string(),
                backend: "local".to_string(),
                local_path: Some(PathBuf::from("/var/lib/chronodb/data")),
                max_disk_usage: "80%".to_string(),
            },
            query: QueryConfig {
                max_concurrent: 100,
                timeout: 120,
                max_samples: 50_000_000,
                enable_vectorized: true,
                enable_parallel: true,
                enable_auto_downsampling: true,
                downsample_policy: "auto".to_string(),
                query_cache_size: "2GB".to_string(),
                enable_query_cache: true,
                query_cache_ttl: 300,
            },
            rules: RulesConfig {
                rule_files: vec![],
                evaluation_interval: 60,
                alert_send_interval: 60,
            },
            targets: TargetsConfig {
                config_file: None,
                scrape_interval: 60,
                scrape_timeout: 10,
            },
            memory: MemoryConfig {
                memstore_size: "4GB".to_string(),
                wal_size: "1GB".to_string(),
                query_cache_size: "2GB".to_string(),
                max_memory_usage: "80%".to_string(),
            },
            compression: CompressionConfig {
                time_column: ColumnCompressionConfig {
                    algorithm: "zstd".to_string(),
                    level: 3,
                },
                value_column: ValueColumnCompressionConfig {
                    algorithm: "zstd".to_string(),
                    level: 3,
                    use_prediction: true,
                },
                label_column: ColumnCompressionConfig {
                    algorithm: "dictionary".to_string(),
                    level: 0,
                },
            },
            log: LogConfig {
                level: "info".to_string(),
                format: "json".to_string(),
                output: Some(PathBuf::from("/var/log/chronodb/chronodb.log")),
            },
            auth: AuthConfig::default(),
            tls: TlsConfig::default(),
            downsampling: DownsamplingConfig::default(),
        }
    }
}

impl ServerConfig {
    pub fn from_file(path: &std::path::Path) -> crate::Result<Self> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| crate::error::ServerError::Config(format!("Failed to read config file: {}", e)))?;
        
        let config: ServerConfig = serde_yaml::from_str(&content)
            .map_err(|e| crate::error::ServerError::Config(format!("Failed to parse config file: {}", e)))?;
        
        Ok(config)
    }
    
    pub fn save_to_file(&self, path: &std::path::Path) -> crate::Result<()> {
        let content = serde_yaml::to_string(self)
            .map_err(|e| crate::error::ServerError::Config(format!("Failed to serialize config: {}", e)))?;
        
        std::fs::write(path, content)
            .map_err(|e| crate::error::ServerError::Config(format!("Failed to write config file: {}", e)))?;
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_server_config_default() {
        let config = ServerConfig::default();
        assert_eq!(config.listen_address, "0.0.0.0");
        assert_eq!(config.port, 9090);
        assert_eq!(config.storage.mode, "standalone");
        assert_eq!(config.storage.backend, "local");
        assert_eq!(config.query.max_concurrent, 100);
        assert_eq!(config.query.timeout, 120);
        assert!(config.query.enable_vectorized);
        assert!(config.query.enable_parallel);
        assert!(config.query.enable_auto_downsampling);
        assert_eq!(config.rules.evaluation_interval, 60);
        assert_eq!(config.targets.scrape_interval, 60);
        assert_eq!(config.compression.time_column.algorithm, "zstd");
        assert_eq!(config.compression.value_column.algorithm, "zstd");
        assert!(config.compression.value_column.use_prediction);
        assert_eq!(config.log.level, "info");
    }

    #[test]
    fn test_server_config_serialization() {
        let config = ServerConfig::default();
        let yaml = serde_yaml::to_string(&config).unwrap();
        assert!(yaml.contains("listen_address"));
        assert!(yaml.contains("port"));
    }

    #[test]
    fn test_server_config_roundtrip() {
        let config = ServerConfig::default();
        let yaml = serde_yaml::to_string(&config).unwrap();
        let deserialized: ServerConfig = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(deserialized.listen_address, config.listen_address);
        assert_eq!(deserialized.port, config.port);
        assert_eq!(deserialized.query.max_concurrent, config.query.max_concurrent);
    }

    #[test]
    fn test_storage_config() {
        let config = ServerConfig::default();
        assert_eq!(config.storage.mode, "standalone");
        assert_eq!(config.storage.backend, "local");
    }

    #[test]
    fn test_query_config() {
        let config = ServerConfig::default();
        assert_eq!(config.query.max_samples, 50_000_000);
        assert_eq!(config.query.downsample_policy, "auto");
        assert_eq!(config.query.query_cache_ttl, 300);
    }

    #[test]
    fn test_rules_config() {
        let config = ServerConfig::default();
        assert!(config.rules.rule_files.is_empty());
        assert_eq!(config.rules.alert_send_interval, 60);
    }

    #[test]
    fn test_compression_config() {
        let config = ServerConfig::default();
        assert_eq!(config.compression.label_column.algorithm, "dictionary");
        assert_eq!(config.compression.time_column.level, 3);
    }

    #[test]
    fn test_config_save_and_load() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config_path = temp_dir.path().join("config.yaml");

        let config = ServerConfig::default();
        config.save_to_file(&config_path).unwrap();

        let loaded = ServerConfig::from_file(&config_path).unwrap();
        assert_eq!(loaded.port, 9090);
        assert_eq!(loaded.listen_address, "0.0.0.0");
    }
}
