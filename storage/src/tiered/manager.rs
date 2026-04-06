use crate::error::Result;
use crate::model::TimeSeries;
use crate::tiered::{TieredStorageConfig, TieredStorageStats, TieredQueryOptions, DataLocation};
use crate::tiered::tier::{DataTier, TierCollection};
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
        
        target_tier.write(series).await?;
        
        Ok(DataLocation {
            tier: target_tier.name().to_string(),
            file_path: None,
            offset: 0,
            size: 0,
        })
    }
    
    pub async fn query(&self, series_id: u64, start: i64, end: i64, options: TieredQueryOptions) -> Result<Vec<TimeSeries>> {
        let results = vec![];
        
        if options.query_all_tiers {
            for tier in self.tiers.get_all_tiers() {
                if let Ok(samples) = tier.query(series_id, start, end).await {
                    if !samples.is_empty() {
                        // 这里应该构建完整的时间序列
                        debug!("Found {} samples in tier {}", samples.len(), tier.name());
                    }
                }
            }
        }
        
        Ok(results)
    }
    
    pub async fn stats(&self) -> TieredStorageStats {
        self.stats.read().await.clone()
    }
}
