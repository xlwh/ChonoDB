use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// ChronoDB YAML配置文件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChronoDBConfig {
    /// 监听地址
    #[serde(default = "default_listen_address")]
    pub listen_address: String,

    /// 存储配置
    #[serde(default)]
    pub storage: StorageConfigYaml,

    /// 降采样配置
    #[serde(default)]
    pub downsampling: DownsamplingConfigYaml,

    /// 内存配置
    #[serde(default)]
    pub memory: MemoryConfigYaml,

    /// 压缩配置
    #[serde(default)]
    pub compression: CompressionConfigYaml,

    /// 查询配置
    #[serde(default)]
    pub query: QueryConfigYaml,

    /// 数据保留策略
    #[serde(default)]
    pub retention: RetentionConfigYaml,

    /// 日志配置
    #[serde(default)]
    pub log: LogConfigYaml,
}

fn default_listen_address() -> String {
    "0.0.0.0:9090".to_string()
}

impl Default for ChronoDBConfig {
    fn default() -> Self {
        Self {
            listen_address: default_listen_address(),
            storage: StorageConfigYaml::default(),
            downsampling: DownsamplingConfigYaml::default(),
            memory: MemoryConfigYaml::default(),
            compression: CompressionConfigYaml::default(),
            query: QueryConfigYaml::default(),
            retention: RetentionConfigYaml::default(),
            log: LogConfigYaml::default(),
        }
    }
}

impl ChronoDBConfig {
    /// 从YAML文件加载配置
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| crate::error::Error::ConfigError(format!("Failed to read config file: {}", e)))?;
        
        Self::from_yaml(&content)
    }

    /// 从YAML字符串解析配置
    pub fn from_yaml(yaml: &str) -> Result<Self> {
        serde_yaml::from_str(yaml)
            .map_err(|e| crate::error::Error::ConfigError(format!("Failed to parse YAML: {}", e)))
    }

    /// 保存配置到YAML文件
    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let yaml = serde_yaml::to_string(self)
            .map_err(|e| crate::error::Error::ConfigError(format!("Failed to serialize config: {}", e)))?;
        
        std::fs::write(path, yaml)
            .map_err(|e| crate::error::Error::ConfigError(format!("Failed to write config file: {}", e)))
    }
}

/// 存储配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfigYaml {
    /// 存储模式: standalone | distributed
    #[serde(default = "default_storage_mode")]
    pub mode: String,

    /// 数据目录
    #[serde(default = "default_data_dir")]
    pub data_dir: String,

    /// 存储后端: local | hdfs | s3
    #[serde(default = "default_storage_backend")]
    pub backend: String,
}

fn default_storage_mode() -> String {
    "standalone".to_string()
}

fn default_data_dir() -> String {
    "/var/lib/chronodb".to_string()
}

fn default_storage_backend() -> String {
    "local".to_string()
}

impl Default for StorageConfigYaml {
    fn default() -> Self {
        Self {
            mode: default_storage_mode(),
            data_dir: default_data_dir(),
            backend: default_storage_backend(),
        }
    }
}

/// 降采样配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownsamplingConfigYaml {
    /// 是否启用自动降采样
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// 降采样层级配置
    #[serde(default = "default_downsample_levels")]
    pub levels: Vec<DownsampleLevelConfig>,

    /// 降采样任务配置
    #[serde(default)]
    pub task: DownsampleTaskConfig,
}

fn default_true() -> bool {
    true
}

fn default_downsample_levels() -> Vec<DownsampleLevelConfig> {
    vec![
        DownsampleLevelConfig {
            level: "L0".to_string(),
            resolution: "10s".to_string(),
            retention: "168h".to_string(),
            functions: None,
        },
        DownsampleLevelConfig {
            level: "L1".to_string(),
            resolution: "1m".to_string(),
            retention: "720h".to_string(),
            functions: Some(vec![
                "min".to_string(),
                "max".to_string(),
                "avg".to_string(),
                "sum".to_string(),
                "count".to_string(),
                "last".to_string(),
            ]),
        },
        DownsampleLevelConfig {
            level: "L2".to_string(),
            resolution: "5m".to_string(),
            retention: "2160h".to_string(),
            functions: Some(vec![
                "min".to_string(),
                "max".to_string(),
                "avg".to_string(),
                "sum".to_string(),
                "count".to_string(),
                "last".to_string(),
            ]),
        },
        DownsampleLevelConfig {
            level: "L3".to_string(),
            resolution: "1h".to_string(),
            retention: "8760h".to_string(),
            functions: Some(vec![
                "min".to_string(),
                "max".to_string(),
                "avg".to_string(),
                "sum".to_string(),
                "count".to_string(),
                "last".to_string(),
            ]),
        },
        DownsampleLevelConfig {
            level: "L4".to_string(),
            resolution: "1d".to_string(),
            retention: "87600h".to_string(),
            functions: Some(vec![
                "min".to_string(),
                "max".to_string(),
                "avg".to_string(),
                "sum".to_string(),
                "count".to_string(),
                "last".to_string(),
            ]),
        },
    ]
}

impl Default for DownsamplingConfigYaml {
    fn default() -> Self {
        Self {
            enabled: true,
            levels: default_downsample_levels(),
            task: DownsampleTaskConfig::default(),
        }
    }
}

/// 降采样层级配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownsampleLevelConfig {
    pub level: String,
    pub resolution: String,
    pub retention: String,
    pub functions: Option<Vec<String>>,
}

/// 降采样任务配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownsampleTaskConfig {
    /// 执行间隔
    #[serde(default = "default_task_interval")]
    pub interval: String,

    /// 并发数
    #[serde(default = "default_task_concurrency")]
    pub concurrency: usize,

    /// 超时时间
    #[serde(default = "default_task_timeout")]
    pub timeout: String,
}

fn default_task_interval() -> String {
    "15m".to_string()
}

fn default_task_concurrency() -> usize {
    4
}

fn default_task_timeout() -> String {
    "1h".to_string()
}

impl Default for DownsampleTaskConfig {
    fn default() -> Self {
        Self {
            interval: default_task_interval(),
            concurrency: default_task_concurrency(),
            timeout: default_task_timeout(),
        }
    }
}

/// 内存配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryConfigYaml {
    /// MemStore大小
    #[serde(default = "default_memstore_size")]
    pub memstore_size: String,

    /// WAL大小
    #[serde(default = "default_wal_size")]
    pub wal_size: String,

    /// 查询缓存大小
    #[serde(default = "default_query_cache_size")]
    pub query_cache_size: String,
}

fn default_memstore_size() -> String {
    "4GB".to_string()
}

fn default_wal_size() -> String {
    "1GB".to_string()
}

fn default_query_cache_size() -> String {
    "2GB".to_string()
}

impl Default for MemoryConfigYaml {
    fn default() -> Self {
        Self {
            memstore_size: default_memstore_size(),
            wal_size: default_wal_size(),
            query_cache_size: default_query_cache_size(),
        }
    }
}

/// 压缩配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressionConfigYaml {
    /// 时间列压缩
    #[serde(default)]
    pub time_column: ColumnCompressionConfig,

    /// 值列压缩
    #[serde(default)]
    pub value_column: ValueColumnCompressionConfig,

    /// 标签列压缩
    #[serde(default)]
    pub label_column: ColumnCompressionConfig,
}

impl Default for CompressionConfigYaml {
    fn default() -> Self {
        Self {
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
        }
    }
}

/// 列压缩配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnCompressionConfig {
    #[serde(default = "default_compression_algorithm")]
    pub algorithm: String,
    #[serde(default = "default_compression_level")]
    pub level: i32,
}

impl Default for ColumnCompressionConfig {
    fn default() -> Self {
        Self {
            algorithm: default_compression_algorithm(),
            level: default_compression_level(),
        }
    }
}

/// 值列压缩配置（支持预测编码）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValueColumnCompressionConfig {
    #[serde(default = "default_compression_algorithm")]
    pub algorithm: String,
    #[serde(default = "default_compression_level")]
    pub level: i32,
    #[serde(default)]
    pub use_prediction: bool,
}

impl Default for ValueColumnCompressionConfig {
    fn default() -> Self {
        Self {
            algorithm: default_compression_algorithm(),
            level: default_compression_level(),
            use_prediction: false,
        }
    }
}

fn default_compression_algorithm() -> String {
    "zstd".to_string()
}

fn default_compression_level() -> i32 {
    3
}

/// 查询配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryConfigYaml {
    /// 最大并发查询数
    #[serde(default = "default_max_concurrent")]
    pub max_concurrent: usize,

    /// 查询超时
    #[serde(default = "default_query_timeout")]
    pub timeout: String,

    /// 最大样本数
    #[serde(default = "default_max_samples")]
    pub max_samples: usize,

    /// 启用向量化执行
    #[serde(default = "default_true")]
    pub enable_vectorized: bool,

    /// 启用查询并行化
    #[serde(default = "default_true")]
    pub enable_parallel: bool,

    /// 启用自动降采样
    #[serde(default = "default_true")]
    pub enable_auto_downsampling: bool,

    /// 降采样策略
    #[serde(default = "default_downsample_policy")]
    pub downsample_policy: String,
}

fn default_max_concurrent() -> usize {
    100
}

fn default_query_timeout() -> String {
    "2m".to_string()
}

fn default_max_samples() -> usize {
    50000000
}

fn default_downsample_policy() -> String {
    "auto".to_string()
}

impl Default for QueryConfigYaml {
    fn default() -> Self {
        Self {
            max_concurrent: default_max_concurrent(),
            timeout: default_query_timeout(),
            max_samples: default_max_samples(),
            enable_vectorized: true,
            enable_parallel: true,
            enable_auto_downsampling: true,
            downsample_policy: default_downsample_policy(),
        }
    }
}

/// 数据保留策略
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetentionConfigYaml {
    /// 热数据保留时间
    #[serde(default = "default_retention_hot")]
    pub hot: String,

    /// 温数据保留时间
    #[serde(default = "default_retention_warm")]
    pub warm: String,

    /// 冷数据保留时间
    #[serde(default = "default_retention_cold")]
    pub cold: String,

    /// 归档数据保留时间
    #[serde(default = "default_retention_archive")]
    pub archive: String,
}

fn default_retention_hot() -> String {
    "24h".to_string()
}

fn default_retention_warm() -> String {
    "168h".to_string()
}

fn default_retention_cold() -> String {
    "720h".to_string()
}

fn default_retention_archive() -> String {
    "8760h".to_string()
}

impl Default for RetentionConfigYaml {
    fn default() -> Self {
        Self {
            hot: default_retention_hot(),
            warm: default_retention_warm(),
            cold: default_retention_cold(),
            archive: default_retention_archive(),
        }
    }
}

/// 日志配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogConfigYaml {
    /// 日志级别
    #[serde(default = "default_log_level")]
    pub level: String,

    /// 日志格式
    #[serde(default = "default_log_format")]
    pub format: String,

    /// 日志输出路径
    #[serde(default)]
    pub output: Option<String>,
}

fn default_log_level() -> String {
    "info".to_string()
}

fn default_log_format() -> String {
    "json".to_string()
}

impl Default for LogConfigYaml {
    fn default() -> Self {
        Self {
            level: default_log_level(),
            format: default_log_format(),
            output: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = ChronoDBConfig::default();
        assert_eq!(config.listen_address, "0.0.0.0:9090");
        assert_eq!(config.storage.mode, "standalone");
        assert_eq!(config.downsampling.enabled, true);
        assert_eq!(config.downsampling.levels.len(), 5);
    }

    #[test]
    fn test_parse_yaml() {
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

        let config = ChronoDBConfig::from_yaml(yaml).unwrap();
        assert_eq!(config.listen_address, "0.0.0.0:9090");
        assert_eq!(config.storage.mode, "standalone");
        assert_eq!(config.downsampling.enabled, true);
        assert_eq!(config.downsampling.levels.len(), 1);
    }
}
