pub mod tier;
pub mod manager;
pub mod migration;
pub mod auto_migration;

pub use tier::{DataTier, TierConfig, TierStats, TierCollection};
pub use manager::TieredStorageManager;
pub use migration::{MigrationTask, MigrationManager};
pub use auto_migration::{AutoMigrationManager, MigrationStats, MigrationStrategy};

use std::collections::HashMap;
use std::path::PathBuf;

/// 数据分层配置
#[derive(Debug, Clone)]
pub struct TieredStorageConfig {
    /// 是否启用分层存储
    pub enabled: bool,
    /// 热数据层配置
    pub hot_tier: TierConfig,
    /// 温数据层配置
    pub warm_tier: TierConfig,
    /// 冷数据层配置
    pub cold_tier: TierConfig,
    /// 归档数据层配置
    pub archive_tier: TierConfig,
    /// 自动迁移间隔（秒）
    pub migration_interval_secs: u64,
    /// 并发迁移任务数
    pub migration_concurrency: usize,
}

impl Default for TieredStorageConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            hot_tier: TierConfig {
                name: "hot".to_string(),
                retention_hours: 24,
                max_size_gb: 10,
                compression_level: 1,
                path: PathBuf::from("data/hot"),
            },
            warm_tier: TierConfig {
                name: "warm".to_string(),
                retention_hours: 24 * 7, // 7天
                max_size_gb: 50,
                compression_level: 3,
                path: PathBuf::from("data/warm"),
            },
            cold_tier: TierConfig {
                name: "cold".to_string(),
                retention_hours: 24 * 30, // 30天
                max_size_gb: 200,
                compression_level: 6,
                path: PathBuf::from("data/cold"),
            },
            archive_tier: TierConfig {
                name: "archive".to_string(),
                retention_hours: 24 * 365, // 1年
                max_size_gb: 1000,
                compression_level: 9,
                path: PathBuf::from("data/archive"),
            },
            migration_interval_secs: 3600, // 1小时
            migration_concurrency: 4,
        }
    }
}

/// 分层存储统计信息
#[derive(Debug, Clone, Default)]
pub struct TieredStorageStats {
    pub total_series: u64,
    pub total_samples: u64,
    pub total_bytes: u64,
    pub tier_stats: HashMap<String, TierStats>,
    pub last_migration_time: Option<i64>,
    pub migration_count: u64,
}

/// 数据访问模式
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessPattern {
    /// 顺序访问
    Sequential,
    /// 随机访问
    Random,
    /// 范围扫描
    RangeScan,
}

impl Default for AccessPattern {
    fn default() -> Self {
        AccessPattern::Random
    }
}

/// 数据访问统计
#[derive(Debug, Clone, Default)]
pub struct AccessStats {
    pub read_count: u64,
    pub write_count: u64,
    pub last_access_time: i64,
    pub access_pattern: AccessPattern,
}

impl AccessStats {
    pub fn new() -> Self {
        Self {
            read_count: 0,
            write_count: 0,
            last_access_time: chrono::Utc::now().timestamp_millis(),
            access_pattern: AccessPattern::Random,
        }
    }

    pub fn record_read(&mut self) {
        self.read_count += 1;
        self.last_access_time = chrono::Utc::now().timestamp_millis();
    }

    pub fn record_write(&mut self) {
        self.write_count += 1;
        self.last_access_time = chrono::Utc::now().timestamp_millis();
    }
}

/// 数据位置信息
#[derive(Debug, Clone)]
pub struct DataLocation {
    pub tier: String,
    pub file_path: Option<PathBuf>,
    pub offset: u64,
    pub size: u64,
}

/// 分层存储查询选项
#[derive(Debug, Clone)]
pub struct TieredQueryOptions {
    /// 是否查询所有层
    pub query_all_tiers: bool,
    /// 优先查询的层
    pub preferred_tier: Option<String>,
    /// 最大查询时间（毫秒）
    pub timeout_ms: u64,
}

impl Default for TieredQueryOptions {
    fn default() -> Self {
        Self {
            query_all_tiers: true,
            preferred_tier: None,
            timeout_ms: 30000,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tiered_storage_config() {
        let config = TieredStorageConfig::default();
        assert!(config.enabled);
        assert_eq!(config.hot_tier.name, "hot");
        assert_eq!(config.warm_tier.name, "warm");
        assert_eq!(config.cold_tier.name, "cold");
        assert_eq!(config.archive_tier.name, "archive");
    }

    #[test]
    fn test_access_stats() {
        let mut stats = AccessStats::new();
        assert_eq!(stats.read_count, 0);
        assert_eq!(stats.write_count, 0);

        stats.record_read();
        assert_eq!(stats.read_count, 1);

        stats.record_write();
        assert_eq!(stats.write_count, 1);
    }
}
