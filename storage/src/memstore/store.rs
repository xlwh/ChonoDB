use crate::config::StorageConfig;
use crate::error::Result;
use crate::index::BloomFilter;
use crate::memstore::{HeadBlock, HeadConfig};
use crate::model::{Labels, Sample, TimeSeries, TimeSeriesId};
use crate::wal::Wal;
use crate::columnstore::DownsampleLevel;
use parking_lot::RwLock;
use std::path::Path;
use std::sync::Arc;

const DEFAULT_BLOOM_FILTER_CAPACITY: usize = 1000000;
const DEFAULT_BLOOM_FALSE_POSITIVE_RATE: f64 = 0.01;

pub struct MemStore {
    head: Arc<HeadBlock>,
    bloom: RwLock<BloomFilter>,
    wal: Option<Arc<Wal>>,
    config: StorageConfig,
    stats: RwLock<MemStoreStats>,
}

#[derive(Debug, Clone, Default)]
pub struct MemStoreStats {
    pub total_series: u64,
    pub total_samples: u64,
    pub total_bytes: u64,
    pub writes: u64,
    pub reads: u64,
}

impl MemStore {
    pub fn new(config: StorageConfig) -> Result<Self> {
        let head_config = HeadConfig {
            max_series: config.memstore_size / 1024,
            max_samples_per_series: 10000,
            chunk_capacity: 120,
        };
        
        let head = Arc::new(HeadBlock::new(head_config));
        let bloom = RwLock::new(BloomFilter::new(
            DEFAULT_BLOOM_FILTER_CAPACITY,
            DEFAULT_BLOOM_FALSE_POSITIVE_RATE,
        ));
        
        let wal = if Path::new(&config.data_dir).exists() {
            let wal_path = Path::new(&config.data_dir).join("wal");
            Some(Arc::new(Wal::new(wal_path)?))
        } else {
            None
        };
        
        Ok(Self {
            head,
            bloom,
            wal,
            config,
            stats: RwLock::new(MemStoreStats::default()),
        })
    }

    pub fn write(&self, labels: Labels, mut samples: Vec<Sample>) -> Result<()> {
        let series_id = self.head.get_or_create_series(labels.clone())?;

        if let Some(ref wal) = self.wal {
            wal.log_write(series_id, &labels, &samples)?;
        }

        samples.sort_by_key(|s| s.timestamp);

        for sample in samples {
            self.head.add_sample(series_id, sample)?;

            let mut stats = self.stats.write();
            stats.total_samples += 1;
            stats.writes += 1;
        }

        {
            let mut stats = self.stats.write();
            stats.total_series = self.head.series_count() as u64;
        }

        Ok(())
    }

    pub fn write_single(&self, labels: Labels, sample: Sample) -> Result<()> {
        self.write(labels, vec![sample])
    }

    pub fn query(
        &self,
        label_matchers: &[(String, String)],
        start: i64,
        end: i64,
    ) -> Result<Vec<TimeSeries>> {
        // 根据时间范围自动选择降采样级别
        let downsample_level = self.auto_select_downsample_level(start, end);
        self.query_with_downsample(label_matchers, start, end, downsample_level)
    }

    /// 根据时间范围自动选择降采样级别
    fn auto_select_downsample_level(&self, start: i64, end: i64) -> DownsampleLevel {
        let duration = end - start;
        
        // 时间范围（毫秒）
        const HOUR: i64 = 3600 * 1000;
        const DAY: i64 = 24 * HOUR;
        const WEEK: i64 = 7 * DAY;
        const MONTH: i64 = 30 * DAY;
        
        match duration {
            // 小于 1 小时：使用原始数据
            d if d < HOUR => DownsampleLevel::L0,
            // 1 小时到 24 小时：使用 L1（1 分钟降采样）
            d if d < DAY => DownsampleLevel::L1,
            // 24 小时到 7 天：使用 L2（10 分钟降采样）
            d if d < WEEK => DownsampleLevel::L2,
            // 7 天到 30 天：使用 L3（1 小时降采样）
            d if d < MONTH => DownsampleLevel::L3,
            // 大于 30 天：使用 L4（6 小时降采样）
            _ => DownsampleLevel::L4,
        }
    }

    pub fn query_with_downsample(
        &self,
        label_matchers: &[(String, String)],
        start: i64,
        end: i64,
        downsample_level: DownsampleLevel,
    ) -> Result<Vec<TimeSeries>> {
        let series_ids = self.find_series(label_matchers)?;
        
        // Use parallel processing for large number of series
        if series_ids.len() > 100 {
            use rayon::prelude::*;
            
            let results: Vec<_> = series_ids
                .into_par_iter()
                .filter_map(|series_id| {
                    let labels = self.head.get_series_labels(series_id)?;
                    let samples = self.head.query(series_id, start, end)?;
                    if samples.is_empty() {
                        return None;
                    }
                    
                    // Apply downsampling based on the level
                    let downsampled_samples = self.apply_downsampling(samples, downsample_level);
                    
                    if downsampled_samples.is_empty() {
                        return None;
                    }
                    
                    let mut ts = TimeSeries::new(series_id, labels);
                    ts.add_samples(downsampled_samples);
                    Some(ts)
                })
                .collect();
            
            {
                let mut stats = self.stats.write();
                stats.reads += 1;
            }
            
            Ok(results)
        } else {
            let mut result = Vec::with_capacity(series_ids.len());
            
            for series_id in series_ids {
                if let Some(labels) = self.head.get_series_labels(series_id) {
                    if let Some(samples) = self.head.query(series_id, start, end) {
                        if !samples.is_empty() {
                            // Apply downsampling based on the level
                            let downsampled_samples = self.apply_downsampling(samples, downsample_level);
                            
                            if !downsampled_samples.is_empty() {
                                let mut ts = TimeSeries::new(series_id, labels);
                                ts.add_samples(downsampled_samples);
                                result.push(ts);
                            }
                        }
                    }
                }
            }
            
            {
                let mut stats = self.stats.write();
                stats.reads += 1;
            }
            
            Ok(result)
        }
    }

    fn apply_downsampling(&self, samples: Vec<Sample>, level: DownsampleLevel) -> Vec<Sample> {
        if samples.is_empty() {
            return samples;
        }
        
        // For L0 (original data), return the samples as-is
        if level == DownsampleLevel::L0 {
            return samples;
        }
        
        let resolution = level.resolution_ms();
        if resolution == 0 {
            return samples;
        }
        
        // Pre-allocate with estimated capacity to reduce reallocations
        let estimated_windows = samples.len() / 10 + 1;
        let mut downsampled = Vec::with_capacity(estimated_windows);
        
        let mut current_window = samples[0].timestamp - (samples[0].timestamp % resolution);
        let mut window_sum = 0.0;
        let mut window_count = 0;
        
        for sample in samples {
            let sample_window = sample.timestamp - (sample.timestamp % resolution);
            
            if sample_window != current_window {
                // Process the current window
                if window_count > 0 {
                    let avg = window_sum / window_count as f64;
                    downsampled.push(Sample::new(current_window, avg));
                }
                
                // Start a new window
                current_window = sample_window;
                window_sum = 0.0;
                window_count = 0;
            }
            
            window_sum += sample.value;
            window_count += 1;
        }
        
        // Process the last window
        if window_count > 0 {
            let avg = window_sum / window_count as f64;
            downsampled.push(Sample::new(current_window, avg));
        }
        
        downsampled
    }

    pub fn find_series(&self, matchers: &[(String, String)]) -> Result<Vec<TimeSeriesId>> {
        if matchers.is_empty() {
            return Ok(self.all_series_ids());
        }
        
        let index = self.head.index();
        let mut result: Option<Vec<TimeSeriesId>> = None;
        
        for (name, value) in matchers {
            let series = index.lookup(name, value);
            
            result = Some(match result {
                None => series,
                Some(prev) => {
                    let set1: std::collections::HashSet<_> = prev.into_iter().collect();
                    let set2: std::collections::HashSet<_> = series.into_iter().collect();
                    set1.intersection(&set2).copied().collect()
                }
            });
        }
        
        Ok(result.unwrap_or_default())
    }

    fn all_series_ids(&self) -> Vec<TimeSeriesId> {
        let index = self.head.index();
        index.all_series_ids()
    }

    pub fn get_series(&self, series_id: TimeSeriesId) -> Option<TimeSeries> {
        let labels = self.head.get_series_labels(series_id)?;
        let samples = self.head.query(series_id, i64::MIN, i64::MAX)?;
        
        let mut ts = TimeSeries::new(series_id, labels);
        ts.add_samples(samples);
        Some(ts)
    }

    pub fn label_names(&self) -> Vec<String> {
        self.head.index().label_names()
    }

    pub fn label_values(&self, name: &str) -> Vec<String> {
        self.head.index().label_values(name)
    }

    pub fn series_count(&self) -> usize {
        self.head.series_count()
    }

    pub fn total_samples(&self) -> usize {
        self.head.total_samples()
    }

    pub fn stats(&self) -> MemStoreStats {
        self.stats.read().clone()
    }

    pub fn flush(&self) -> Result<()> {
        if let Some(ref wal) = self.wal {
            wal.sync()?;
        }
        Ok(())
    }

    pub fn close(&self) -> Result<()> {
        self.flush()
    }

    /// 获取所有时间序列ID
    pub fn get_all_series_ids(&self) -> Vec<TimeSeriesId> {
        self.all_series_ids()
    }

    /// 获取时间序列的标签
    pub fn get_series_labels(&self, series_id: TimeSeriesId) -> Option<Labels> {
        self.head.get_series_labels(series_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Label;
    use tempfile::tempdir;

    fn create_test_config() -> StorageConfig {
        let temp_dir = tempdir().unwrap();
        StorageConfig {
            data_dir: temp_dir.path().to_string_lossy().to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn test_memstore_write_query() {
        let config = create_test_config();
        let store = MemStore::new(config).unwrap();
        
        let labels = vec![
            Label::new("__name__", "http_requests_total"),
            Label::new("job", "prometheus"),
            Label::new("instance", "localhost:9090"),
        ];
        
        let samples = vec![
            Sample::new(1000, 100.0),
            Sample::new(2000, 200.0),
            Sample::new(3000, 300.0),
        ];
        
        store.write(labels.clone(), samples).unwrap();
        
        let result = store.query(
            &[("job".to_string(), "prometheus".to_string())],
            0,
            4000,
        ).unwrap();
        
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].samples.len(), 3);
    }

    #[test]
    fn test_memstore_multiple_series() {
        let config = create_test_config();
        let store = MemStore::new(config).unwrap();
        
        for i in 0..10 {
            let labels = vec![
                Label::new("__name__", "test_metric"),
                Label::new("instance", format!("localhost:{}", 9000 + i)),
            ];
            
            store.write_single(labels, Sample::new(1000, i as f64)).unwrap();
        }
        
        assert_eq!(store.series_count(), 10);
        assert_eq!(store.total_samples(), 10);
    }

    #[test]
    fn test_memstore_label_queries() {
        let config = create_test_config();
        let store = MemStore::new(config).unwrap();
        
        let labels = vec![
            Label::new("__name__", "test_metric"),
            Label::new("job", "test"),
            Label::new("env", "prod"),
        ];
        
        store.write_single(labels, Sample::new(1000, 1.0)).unwrap();
        
        let names = store.label_names();
        assert!(names.contains(&"job".to_string()));
        assert!(names.contains(&"env".to_string()));
        
        let values = store.label_values("job");
        assert!(values.contains(&"test".to_string()));
    }
}
