use crate::columnstore::{Block, BlockMeta, BlockWriter, Column, ColumnBuilder, ColumnType};
use crate::error::{Error, Result};
use crate::flush::BlockManager;
use crate::model::{Label, Labels, Sample, TimeSeriesId};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::time::{interval, Duration};
use tracing::{debug, error, info, warn};

/// Compaction管理器
pub struct CompactionManager {
    data_dir: PathBuf,
    block_manager: Arc<std::sync::RwLock<BlockManager>>,
    config: CompactionConfig,
    tx: mpsc::Sender<CompactionCommand>,
}

enum CompactionCommand {
    Compact,
    CompactBlock(u64),
    Shutdown,
}

/// Compaction配置
#[derive(Debug, Clone)]
pub struct CompactionConfig {
    /// 是否启用自动compaction
    pub enabled: bool,
    /// Compaction检查间隔（秒）
    pub check_interval_secs: u64,
    /// Level 0到Level 1的阈值（块数）
    pub l0_threshold: usize,
    /// Level 1到Level 2的阈值（块数）
    pub l1_threshold: usize,
    /// Level 2到Level 3的阈值（块数）
    pub l2_threshold: usize,
    /// 最大compaction级别
    pub max_level: u8,
    /// 目标块大小（样本数）
    pub target_block_size: usize,
}

impl Default for CompactionConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            check_interval_secs: 600, // 10 minutes
            l0_threshold: 4,
            l1_threshold: 4,
            l2_threshold: 4,
            max_level: 4,
            target_block_size: 1_000_000,
        }
    }
}

impl CompactionManager {
    pub fn new<P: AsRef<Path>>(
        data_dir: P,
        block_manager: Arc<std::sync::RwLock<BlockManager>>,
        config: CompactionConfig,
    ) -> (Self, mpsc::Receiver<CompactionCommand>) {
        let (tx, rx) = mpsc::channel(100);

        let manager = Self {
            data_dir: data_dir.as_ref().to_path_buf(),
            block_manager,
            config,
            tx,
        };

        (manager, rx)
    }

    pub async fn run(&self, mut rx: mpsc::Receiver<CompactionCommand>) -> Result<()> {
        if !self.config.enabled {
            info!("Compaction is disabled");
            return Ok(());
        }

        let mut interval = interval(Duration::from_secs(self.config.check_interval_secs));

        info!(
            "Compaction manager started, check interval: {}s",
            self.config.check_interval_secs
        );

        loop {
            tokio::select! {
                _ = interval.tick() => {
                    if let Err(e) = self.check_and_compact().await {
                        error!("Scheduled compaction failed: {}", e);
                    }
                }
                Some(cmd) = rx.recv() => {
                    match cmd {
                        CompactionCommand::Compact => {
                            if let Err(e) = self.check_and_compact().await {
                                error!("Manual compaction failed: {}", e);
                            }
                        }
                        CompactionCommand::CompactBlock(block_id) => {
                            if let Err(e) = self.compact_specific_block(block_id).await {
                                error!("Compaction of block {} failed: {}", block_id, e);
                            }
                        }
                        CompactionCommand::Shutdown => {
                            info!("Compaction manager shutting down...");
                            break;
                        }
                    }
                }
            }
        }

        info!("Compaction manager stopped");
        Ok(())
    }

    async fn check_and_compact(&self) -> Result<()> {
        debug!("Checking for compaction opportunities...");

        // Check each level for compaction
        for level in 0..=self.config.max_level {
            let blocks_to_compact = self.get_blocks_for_level(level).await?;

            let threshold = match level {
                0 => self.config.l0_threshold,
                1 => self.config.l1_threshold,
                2 => self.config.l2_threshold,
                _ => 2, // Higher levels compact more aggressively
            };

            if blocks_to_compact.len() >= threshold {
                info!(
                    "Level {} has {} blocks, threshold is {}, starting compaction",
                    level,
                    blocks_to_compact.len(),
                    threshold
                );

                self.compact_blocks(level, &blocks_to_compact).await?;
            }
        }

        Ok(())
    }

    async fn get_blocks_for_level(&self, level: u8) -> Result<Vec<u64>> {
        let block_manager = self
            .block_manager
            .read()
            .map_err(|_| Error::Internal("Failed to acquire read lock".to_string()))?;

        let blocks: Vec<u64> = block_manager
            .all_blocks()
            .values()
            .filter(|b| b.compaction_level == level)
            .map(|b| b.block_id)
            .collect();

        Ok(blocks)
    }

    async fn compact_blocks(&self, level: u8, block_ids: &[u64]) -> Result<()> {
        let start_time = std::time::Instant::now();

        info!(
            "Starting compaction for level {} with {} blocks",
            level,
            block_ids.len()
        );

        // Load all blocks to be compacted
        let mut all_series: HashMap<TimeSeriesId, (Labels, Vec<Sample>)> = HashMap::new();

        for block_id in block_ids {
            self.load_block_data(*block_id, &mut all_series).await?;
        }

        if all_series.is_empty() {
            warn!("No data to compact in blocks {:?}", block_ids);
            return Ok(());
        }

        // Create new compacted block
        let new_block_id = self.generate_block_id();
        let new_level = level + 1;

        let block_dir = self.data_dir.join("blocks");
        std::fs::create_dir_all(&block_dir)?;

        let mut writer = BlockWriter::new(&block_dir, new_block_id, 3);

        // Merge and deduplicate samples for each series
        for (series_id, (labels, mut samples)) in all_series {
            // Sort by timestamp
            samples.sort_by_key(|s| s.timestamp);

            // Remove duplicates (same timestamp)
            samples.dedup_by(|a, b| a.timestamp == b.timestamp);

            writer.add_series(series_id, labels, samples);
        }

        // Write the compacted block
        let block = writer.write()?;

        // Update block manager
        {
            let mut block_manager = self
                .block_manager
                .write()
                .map_err(|_| Error::Internal("Failed to acquire write lock".to_string()))?;

            // Remove old blocks
            for block_id in block_ids {
                if let Err(e) = block_manager.remove_block(*block_id) {
                    warn!("Failed to remove old block {}: {}", block_id, e);
                }
            }

            // Add new block info
            let new_block_info = crate::flush::BlockInfo {
                block_id: new_block_id,
                min_timestamp: block.meta.min_timestamp,
                max_timestamp: block.meta.max_timestamp,
                series_count: block.meta.total_series,
                sample_count: block.meta.total_samples,
                compaction_level: new_level,
                path: block_dir.join(format!("block-{:020}", new_block_id)),
            };

            block_manager.add_block(new_block_info);
        }

        let duration = start_time.elapsed();
        info!(
            "Compaction completed in {:?}: level {} -> {}, new_block_id={}, series={}, samples={}",
            duration,
            level,
            new_level,
            new_block_id,
            block.meta.total_series,
            block.meta.total_samples
        );

        Ok(())
    }

    async fn load_block_data(
        &self,
        block_id: u64,
        all_series: &mut HashMap<TimeSeriesId, (Labels, Vec<Sample>)>,
    ) -> Result<()> {
        // In a real implementation, this would load the block from disk
        // and extract all series data

        // For now, this is a placeholder
        debug!("Loading block {} for compaction", block_id);

        Ok(())
    }

    async fn compact_specific_block(&self, block_id: u64) -> Result<()> {
        info!("Compacting specific block: {}", block_id);

        let block_manager = self
            .block_manager
            .read()
            .map_err(|_| Error::Internal("Failed to acquire read lock".to_string()))?;

        let block_info = block_manager
            .get_block(block_id)
            .ok_or_else(|| Error::NotFound(format!("Block {} not found", block_id)))?;

        let level = block_info.compaction_level;
        drop(block_manager);

        self.compact_blocks(level, &[block_id]).await
    }

    fn generate_block_id(&self) -> u64 {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64
    }

    pub fn trigger_compaction(&self) -> Result<()> {
        self.tx
            .try_send(CompactionCommand::Compact)
            .map_err(|e| Error::Internal(format!("Failed to send compact command: {}", e)))?;
        Ok(())
    }

    pub fn compact_block(&self, block_id: u64) -> Result<()> {
        self.tx
            .try_send(CompactionCommand::CompactBlock(block_id))
            .map_err(|e| Error::Internal(format!("Failed to send compact block command: {}", e)))?;
        Ok(())
    }

    pub fn shutdown(&self) -> Result<()> {
        self.tx
            .try_send(CompactionCommand::Shutdown)
            .map_err(|e| Error::Internal(format!("Failed to send shutdown command: {}", e)))?;
        Ok(())
    }
}

/// Compaction统计信息
#[derive(Debug, Clone, Default)]
pub struct CompactionStats {
    pub total_compactions: u64,
    pub bytes_compacted: u64,
    pub bytes_saved: u64,
    pub duration_secs: f64,
}

/// Compaction策略
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompactionStrategy {
    /// 基于大小的compaction
    SizeBased,
    /// 基于时间的compaction
    TimeBased,
    /// 基于级别的compaction（默认）
    LevelBased,
}

impl Default for CompactionStrategy {
    fn default() -> Self {
        CompactionStrategy::LevelBased
    }
}

/// Compaction优先级
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CompactionPriority {
    Low,
    Normal,
    High,
    Urgent,
}

impl Default for CompactionPriority {
    fn default() -> Self {
        CompactionPriority::Normal
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_compaction_config_default() {
        let config = CompactionConfig::default();
        assert!(config.enabled);
        assert_eq!(config.check_interval_secs, 600);
        assert_eq!(config.l0_threshold, 4);
        assert_eq!(config.max_level, 4);
    }

    #[test]
    fn test_compaction_strategy_default() {
        let strategy = CompactionStrategy::default();
        assert_eq!(strategy, CompactionStrategy::LevelBased);
    }

    #[test]
    fn test_compaction_priority_ordering() {
        assert!(CompactionPriority::Low < CompactionPriority::Normal);
        assert!(CompactionPriority::Normal < CompactionPriority::High);
        assert!(CompactionPriority::High < CompactionPriority::Urgent);
    }
}
