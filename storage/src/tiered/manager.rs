use crate::error::Result;
use crate::model::TimeSeries;
use crate::tiered::{TieredStorageConfig, TieredStorageStats, TieredQueryOptions, DataLocation};
use crate::tiered::tier::{DataTier, TierCollection, AccessPattern};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::debug;

pub struct TieredStorageManager {
    config: TieredStorageConfig,
    tiers: TierCollection,
    stats: Arc<RwLock<TieredStorageStats>>,
}

impl TieredStorageManager {
    pub fn new(config: TieredStorageConfig) -> Self {
        let mut tiers = TierCollection::new();
        
        if config.enabled {
            tiers.add_tier(Arc::new(DataTier::new(config.hot_tier.clone())));
            tiers.add_tier(Arc::new(DataTier::new(config.warm_tier.clone())));
            tiers.add_tier(Arc::new(DataTier::new(config.cold_tier.clone())));
            tiers.add_tier(Arc::new(DataTier::new(config.archive_tier.clone())));
        }
        
        Self {
            config,
            tiers,
            stats: Arc::new(RwLock::new(TieredStorageStats::default())),
        }
    }
    
    pub async fn write(&self, series: &TimeSeries) -> Result<DataLocation> {
        let now = chrono::Utc::now().timestamp_millis();
        let target_tier = self.tiers.get_tier_for_timestamp(now)
            .ok_or_else(|| crate::error::Error::Internal("No tier available".to_string()))?;
        
        let block_id = target_tier.write(series).await?;
        
        Ok(DataLocation {
            tier: target_tier.name().to_string(),
            file_path: Some(target_tier.config().path.join(format!("block_{}.bin", block_id))),
            offset: 0,
            size: 0,
        })
    }

    /// 清理所有层的过期数据
    pub async fn cleanup(&self) -> Result<()> {
        for tier in self.tiers.get_all_tiers() {
            tier.cleanup().await?;
        }
        Ok(())
    }

    /// 收集统计信息
    pub async fn collect_stats(&self) -> Result<TieredStorageStats> {
        let mut stats = TieredStorageStats::default();
        let mut tier_stats = std::collections::HashMap::new();
        
        for tier in self.tiers.get_all_tiers() {
            let tier_stat = tier.get_stats().await?;
            stats.total_series += tier_stat.series_count;
            stats.total_samples += tier_stat.sample_count;
            stats.total_bytes += tier_stat.total_bytes;
            tier_stats.insert(tier.name().to_string(), tier_stat);
        }
        
        stats.tier_stats = tier_stats;
        stats.last_migration_time = Some(chrono::Utc::now().timestamp_millis());
        
        // 更新全局统计
        let mut global_stats = self.stats.write().await;
        *global_stats = stats.clone();
        
        Ok(stats)
    }
    
    pub async fn query(&self, series_id: u64, start: i64, end: i64, options: TieredQueryOptions) -> Result<Vec<TimeSeries>> {
        let mut results = Vec::new();
        
        if options.query_all_tiers {
            // 并行查询所有层
            let tiers = self.tiers.get_all_tiers();
            let mut tasks = Vec::with_capacity(tiers.len());
            
            for tier in tiers {
                let series_id = series_id;
                let start = start;
                let end = end;
                
                tasks.push(tokio::spawn(async move {
                    match tier.query(series_id, start, end).await {
                        Ok(samples) if !samples.is_empty() => {
                            let mut series = TimeSeries::new(series_id, vec![]);
                            series.add_samples(samples);
                            Some((tier.name().to_string(), series))
                        },
                        _ => None
                    }
                }));
            }
            
            // 收集所有任务的结果
            for task in tasks {
                if let Ok(Some((tier_name, series))) = task.await {
                    let sample_count = series.samples.len();
                    results.push(series);
                    debug!("Found {} samples in tier {}", sample_count, tier_name);
                }
            }
        } else if let Some(preferred_tier) = &options.preferred_tier {
            // 只查询指定的层
            if let Some(tier) = self.tiers.get_tier(preferred_tier) {
                if let Ok(samples) = tier.query(series_id, start, end).await {
                    if !samples.is_empty() {
                        let mut series = TimeSeries::new(series_id, vec![]);
                        series.add_samples(samples);
                        results.push(series);
                    }
                }
            }
        }
        
        Ok(results)
    }

    /// 获取数据位置
    pub async fn get_data_location(&self, series_id: u64) -> Result<Option<DataLocation>> {
        // 简化实现，返回第一个包含数据的层
        for tier in self.tiers.get_all_tiers() {
            // 尝试查询数据，如果成功则返回位置
            if let Ok(samples) = tier.query(series_id, 0, i64::MAX).await {
                if !samples.is_empty() {
                    return Ok(Some(DataLocation {
                        tier: tier.name().to_string(),
                        file_path: None, // 简化实现，不返回具体文件路径
                        offset: 0,
                        size: 0,
                    }));
                }
            }
        }
        
        Ok(None)
    }
    
    pub async fn stats(&self) -> TieredStorageStats {
        self.stats.read().await.clone()
    }

    /// 执行自动数据迁移
    pub async fn auto_migrate(&self) -> Result<u64> {
        debug!("Starting automatic data migration");
        let mut tasks = Vec::new();
        
        // 遍历所有层
        for source_tier in self.tiers.get_all_tiers() {
            // 获取该层的所有访问模式
            let access_patterns = source_tier.get_all_access_patterns().await;
            
            // 分析每个系列的访问模式
            for (series_id, access_pattern) in access_patterns {
                // 确定目标层
                if let Some(target_tier) = self.determine_target_tier(series_id, &access_pattern).await {
                    if target_tier.name() != source_tier.name() {
                        // 创建迁移任务
                        let source_tier_clone = source_tier.clone();
                        let target_tier_clone = target_tier.clone();
                        let series_id_clone = series_id;
                        
                        tasks.push(tokio::spawn(async move {
                            Self::migrate_series_async(series_id_clone, source_tier_clone, target_tier_clone).await
                        }));
                    }
                }
            }
        }
        
        // 收集所有迁移任务的结果
        let mut migrated_count = 0;
        for task in tasks {
            if let Ok(Ok(_)) = task.await {
                migrated_count += 1;
            }
        }
        
        debug!("Automatic data migration completed, migrated {} series", migrated_count);
        Ok(migrated_count)
    }

    /// 异步迁移系列数据
    async fn migrate_series_async(series_id: u64, source_tier: Arc<DataTier>, target_tier: Arc<DataTier>) -> Result<()> {
        debug!("Migrating series {} from {} to {}", series_id, source_tier.name(), target_tier.name());
        
        // 从源层读取数据
        let start = 0;
        let end = chrono::Utc::now().timestamp_millis();
        let samples = source_tier.query(series_id, start, end).await?;
        
        if samples.is_empty() {
            return Ok(());
        }
        
        // 创建时间序列
        let series = TimeSeries::new(series_id, vec![]);
        let mut series_with_samples = series;
        series_with_samples.add_samples(samples);
        
        // 写入目标层
        target_tier.write(&series_with_samples).await?;
        
        // 从源层删除数据
        source_tier.delete(series_id).await?;
        
        debug!("Successfully migrated series {} from {} to {}", series_id, source_tier.name(), target_tier.name());
        Ok(())
    }

    /// 根据访问模式确定目标层
    async fn determine_target_tier(&self, series_id: u64, access_pattern: &AccessPattern) -> Option<Arc<DataTier>> {
        let now = chrono::Utc::now().timestamp_millis();
        
        // 基于访问频率和时间戳确定目标层
        let access_frequency = access_pattern.access_frequency;
        let age_hours = (now - access_pattern.last_access_time) as f64 / (3600.0 * 1000.0);
        
        // 访问频率高的数据放在热层
        if access_frequency > 10.0 { // 每小时访问10次以上
            return self.tiers.get_tier("hot");
        }
        // 访问频率中等的放在温层
        else if access_frequency > 1.0 { // 每小时访问1次以上
            return self.tiers.get_tier("warm");
        }
        // 访问频率低的放在冷层
        else if access_frequency > 0.1 { // 每10小时访问1次以上
            return self.tiers.get_tier("cold");
        }
        // 很少访问的放在归档层
        else {
            return self.tiers.get_tier("archive");
        }
    }

    /// 迁移系列数据
    async fn migrate_series(&self, series_id: u64, source_tier: Arc<DataTier>, target_tier: Arc<DataTier>) -> Result<()> {
        debug!("Migrating series {} from {} to {}", series_id, source_tier.name(), target_tier.name());
        
        // 从源层读取数据
        let start = 0;
        let end = chrono::Utc::now().timestamp_millis();
        let samples = source_tier.query(series_id, start, end).await?;
        
        if samples.is_empty() {
            return Ok(());
        }
        
        // 创建时间序列
        let series = TimeSeries::new(series_id, vec![]);
        let mut series_with_samples = series;
        series_with_samples.add_samples(samples);
        
        // 写入目标层
        target_tier.write(&series_with_samples).await?;
        
        // 从源层删除数据
        source_tier.delete(series_id).await?;
        
        debug!("Successfully migrated series {} from {} to {}", series_id, source_tier.name(), target_tier.name());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tiered::TieredStorageConfig;

    #[test]
    fn test_tiered_storage_manager_new_disabled() {
        let config = TieredStorageConfig {
            enabled: false,
            ..Default::default()
        };
        let manager = TieredStorageManager::new(config);
        assert!(manager.tiers.get_all_tiers().is_empty());
    }

    #[tokio::test]
    async fn test_tiered_storage_manager_stats() {
        let config = TieredStorageConfig {
            enabled: false,
            ..Default::default()
        };
        let manager = TieredStorageManager::new(config);
        let stats = manager.stats().await;
        assert_eq!(stats.total_series, 0);
        assert_eq!(stats.total_samples, 0);
        assert_eq!(stats.total_bytes, 0);
    }

    #[tokio::test]
    async fn test_get_data_location_no_data() {
        let config = TieredStorageConfig {
            enabled: false,
            ..Default::default()
        };
        let manager = TieredStorageManager::new(config);
        let location = manager.get_data_location(999).await.unwrap();
        assert!(location.is_none());
    }
}
