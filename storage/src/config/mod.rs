pub mod yaml_config;
pub mod manager;

pub use yaml_config::{
    ChronoDBConfig, StorageConfigYaml, DownsamplingConfigYaml, DownsampleLevelConfig,
    DownsampleTaskConfig, MemoryConfigYaml, CompressionConfigYaml, ColumnCompressionConfig,
    ValueColumnCompressionConfig, QueryConfigYaml, RetentionConfigYaml, LogConfigYaml,
    DistributedConfigYaml, ShardConfigYaml, ReplicationConfigYaml, ClusterConfigYaml,
};

pub use manager::ConfigManager;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    pub data_dir: String,
    pub memstore_size: usize,
    pub wal_size: usize,
    pub wal_sync_interval_ms: u64,
    pub block_size: usize,
    pub compression: CompressionConfig,
    pub retention: RetentionConfig,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            data_dir: "/var/lib/chronodb".to_string(),
            memstore_size: 4 * 1024 * 1024 * 1024,
            wal_size: 1024 * 1024 * 1024,
            wal_sync_interval_ms: 100,
            block_size: 64 * 1024,
            compression: CompressionConfig::default(),
            retention: RetentionConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressionConfig {
    pub time_column: ColumnCompression,
    pub value_column: ColumnCompression,
    pub label_column: ColumnCompression,
}

impl Default for CompressionConfig {
    fn default() -> Self {
        Self {
            time_column: ColumnCompression::zstd_level(3),
            value_column: ColumnCompression::zstd_level(3),
            label_column: ColumnCompression::dictionary(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnCompression {
    pub algorithm: CompressionAlgorithm,
    pub level: i32,
    pub use_prediction: bool,
}

impl ColumnCompression {
    pub fn zstd_level(level: i32) -> Self {
        Self {
            algorithm: CompressionAlgorithm::Zstd,
            level,
            use_prediction: false,
        }
    }

    pub fn dictionary() -> Self {
        Self {
            algorithm: CompressionAlgorithm::Dictionary,
            level: 0,
            use_prediction: false,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum CompressionAlgorithm {
    None,
    Zstd,
    Dictionary,
    Delta,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetentionConfig {
    pub hot_duration_hours: u64,
    pub warm_duration_hours: u64,
    pub cold_duration_hours: u64,
    pub archive_duration_hours: u64,
}

impl Default for RetentionConfig {
    fn default() -> Self {
        Self {
            hot_duration_hours: 24,
            warm_duration_hours: 168,
            cold_duration_hours: 720,
            archive_duration_hours: 8760,
        }
    }
}
