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
