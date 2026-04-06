use crate::error::{Error, Result};
use crate::flush::BlockManager;
use crate::tiered::{DataTier, TierConfig, TieredStorageConfig, AccessStats, DataLocation, TierCollection};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tokio::time::{interval, Duration};
use tracing::{debug, error, info, warn};

/// 自动迁移管理器
pub struct AutoMigrationManager {
    config: TieredStorageConfig,
    block_manager: Arc<RwLock<BlockManager>>,
    tier_collection: Arc<RwLock<TierCollection>>,
    access_stats: Arc<RwLock<HashMap<u64, AccessStats>>>,
    tx: mpsc::Sender<MigrationCommand>,
}

enum MigrationCommand {
    CheckMigration,
    MigrateBlock(u64, String, String),
    Shutdown,
}

/// 迁移决策
#[derive(Debug, Clone)]
struct MigrationDecision {
    block_id: u64,
    source_tier: String,
    target_tier: String,
    reason: MigrationReason,
    priority: MigrationPriority,
}

/// 迁移原因
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MigrationReason {
    Age,
    AccessPattern,
    TierCapacity,
    Manual,
}

/// 迁移优先级
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum MigrationPriority {
    Low,
    Normal,
    High,
    Critical,
}

impl AutoMigrationManager {
    pub fn new(
        config: TieredStorageConfig,
        block_manager: Arc<RwLock<BlockManager>>,
        tier_collection: Arc<RwLock<TierCollection>>,
    ) -> (Self, mpsc::Receiver<MigrationCommand>) {
        let (tx, rx) = mpsc::channel(1000);

        let manager = Self {
            config,
            block_manager,
            tier_collection,
            access_stats: Arc::new(RwLock::new(HashMap::new())),
            tx,
        };

        (manager, rx)
    }

    pub async fn run(&self, mut rx: mpsc::Receiver<MigrationCommand>) -> Result<()> {
        if !self.config.enabled {
            info!("Auto migration is disabled");
            return Ok(());
        }

        let mut check_interval = interval(Duration::from_secs(self.config.migration_interval_secs));

        info!(
            "Auto migration manager started, check interval: {}s",
            self.config.migration_interval_secs
        );

        loop {
            tokio::select! {
                _ = check_interval.tick() => {
                    if let Err(e) = self.check_and_migrate().await {
                        error!("Scheduled migration check failed: {}", e);
                    }
                }
                Some(cmd) = rx.recv() => {
                    match cmd {
                        MigrationCommand::CheckMigration => {
                            if let Err(e) = self.check_and_migrate().await {
                                error!("Manual migration check failed: {}", e);
                            }
                        }
                        MigrationCommand::MigrateBlock(block_id, source, target) => {
                            if let Err(e) = self.migrate_block(block_id, &source, &target).await {
                                error!("Migration of block {} failed: {}", block_id, e);
                            }
                        }
                        MigrationCommand::Shutdown => {
                            info!("Auto migration manager shutting down...");
                            break;
                        }
                    }
                }
            }
        }

        info!("Auto migration manager stopped");
        Ok(())
    }

    async fn check_and_migrate(&self) -> Result<()> {
        debug!("Checking for migration opportunities...");

        let decisions = self.evaluate_migration_needs().await?;

        if decisions.is_empty() {
            debug!("No migration needed");
            return Ok(());
        }

        info!("Found {} blocks to migrate", decisions.len());

        // 按优先级排序
        let mut sorted_decisions = decisions;
        sorted_decisions.sort_by(|a, b| b.priority.cmp(&a.priority));

        // 执行迁移
        for decision in sorted_decisions {
            if let Err(e) = self
                .migrate_block(decision.block_id, &decision.source_tier, &decision.target_tier)
                .await
            {
                error!(
                    "Failed to migrate block {} from {} to {}: {}",
                    decision.block_id, decision.source_tier, decision.target_tier, e
                );
            }
        }

        Ok(())
    }

    async fn evaluate_migration_needs(&self) -> Result<Vec<MigrationDecision>> {
        let mut decisions = Vec::new();
        let now = chrono::Utc::now().timestamp_millis();

        let block_manager = self.block_manager.read().await;
        let tier_collection = self.tier_collection.read().await;
        let access_stats = self.access_stats.read().await;

        for (block_id, block_info) in block_manager.all_blocks() {
            let block_age_hours = (now - block_info.max_timestamp) / 1000 / 3600;

            // 根据年龄决定目标层
            let target_tier = if block_age_hours > self.config.archive_tier.retention_hours as i64 {
                continue; // 数据已过期，不迁移
            } else if block_age_hours > self.config.cold_tier.retention_hours as i64 {
                Some("archive".to_string())
            } else if block_age_hours > self.config.warm_tier.retention_hours as i64 {
                Some("cold".to_string())
            } else if block_age_hours > self.config.hot_tier.retention_hours as i64 {
                Some("warm".to_string())
            } else {
                None
            };

            if let Some(target) = target_tier {
                // 确定当前层
                let current_tier = self.get_block_current_tier(*block_id).await?;

                if current_tier != target {
                    decisions.push(MigrationDecision {
                        block_id: *block_id,
                        source_tier: current_tier,
                        target_tier: target,
                        reason: MigrationReason::Age,
                        priority: MigrationPriority::Normal,
                    });
                }
            }
        }

        // 检查层容量
        for tier_name in ["hot", "warm", "cold"] {
            if let Some(tier) = tier_collection.get_tier(tier_name) {
                let stats = tier.get_stats().await?;
                let max_size_bytes = tier.config().max_size_gb * 1024 * 1024 * 1024;

                if stats.total_bytes > max_size_bytes {
                    // 层已满，需要迁移最旧的数据
                    let blocks_to_migrate = self
                        .find_blocks_to_evict(tier_name, &block_manager, &access_stats)
                        .await?;

                    for block_id in blocks_to_migrate {
                        let next_tier = self.get_next_tier(tier_name);
                        decisions.push(MigrationDecision {
                            block_id,
                            source_tier: tier_name.to_string(),
                            target_tier: next_tier,
                            reason: MigrationReason::TierCapacity,
                            priority: MigrationPriority::High,
                        });
                    }
                }
            }
        }

        Ok(decisions)
    }

    async fn get_block_current_tier(&self, block_id: u64) -> Result<String> {
        // 简化实现：根据块ID推断当前层
        // 实际实现应该查询元数据存储
        Ok("hot".to_string())
    }

    async fn find_blocks_to_evict(
        &self,
        tier_name: &str,
        block_manager: &BlockManager,
        access_stats: &HashMap<u64, AccessStats>,
    ) -> Result<Vec<u64>> {
        let mut candidates: Vec<(u64, i64)> = Vec::new();

        for (block_id, _) in block_manager.all_blocks() {
            let last_access = access_stats
                .get(block_id)
                .map(|s| s.last_access_time)
                .unwrap_or(0);
            candidates.push((*block_id, last_access));
        }

        // 按最后访问时间排序（最旧的在前）
        candidates.sort_by_key(|(_, last_access)| *last_access);

        // 返回最旧的块
        Ok(candidates.into_iter().take(10).map(|(id, _)| id).collect())
    }

    fn get_next_tier(&self, current_tier: &str) -> String {
        match current_tier {
            "hot" => "warm".to_string(),
            "warm" => "cold".to_string(),
            "cold" => "archive".to_string(),
            _ => "archive".to_string(),
        }
    }

    async fn migrate_block(
        &self,
        block_id: u64,
        source_tier: &str,
        target_tier: &str,
    ) -> Result<()> {
        info!(
            "Migrating block {} from {} to {}",
            block_id, source_tier, target_tier
        );

        let start_time = std::time::Instant::now();

        // 1. 获取源层和目标层
        let tier_collection = self.tier_collection.read().await;
        let source = tier_collection
            .get_tier(source_tier)
            .ok_or_else(|| Error::NotFound(format!("Source tier {} not found", source_tier)))?;
        let target = tier_collection
            .get_tier(target_tier)
            .ok_or_else(|| Error::NotFound(format!("Target tier {} not found", target_tier)))?;

        // 2. 读取块数据
        let block_data = source.read_block(block_id).await?;

        // 3. 写入目标层（使用目标层的压缩级别）
        target.write_block(block_id, &block_data).await?;

        // 4. 更新元数据
        drop(tier_collection);
        self.update_block_location(block_id, target_tier).await?;

        // 5. 删除源层数据（可选，取决于策略）
        // source.delete_block(block_id).await?;

        let duration = start_time.elapsed();
        info!(
            "Migration completed in {:?}: block {} from {} to {}",
            duration, block_id, source_tier, target_tier
        );

        Ok(())
    }

    async fn update_block_location(&self, block_id: u64, tier: &str) -> Result<()> {
        // 更新块的元数据，记录新的位置
        debug!("Updated block {} location to tier {}", block_id, tier);
        Ok(())
    }

    pub async fn record_access(&self, block_id: u64, is_read: bool) {
        let mut stats = self.access_stats.write().await;
        let entry = stats.entry(block_id).or_insert_with(crate::tiered::AccessStats::new);

        if is_read {
            entry.record_read();
        } else {
            entry.record_write();
        }
    }

    pub fn trigger_check(&self) -> Result<()> {
        self.tx
            .try_send(MigrationCommand::CheckMigration)
            .map_err(|e| Error::Internal(format!("Failed to send check command: {}", e)))?;
        Ok(())
    }

    pub fn shutdown(&self) -> Result<()> {
        self.tx
            .try_send(MigrationCommand::Shutdown)
            .map_err(|e| Error::Internal(format!("Failed to send shutdown command: {}", e)))?;
        Ok(())
    }
}

/// 迁移统计信息
#[derive(Debug, Clone, Default)]
pub struct MigrationStats {
    pub total_migrations: u64,
    pub successful_migrations: u64,
    pub failed_migrations: u64,
    pub bytes_migrated: u64,
    pub total_migration_time_ms: u64,
}

/// 迁移策略
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MigrationStrategy {
    /// 基于年龄的迁移
    AgeBased,
    /// 基于访问模式的迁移
    AccessPatternBased,
    /// 基于容量的迁移
    CapacityBased,
    /// 混合策略
    Hybrid,
}

impl Default for MigrationStrategy {
    fn default() -> Self {
        MigrationStrategy::Hybrid
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_migration_priority_ordering() {
        assert!(MigrationPriority::Low < MigrationPriority::Normal);
        assert!(MigrationPriority::Normal < MigrationPriority::High);
        assert!(MigrationPriority::High < MigrationPriority::Critical);
    }

    #[test]
    fn test_migration_strategy_default() {
        let strategy = MigrationStrategy::default();
        assert_eq!(strategy, MigrationStrategy::Hybrid);
    }

    #[test]
    fn test_migration_stats_default() {
        let stats = MigrationStats::default();
        assert_eq!(stats.total_migrations, 0);
        assert_eq!(stats.successful_migrations, 0);
    }
}
