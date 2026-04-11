use crate::error::Result;
use crate::memstore::Chunk;
use crate::model::{Labels, Sample, TimeSeriesId};
use crate::index::InvertedIndex;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;

pub struct HeadBlock {
    series: RwLock<HashMap<TimeSeriesId, TimeSeriesEntry>>,
    index: Arc<InvertedIndex>,
    config: HeadConfig,
}

#[derive(Debug, Clone)]
pub struct HeadConfig {
    pub max_series: usize,
    pub max_samples_per_series: usize,
    pub chunk_capacity: usize,
}

impl Default for HeadConfig {
    fn default() -> Self {
        Self {
            max_series: 1_000_000,
            max_samples_per_series: 10000,
            chunk_capacity: 120,
        }
    }
}

struct TimeSeriesEntry {
    labels: Labels,
    current_chunk: Chunk,
    frozen_chunks: Vec<Chunk>,
    total_samples: usize,
}

impl TimeSeriesEntry {
    fn new(labels: Labels) -> Self {
        Self {
            labels,
            current_chunk: Chunk::new(),
            frozen_chunks: Vec::new(),
            total_samples: 0,
        }
    }

    fn add_sample(&mut self, sample: Sample) -> Result<()> {
        if self.current_chunk.is_full() {
            let frozen = std::mem::replace(&mut self.current_chunk, Chunk::new());
            self.frozen_chunks.push(frozen);
        }
        
        self.current_chunk.add(sample)?;
        self.total_samples += 1;
        Ok(())
    }

    fn add_samples(&mut self, samples: Vec<Sample>) -> Result<()> {
        for sample in samples {
            self.add_sample(sample)?;
        }
        Ok(())
    }

    fn samples_in_range(&self, start: i64, end: i64) -> Vec<Sample> {
        let mut samples = Vec::new();
        
        for chunk in &self.frozen_chunks {
            samples.extend(chunk.samples_in_range(start, end));
        }
        
        samples.extend(self.current_chunk.samples_in_range(start, end));
        samples
    }

    fn total_samples(&self) -> usize {
        self.total_samples
    }
}

impl HeadBlock {
    pub fn new(config: HeadConfig) -> Self {
        Self {
            series: RwLock::new(HashMap::new()),
            index: Arc::new(InvertedIndex::new()),
            config,
        }
    }

    pub fn add_series(&self, series_id: TimeSeriesId, labels: Labels) -> Result<()> {
        let mut series = self.series.write();
        
        if series.len() >= self.config.max_series {
            return Err(crate::error::Error::StorageFull);
        }
        
        if !series.contains_key(&series_id) {
            let entry = TimeSeriesEntry::new(labels.clone());
            series.insert(series_id, entry);
            self.index.add_series(series_id, &labels)?;
        }
        
        Ok(())
    }

    pub fn add_sample(&self, series_id: TimeSeriesId, sample: Sample) -> Result<()> {
        let mut series = self.series.write();
        
        if let Some(entry) = series.get_mut(&series_id) {
            entry.add_sample(sample)
        } else {
            Err(crate::error::Error::SeriesNotFound(series_id))
        }
    }

    pub fn add_samples(&self, series_id: TimeSeriesId, samples: Vec<Sample>) -> Result<()> {
        let mut series = self.series.write();
        
        if let Some(entry) = series.get_mut(&series_id) {
            entry.add_samples(samples)
        } else {
            Err(crate::error::Error::SeriesNotFound(series_id))
        }
    }

    pub fn get_or_create_series(&self, labels: Labels) -> Result<TimeSeriesId> {
        use crate::model::calculate_series_id;
        
        let series_id = calculate_series_id(&labels);
        
        {
            let series = self.series.read();
            if series.contains_key(&series_id) {
                return Ok(series_id);
            }
        }
        
        self.add_series(series_id, labels)?;
        Ok(series_id)
    }

    pub fn query(&self, series_id: TimeSeriesId, start: i64, end: i64) -> Option<Vec<Sample>> {
        let series = self.series.read();
        series.get(&series_id).map(|entry| entry.samples_in_range(start, end))
    }

    pub fn get_series_labels(&self, series_id: TimeSeriesId) -> Option<Labels> {
        let series = self.series.read();
        series.get(&series_id).map(|entry| entry.labels.clone())
    }

    pub fn series_count(&self) -> usize {
        let series = self.series.read();
        series.len()
    }

    pub fn total_samples(&self) -> usize {
        let series = self.series.read();
        series.values().map(|e| e.total_samples()).sum()
    }

    pub fn index(&self) -> Arc<InvertedIndex> {
        Arc::clone(&self.index)
    }

    // 获取所有系列 ID
    pub fn get_all_series_ids(&self) -> Vec<TimeSeriesId> {
        let series = self.series.read();
        series.keys().cloned().collect()
    }
}
