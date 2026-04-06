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

    pub fn write(&self, labels: Labels, samples: Vec<Sample>) -> Result<()> {
        let series_id = self.head.get_or_create_series(labels.clone())?;
        
        if let Some(ref wal) = self.wal {
            wal.log_write(series_id, &labels, &samples)?;
        }
        
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
        self.query_with_downsample(label_matchers, start, end, DownsampleLevel::L0)
    }

    pub fn query_with_downsample(
        &self,
        label_matchers: &[(String, String)],
        start: i64,
        end: i64,
        downsample_level: DownsampleLevel,
    ) -> Result<Vec<TimeSeries>> {
        let series_ids = self.find_series(label_matchers)?;
        
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

    fn apply_downsampling(&self, samples: Vec<Sample>, level: DownsampleLevel) -> Vec<Sample> {
        if samples.is_empty() {
            return samples;
        }
        
        let resolution = level.resolution_ms();
        if resolution == 0 {
            return samples;
        }
        
        let mut downsampled = Vec::new();
        let mut current_window = samples[0].timestamp - (samples[0].timestamp % resolution);
        let mut window_samples = Vec::new();
        
        for sample in samples {
            let sample_window = sample.timestamp - (sample.timestamp % resolution);
            
            if sample_window != current_window {
                // Process the current window
                if !window_samples.is_empty() {
                    let window_samples_clone = window_samples.clone();
                    let downsampled_sample = self.compute_downsample(window_samples_clone, current_window);
                    downsampled.push(downsampled_sample);
                }
                
                // Start a new window
                current_window = sample_window;
                window_samples.clear();
            }
            
            window_samples.push(sample);
        }
        
        // Process the last window
        if !window_samples.is_empty() {
            let downsampled_sample = self.compute_downsample(window_samples, current_window);
            downsampled.push(downsampled_sample);
        }
        
        downsampled
    }

    fn compute_downsample(&self, samples: Vec<Sample>, window_timestamp: i64) -> Sample {
        if samples.is_empty() {
            return Sample::new(window_timestamp, 0.0);
        }
        
        let sum: f64 = samples.iter().map(|s| s.value).sum();
        let avg = sum / samples.len() as f64;
        
        Sample::new(window_timestamp, avg)
    }

    fn find_series(&self, matchers: &[(String, String)]) -> Result<Vec<TimeSeriesId>> {
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
        index.label_values("__name__")
            .into_iter()
            .flat_map(|v| index.lookup("__name__", &v))
            .collect()
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
