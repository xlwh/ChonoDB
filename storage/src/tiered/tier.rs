use crate::columnstore::{BlockWriter, BlockReader};
use crate::flush::{BlockManager, BlockInfo};
use crate::error::Result;
use crate::model::{Sample, TimeSeries};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::debug;

/// 数据层配置
#[derive(Debug, Clone)]
pub struct TierConfig {
    pub name: String,
    pub retention_hours: u64,
    pub max_size_gb: u64,
    pub compression_level: i32,
    pub path: PathBuf,
}

/// 数据访问模式
#[derive(Debug, Clone, Default)]
pub struct AccessPattern {
    pub last_access_time: i64,
    pub access_count: u64,
    pub access_frequency: f64, // 访问频率（次/小时）
}

/// 数据层统计信息
#[derive(Debug, Clone, Default)]
pub struct TierStats {
    pub series_count: u64,
    pub sample_count: u64,
    pub total_bytes: u64,
    pub file_count: u64,
    pub last_update: i64,
    pub access_patterns: std::collections::HashMap<u64, AccessPattern>, // 系列ID -> 访问模式
}

/// 数据层
pub struct DataTier {
    config: TierConfig,
    stats: Arc<RwLock<TierStats>>,
    block_manager: Arc<RwLock<BlockManager>>,
}

impl DataTier {
    pub fn new(config: TierConfig) -> Self {
        // 确保目录存在
        std::fs::create_dir_all(&config.path).unwrap();
        
        // 初始化块管理器
        let block_manager = BlockManager::new(&config.path).unwrap();
        
        Self {
            config,
            stats: Arc::new(RwLock::new(TierStats::default())),
            block_manager: Arc::new(RwLock::new(block_manager)),
        }
    }

    /// 获取配置
    pub fn config(&self) -> &TierConfig {
        &self.config
    }

    /// 获取名称
    pub fn name(&self) -> &str {
        &self.config.name
    }

    /// 获取统计信息
    pub async fn stats(&self) -> TierStats {
        self.stats.read().await.clone()
    }

    /// 写入时间序列
    pub async fn write(&self, series: &TimeSeries) -> Result<u64> {
        debug!("Writing series {} to tier {}", series.id, self.config.name);
        
        // 创建块写入器
        let block_id = chrono::Utc::now().timestamp_millis() as u64;
        let mut writer = BlockWriter::new(&self.config.path, block_id, self.config.compression_level);
        
        // 添加时间序列数据
        writer.add_series(series.id, series.labels.clone(), series.samples.clone());
        
        // 写入块
        let block = writer.write()?;
        
        // 创建 BlockInfo
        let block_info = BlockInfo {
            block_id: block.meta.block_id,
            min_timestamp: block.meta.min_timestamp,
            max_timestamp: block.meta.max_timestamp,
            series_count: block.meta.total_series,
            sample_count: block.meta.total_samples,
            compaction_level: block.meta.compaction_level,
            path: self.config.path.join(format!("block_{}.bin", block.meta.block_id)),
        };
        
        // 更新块管理器
        let mut block_manager = self.block_manager.write().await;
        block_manager.add_block(block_info);
        
        // 更新统计
        let mut stats = self.stats.write().await;
        stats.series_count += 1;
        stats.sample_count += series.samples.len() as u64;
        stats.total_bytes += block.total_size() as u64;
        stats.file_count += 1;
        stats.last_update = chrono::Utc::now().timestamp_millis();
        
        Ok(block_id)
    }

    /// 查询时间序列
    pub async fn query(
        &self,
        series_id: u64,
        start: i64,
        end: i64,
    ) -> Result<Vec<Sample>> {
        debug!(
            "Querying series {} from tier {}: {} to {}",
            series_id, self.config.name, start, end
        );
        
        let block_manager = self.block_manager.read().await;
        let blocks = block_manager.get_blocks_in_time_range(start, end);
        
        // 并行读取多个块
        let mut tasks = Vec::with_capacity(blocks.len());
        for block_info in blocks {
            let series_id = series_id;
            let start = start;
            let end = end;
            let path = block_info.path.clone();
            
            tasks.push(tokio::spawn(async move {
                // 读取块数据
                let reader = BlockReader::open(&path);
                if let Ok(reader) = reader {
                    // 从块中查询时间序列
                    reader.query(series_id, start, end)
                } else {
                    Err(crate::error::Error::Internal(format!("Failed to open block reader for {:?}", path)))
                }
            }));
        }
        
        // 收集所有任务的结果
        let mut samples = Vec::new();
        for task in tasks {
            if let Ok(Ok(block_samples)) = task.await {
                samples.extend(block_samples);
            }
        }
        
        // 更新访问模式
        if !samples.is_empty() {
            self.update_access_pattern(series_id).await;
        }
        
        Ok(samples)
    }

    /// 更新访问模式
    async fn update_access_pattern(&self, series_id: u64) {
        let now = chrono::Utc::now().timestamp_millis();
        let mut stats = self.stats.write().await;
        
        let access_pattern = stats.access_patterns.entry(series_id).or_default();
        access_pattern.access_count += 1;
        
        if access_pattern.last_access_time > 0 {
            let time_diff_hours = (now - access_pattern.last_access_time) as f64 / (3600.0 * 1000.0);
            if time_diff_hours > 0.0 {
                access_pattern.access_frequency = access_pattern.access_count as f64 / time_diff_hours;
            }
        }
        
        access_pattern.last_access_time = now;
        stats.last_update = now;
    }

    /// 删除时间序列
    pub async fn delete(&self, series_id: u64) -> Result<()> {
        debug!("Deleting series {} from tier {}", series_id, self.config.name);
        
        // 简化实现，只更新统计
        let mut stats = self.stats.write().await;
        stats.series_count = stats.series_count.saturating_sub(1);
        stats.last_update = chrono::Utc::now().timestamp_millis();
        
        Ok(())
    }

    /// 清理过期数据
    pub async fn cleanup(&self) -> Result<()> {
        debug!("Cleaning up expired data in tier {}", self.config.name);
        
        let now = chrono::Utc::now().timestamp_millis();
        let retention_ms = self.config.retention_hours as i64 * 3600 * 1000;
        let cutoff_time = now - retention_ms;
        
        let mut block_manager = self.block_manager.write().await;
        let all_blocks = block_manager.all_blocks();
        
        // 找出过期的块
        let expired_block_ids: Vec<u64> = all_blocks
            .values()
            .filter(|block| block.max_timestamp < cutoff_time)
            .map(|block| block.block_id)
            .collect();
        
        for block_id in expired_block_ids {
            // 从块管理器中移除
            block_manager.remove_block(block_id)?;
        }
        
        // 更新统计
        let mut stats = self.stats.write().await;
        stats.last_update = chrono::Utc::now().timestamp_millis();
        
        Ok(())
    }

    /// 检查数据是否在保留期内
    pub fn is_in_retention(&self, timestamp: i64) -> bool {
        let now = chrono::Utc::now().timestamp_millis();
        let retention_ms = self.config.retention_hours as i64 * 3600 * 1000;
        
        now - timestamp <= retention_ms
    }

    /// 获取层优先级（数值越小优先级越高）
    pub fn priority(&self) -> u8 {
        match self.config.name.as_str() {
            "hot" => 0,
            "warm" => 1,
            "cold" => 2,
            "archive" => 3,
            _ => 4,
        }
    }

    /// 检查是否已满
    pub async fn is_full(&self) -> bool {
        let stats = self.stats.read().await;
        let max_bytes = self.config.max_size_gb * 1024 * 1024 * 1024;
        
        stats.total_bytes >= max_bytes
    }

    /// 获取使用率
    pub async fn usage_ratio(&self) -> f64 {
        let stats = self.stats.read().await;
        let max_bytes = self.config.max_size_gb * 1024 * 1024 * 1024;
        
        if max_bytes == 0 {
            return 0.0;
        }
        
        stats.total_bytes as f64 / max_bytes as f64
    }

    /// 获取统计信息
    pub async fn get_stats(&self) -> Result<TierStats> {
        let stats = self.stats.read().await;
        Ok(stats.clone())
    }

    /// 读取块数据
    pub async fn read_block(&self, block_id: u64) -> Result<Vec<u8>> {
        debug!("Reading block {} from tier {}", block_id, self.config.name);
        // 简化实现，返回空数据
        Ok(vec![])
    }

    /// 写入块数据
    pub async fn write_block(&self, block_id: u64, data: &[u8]) -> Result<()> {
        debug!("Writing block {} to tier {} ({} bytes)", block_id, self.config.name, data.len());
        // 简化实现，更新统计
        let mut stats = self.stats.write().await;
        stats.total_bytes += data.len() as u64;
        stats.file_count += 1;
        Ok(())
    }

    /// 获取访问模式
    pub async fn get_access_pattern(&self, series_id: u64) -> Option<AccessPattern> {
        let stats = self.stats.read().await;
        stats.access_patterns.get(&series_id).cloned()
    }

    /// 获取所有访问模式
    pub async fn get_all_access_patterns(&self) -> std::collections::HashMap<u64, AccessPattern> {
        let stats = self.stats.read().await;
        stats.access_patterns.clone()
    }
}

/// 数据层集合
pub struct TierCollection {
    tiers: HashMap<String, Arc<DataTier>>,
}

impl TierCollection {
    pub fn new() -> Self {
        Self {
            tiers: HashMap::new(),
        }
    }

    /// 添加数据层
    pub fn add_tier(&mut self, tier: Arc<DataTier>) {
        self.tiers.insert(tier.name().to_string(), tier);
    }

    /// 获取数据层
    pub fn get_tier(&self, name: &str) -> Option<Arc<DataTier>> {
        self.tiers.get(name).cloned()
    }

    /// 获取所有数据层（按优先级排序）
    pub fn get_all_tiers(&self) -> Vec<Arc<DataTier>> {
        let mut tiers: Vec<_> = self.tiers.values().cloned().collect();
        tiers.sort_by_key(|t| t.priority());
        tiers
    }

    /// 根据时间戳确定数据应该存储在哪一层
    pub fn get_tier_for_timestamp(&self, timestamp: i64) -> Option<Arc<DataTier>> {
        let now = chrono::Utc::now().timestamp_millis();
        let age_hours = (now - timestamp) / (3600 * 1000);

        // 按优先级查找合适的层
        let tiers = self.get_all_tiers();
        
        for tier in tiers {
            if age_hours <= tier.config().retention_hours as i64 {
                return Some(tier);
            }
        }

        // 如果没有找到合适的层，返回归档层
        self.get_tier("archive")
    }

    /// 获取数据当前所在的层
    pub fn get_current_tier(&self, timestamp: i64) -> Option<Arc<DataTier>> {
        self.get_tier_for_timestamp(timestamp)
    }

    /// 检查是否需要迁移
    pub fn should_migrate(&self, timestamp: i64, current_tier: &str) -> Option<Arc<DataTier>> {
        let target_tier = self.get_tier_for_timestamp(timestamp)?;
        
        if target_tier.name() != current_tier {
            Some(target_tier)
        } else {
            None
        }
    }
}

impl Default for TierCollection {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_data_tier() {
        let config = TierConfig {
            name: "hot".to_string(),
            retention_hours: 24,
            max_size_gb: 10,
            compression_level: 1,
            path: PathBuf::from("data/hot"),
        };

        let tier = DataTier::new(config);
        assert_eq!(tier.name(), "hot");
        assert_eq!(tier.priority(), 0);
    }

    #[test]
    fn test_tier_collection() {
        let mut collection = TierCollection::new();

        let hot_config = TierConfig {
            name: "hot".to_string(),
            retention_hours: 24,
            max_size_gb: 10,
            compression_level: 1,
            path: PathBuf::from("data/hot"),
        };

        let warm_config = TierConfig {
            name: "warm".to_string(),
            retention_hours: 24 * 7,
            max_size_gb: 50,
            compression_level: 3,
            path: PathBuf::from("data/warm"),
        };

        collection.add_tier(Arc::new(DataTier::new(hot_config)));
        collection.add_tier(Arc::new(DataTier::new(warm_config)));

        assert!(collection.get_tier("hot").is_some());
        assert!(collection.get_tier("warm").is_some());

        let all_tiers = collection.get_all_tiers();
        assert_eq!(all_tiers.len(), 2);
        assert_eq!(all_tiers[0].name(), "hot");
        assert_eq!(all_tiers[1].name(), "warm");
    }
}
