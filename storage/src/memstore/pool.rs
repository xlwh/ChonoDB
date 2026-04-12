use crate::model::{Sample, TimeSeries, Labels, TimeSeriesId};
use std::sync::Arc;
use std::hash::{Hash, Hasher};
use std::collections::hash_map::DefaultHasher;
use dashmap::DashMap;

pub struct ObjectPool {
    sample_pool: DashMap<usize, Vec<Sample>>,
    series_pool: DashMap<usize, Vec<TimeSeries>>,
    max_samples_per_bucket: usize,
    max_series_per_bucket: usize,
}

impl ObjectPool {
    pub fn new() -> Self {
        Self {
            sample_pool: DashMap::new(),
            series_pool: DashMap::new(),
            max_samples_per_bucket: 1024,
            max_series_per_bucket: 256,
        }
    }

    pub fn acquire_sample(&self) -> Sample {
        if let Some(mut bucket) = self.sample_pool.get_mut(&0) {
            if let Some(sample) = bucket.pop() {
                return sample;
            }
        }
        Sample::new(0, 0.0)
    }

    pub fn release_sample(&self, mut sample: Sample) {
        sample.timestamp = 0;
        sample.value = 0.0;
        
        let mut bucket = self.sample_pool.entry(0).or_insert_with(Vec::new);
        if bucket.len() < self.max_samples_per_bucket {
            bucket.push(sample);
        }
    }

    pub fn acquire_series(&self, series_id: TimeSeriesId, labels: Labels) -> TimeSeries {
        let key = Self::series_key(&labels);
        if let Some(mut bucket) = self.series_pool.get_mut(&key) {
            if let Some(mut series) = bucket.pop() {
                series.id = series_id;
                series.labels = labels;
                series.samples.clear();
                return series;
            }
        }
        TimeSeries::new(series_id, labels)
    }

    pub fn release_series(&self, mut series: TimeSeries) {
        series.samples.clear();
        
        let key = Self::series_key(&series.labels);
        let mut bucket = self.series_pool.entry(key).or_insert_with(Vec::new);
        if bucket.len() < self.max_series_per_bucket {
            bucket.push(series);
        }
    }

    fn series_key(labels: &Labels) -> usize {
        let mut hasher = DefaultHasher::new();
        for label in labels.iter() {
            label.name.hash(&mut hasher);
            label.value.hash(&mut hasher);
        }
        hasher.finish() as usize
    }
}

impl Default for ObjectPool {
    fn default() -> Self {
        Self::new()
    }
}

pub struct PooledSample {
    pool: Arc<ObjectPool>,
    sample: Sample,
}

impl PooledSample {
    pub fn new(pool: Arc<ObjectPool>, timestamp: i64, value: f64) -> Self {
        let mut sample = pool.acquire_sample();
        sample.timestamp = timestamp;
        sample.value = value;
        Self { pool, sample }
    }

    pub fn get(&self) -> &Sample {
        &self.sample
    }

    pub fn get_mut(&mut self) -> &mut Sample {
        &mut self.sample
    }
}

impl Drop for PooledSample {
    fn drop(&mut self) {
        let sample = std::mem::replace(&mut self.sample, Sample { timestamp: 0, value: 0.0 });
        self.pool.release_sample(sample);
    }
}
