use crate::columnstore::BlockWriter;
use crate::error::{Error, Result};
use crate::memstore::MemStore;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::time::{interval, Duration};
use tracing::{debug, error, info, warn};

#[derive(Clone)]
pub struct FlushManager {
    data_dir: PathBuf,
    block_size_threshold: usize,
    flush_interval_secs: u64,
    tx: mpsc::Sender<FlushCommand>,
}

#[derive(Clone, Debug)]
pub enum FlushCommand {
    Flush,
    Shutdown,
}

impl FlushManager {
    pub fn new<P: AsRef<Path>>(
        data_dir: P,
        block_size_threshold: usize,
        flush_interval_secs: u64,
    ) -> (Self, mpsc::Receiver<FlushCommand>) {
        let (tx, rx) = mpsc::channel(100);
        
        let manager = Self {
            data_dir: data_dir.as_ref().to_path_buf(),
            block_size_threshold,
            flush_interval_secs,
            tx,
        };
        
        (manager, rx)
    }

    pub async fn run(
        &self,
        mut rx: mpsc::Receiver<FlushCommand>,
        memstore: Arc<MemStore>,
    ) -> Result<()> {
        let mut interval = interval(Duration::from_secs(self.flush_interval_secs));
        
        info!("Flush manager started, interval: {}s", self.flush_interval_secs);
        
        loop {
            tokio::select! {
                _ = interval.tick() => {
                    if let Err(e) = self.check_and_flush(&memstore).await {
                        error!("Scheduled flush failed: {}", e);
                    }
                }
                Some(cmd) = rx.recv() => {
                    match cmd {
                        FlushCommand::Flush => {
                            if let Err(e) = self.check_and_flush(&memstore).await {
                                error!("Manual flush failed: {}", e);
                            }
                        }
                        FlushCommand::Shutdown => {
                            info!("Flush manager shutting down...");
                            // Final flush before shutdown
                            if let Err(e) = self.check_and_flush(&memstore).await {
                                error!("Final flush failed: {}", e);
                            }
                            break;
                        }
                    }
                }
            }
        }
        
        info!("Flush manager stopped");
        Ok(())
    }

    async fn check_and_flush(&self, memstore: &Arc<MemStore>) -> Result<()> {
        let stats = memstore.stats();
        
        if stats.total_samples < self.block_size_threshold as u64 {
            debug!(
                "Skipping flush: {} samples < threshold {}",
                stats.total_samples, self.block_size_threshold
            );
            return Ok(());
        }
        
        info!("Starting flush: {} samples, {} series", stats.total_samples, stats.total_series);
        
        let start_time = std::time::Instant::now();
        
        // Perform the flush
        let flushed_block = self.flush_memstore(memstore).await?;
        
        let duration = start_time.elapsed();
        info!(
            "Flush completed in {:?}: block_id={}, series={}, samples={}",
            duration,
            flushed_block.block_id,
            flushed_block.series_count,
            flushed_block.sample_count
        );
        
        Ok(())
    }

    pub async fn flush_memstore(&self, memstore: &Arc<MemStore>) -> Result<FlushedBlockInfo> {
        let block_id = self.generate_block_id();
        let blocks_dir = self.data_dir.join("blocks");

        std::fs::create_dir_all(&blocks_dir)?;

        let mut writer = BlockWriter::new(&blocks_dir, block_id, 3);

        let series_ids = memstore.get_all_series_ids();
        let mut total_series = 0u64;
        let mut total_samples = 0u64;
        let mut min_ts = i64::MAX;
        let mut max_ts = i64::MIN;

        for series_id in &series_ids {
            if let Some(ts) = memstore.get_series(*series_id) {
                let labels = ts.labels.clone();
                let samples = ts.samples.clone();

                if !samples.is_empty() {
                    for sample in &samples {
                        min_ts = min_ts.min(sample.timestamp);
                        max_ts = max_ts.max(sample.timestamp);
                    }

                    total_samples += samples.len() as u64;
                    writer.add_series(*series_id, labels, samples);
                    total_series += 1;
                }
            }
        }

        if total_series == 0 {
            std::fs::remove_dir_all(&blocks_dir)?;
            return Ok(FlushedBlockInfo {
                block_id,
                series_count: 0,
                sample_count: 0,
                min_timestamp: 0,
                max_timestamp: 0,
            });
        }

        let block = writer.write()?;

        memstore.flush()?;

        memstore.remove_series_batch(&series_ids)?;

        info!(
            "Flushed block {}: {} series, {} samples, time range [{}, {}]",
            block_id, total_series, total_samples, min_ts, max_ts
        );

        Ok(FlushedBlockInfo {
            block_id,
            series_count: total_series,
            sample_count: total_samples,
            min_timestamp: if min_ts == i64::MAX { block.meta.min_timestamp } else { min_ts },
            max_timestamp: if max_ts == i64::MIN { block.meta.max_timestamp } else { max_ts },
        })
    }

    fn generate_block_id(&self) -> u64 {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64
    }

    pub fn trigger_flush(&self) -> Result<()> {
        self.tx
            .try_send(FlushCommand::Flush)
            .map_err(|e| Error::Internal(format!("Failed to send flush command: {}", e)))?;
        Ok(())
    }

    pub fn shutdown(&self) -> Result<()> {
        self.tx
            .try_send(FlushCommand::Shutdown)
            .map_err(|e| Error::Internal(format!("Failed to send shutdown command: {}", e)))?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct FlushedBlockInfo {
    pub block_id: u64,
    pub series_count: u64,
    pub sample_count: u64,
    pub min_timestamp: i64,
    pub max_timestamp: i64,
}

/// 刷盘配置
#[derive(Debug, Clone)]
pub struct FlushConfig {
    /// 数据目录
    pub data_dir: String,
    /// 块大小阈值（样本数）
    pub block_size_threshold: usize,
    /// 刷盘间隔（秒）
    pub flush_interval_secs: u64,
    /// 是否启用自动刷盘
    pub auto_flush: bool,
}

impl Default for FlushConfig {
    fn default() -> Self {
        Self {
            data_dir: "/var/lib/chronodb/data".to_string(),
            block_size_threshold: 100_000,
            flush_interval_secs: 300, // 5 minutes
            auto_flush: true,
        }
    }
}

/// 块管理器 - 管理所有持久化的块
pub struct BlockManager {
    data_dir: PathBuf,
    blocks: HashMap<u64, BlockInfo>,
}

#[derive(Debug, Clone)]
pub struct BlockInfo {
    pub block_id: u64,
    pub min_timestamp: i64,
    pub max_timestamp: i64,
    pub series_count: u64,
    pub sample_count: u64,
    pub compaction_level: u8,
    pub path: PathBuf,
}

impl BlockManager {
    pub fn new<P: AsRef<Path>>(data_dir: P) -> Result<Self> {
        let data_dir = data_dir.as_ref().to_path_buf();
        std::fs::create_dir_all(&data_dir)?;
        
        let mut manager = Self {
            data_dir,
            blocks: HashMap::new(),
        };
        
        // Load existing blocks
        manager.load_existing_blocks()?;
        
        Ok(manager)
    }

    fn load_existing_blocks(&mut self) -> Result<()> {
        let blocks_dir = self.data_dir.join("blocks");
        if !blocks_dir.exists() {
            return Ok(());
        }

        for entry in std::fs::read_dir(&blocks_dir)? {
            let entry = entry?;
            let path = entry.path();
            
            if path.is_dir() {
                // Try to load block metadata
                let meta_path = path.join("meta.json");
                if meta_path.exists() {
                    match self.load_block_info(&path) {
                        Ok(info) => {
                            info!("Loaded block: id={}, samples={}", info.block_id, info.sample_count);
                            self.blocks.insert(info.block_id, info);
                        }
                        Err(e) => {
                            warn!("Failed to load block at {:?}: {}", path, e);
                        }
                    }
                }
            }
        }

        info!("Loaded {} existing blocks", self.blocks.len());
        Ok(())
    }

    fn load_block_info(&self, block_path: &Path) -> Result<BlockInfo> {
        let meta_path = block_path.join("meta.json");
        let meta_content = std::fs::read_to_string(&meta_path)?;
        let meta: crate::columnstore::BlockMeta = serde_json::from_str(&meta_content)?;
        
        let block_id = meta.block_id;
        
        Ok(BlockInfo {
            block_id,
            min_timestamp: meta.min_timestamp,
            max_timestamp: meta.max_timestamp,
            series_count: meta.total_series,
            sample_count: meta.total_samples,
            compaction_level: meta.compaction_level,
            path: block_path.to_path_buf(),
        })
    }

    pub fn add_block(&mut self, info: BlockInfo) {
        self.blocks.insert(info.block_id, info);
    }

    pub fn get_block(&self, block_id: u64) -> Option<&BlockInfo> {
        self.blocks.get(&block_id)
    }

    pub fn get_blocks_in_time_range(&self, start: i64, end: i64) -> Vec<&BlockInfo> {
        self.blocks
            .values()
            .filter(|b| b.max_timestamp >= start && b.min_timestamp <= end)
            .collect()
    }

    pub fn get_blocks_for_compaction(&self, max_level: u8) -> Vec<&BlockInfo> {
        self.blocks
            .values()
            .filter(|b| b.compaction_level <= max_level)
            .collect()
    }

    pub fn all_blocks(&self) -> &HashMap<u64, BlockInfo> {
        &self.blocks
    }

    pub fn remove_block(&mut self, block_id: u64) -> Result<()> {
        if let Some(info) = self.blocks.remove(&block_id) {
            if info.path.exists() {
                std::fs::remove_dir_all(&info.path)?;
                info!("Removed block {} at {:?}", block_id, info.path);
            }
        }
        Ok(())
    }

    pub fn total_blocks(&self) -> usize {
        self.blocks.len()
    }

    pub fn total_samples(&self) -> u64 {
        self.blocks.values().map(|b| b.sample_count).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_flush_config_default() {
        let config = FlushConfig::default();
        assert_eq!(config.block_size_threshold, 100_000);
        assert_eq!(config.flush_interval_secs, 300);
        assert!(config.auto_flush);
    }

    #[test]
    fn test_block_manager() {
        let temp_dir = tempdir().unwrap();
        let manager = BlockManager::new(temp_dir.path()).unwrap();
        
        assert_eq!(manager.total_blocks(), 0);
        assert_eq!(manager.total_samples(), 0);
    }

    #[test]
    fn test_block_manager_add_block() {
        let temp_dir = tempdir().unwrap();
        let mut manager = BlockManager::new(temp_dir.path()).unwrap();

        let block_info = BlockInfo {
            block_id: 1,
            min_timestamp: 1000,
            max_timestamp: 2000,
            series_count: 10,
            sample_count: 100,
            compaction_level: 0,
            path: temp_dir.path().join("blocks/1"),
        };

        manager.add_block(block_info);
        assert_eq!(manager.total_blocks(), 1);
        assert_eq!(manager.total_samples(), 100);
    }

    #[test]
    fn test_block_manager_get_block() {
        let temp_dir = tempdir().unwrap();
        let mut manager = BlockManager::new(temp_dir.path()).unwrap();

        let block_info = BlockInfo {
            block_id: 42,
            min_timestamp: 1000,
            max_timestamp: 2000,
            series_count: 5,
            sample_count: 50,
            compaction_level: 0,
            path: temp_dir.path().join("blocks/42"),
        };

        manager.add_block(block_info);
        let found = manager.get_block(42);
        assert!(found.is_some());
        assert_eq!(found.unwrap().series_count, 5);

        let not_found = manager.get_block(99);
        assert!(not_found.is_none());
    }

    #[test]
    fn test_block_manager_get_blocks_in_time_range() {
        let temp_dir = tempdir().unwrap();
        let mut manager = BlockManager::new(temp_dir.path()).unwrap();

        manager.add_block(BlockInfo {
            block_id: 1,
            min_timestamp: 1000,
            max_timestamp: 2000,
            series_count: 5,
            sample_count: 50,
            compaction_level: 0,
            path: temp_dir.path().join("blocks/1"),
        });

        manager.add_block(BlockInfo {
            block_id: 2,
            min_timestamp: 5000,
            max_timestamp: 6000,
            series_count: 3,
            sample_count: 30,
            compaction_level: 0,
            path: temp_dir.path().join("blocks/2"),
        });

        let blocks = manager.get_blocks_in_time_range(1500, 5500);
        assert_eq!(blocks.len(), 2);

        let blocks = manager.get_blocks_in_time_range(3000, 4000);
        assert_eq!(blocks.len(), 0);
    }

    #[test]
    fn test_block_manager_get_blocks_for_compaction() {
        let temp_dir = tempdir().unwrap();
        let mut manager = BlockManager::new(temp_dir.path()).unwrap();

        manager.add_block(BlockInfo {
            block_id: 1,
            min_timestamp: 0,
            max_timestamp: 0,
            series_count: 0,
            sample_count: 0,
            compaction_level: 0,
            path: temp_dir.path().join("blocks/1"),
        });

        manager.add_block(BlockInfo {
            block_id: 2,
            min_timestamp: 0,
            max_timestamp: 0,
            series_count: 0,
            sample_count: 0,
            compaction_level: 2,
            path: temp_dir.path().join("blocks/2"),
        });

        let blocks = manager.get_blocks_for_compaction(1);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].block_id, 1);
    }

    #[test]
    fn test_flushed_block_info() {
        let info = FlushedBlockInfo {
            block_id: 123,
            series_count: 10,
            sample_count: 100,
            min_timestamp: 1000,
            max_timestamp: 2000,
        };
        assert_eq!(info.block_id, 123);
        assert_eq!(info.series_count, 10);
    }

    #[test]
    fn test_flush_manager_new() {
        let temp_dir = tempdir().unwrap();
        let (manager, _rx) = FlushManager::new(temp_dir.path(), 1000, 60);
        assert!(manager.trigger_flush().is_ok());
    }

    #[test]
    fn test_flush_manager_shutdown() {
        let temp_dir = tempdir().unwrap();
        let (manager, _rx) = FlushManager::new(temp_dir.path(), 1000, 60);
        assert!(manager.shutdown().is_ok());
    }
}
