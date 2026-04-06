use crate::columnstore::Column;
use crate::error::Result;
use crate::model::Timestamp;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub const BLOCK_MAGIC: u32 = 0x43_48_52_4F;
pub const BLOCK_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockMeta {
    pub magic: u32,
    pub version: u32,
    pub block_id: u64,
    pub min_timestamp: Timestamp,
    pub max_timestamp: Timestamp,
    pub total_series: u64,
    pub total_samples: u64,
    pub created_at: i64,
    pub compaction_level: u8,
    pub columns: Vec<String>,
    pub index_offset: u64,
    pub index_size: u64,
}

impl BlockMeta {
    pub fn new(block_id: u64) -> Self {
        Self {
            magic: BLOCK_MAGIC,
            version: BLOCK_VERSION,
            block_id,
            min_timestamp: i64::MAX,
            max_timestamp: i64::MIN,
            total_series: 0,
            total_samples: 0,
            created_at: chrono::Utc::now().timestamp_millis(),
            compaction_level: 0,
            columns: Vec::new(),
            index_offset: 0,
            index_size: 0,
        }
    }

    pub fn duration(&self) -> i64 {
        self.max_timestamp - self.min_timestamp
    }

    pub fn encode(&self) -> Result<Vec<u8>> {
        Ok(serde_json::to_vec(self)?)
    }

    pub fn decode(data: &[u8]) -> Result<Self> {
        Ok(serde_json::from_slice(data)?)
    }
}

#[derive(Debug, Clone)]
pub struct Block {
    pub meta: BlockMeta,
    pub columns: HashMap<String, Column>,
    pub index_data: Vec<u8>,
}

impl Block {
    pub fn new(block_id: u64) -> Self {
        Self {
            meta: BlockMeta::new(block_id),
            columns: HashMap::new(),
            index_data: Vec::new(),
        }
    }

    pub fn add_column(&mut self, name: String, column: Column) {
        self.meta.columns.push(name.clone());
        self.columns.insert(name, column);
    }

    pub fn get_column(&self, name: &str) -> Option<&Column> {
        self.columns.get(name)
    }

    pub fn total_size(&self) -> usize {
        self.columns.values().map(|c| c.data.len()).sum::<usize>() + self.index_data.len()
    }

    pub fn compression_ratio(&self) -> f64 {
        let total_uncompressed: usize = self.columns.values().map(|c| c.uncompressed_size).sum();
        let total_compressed: usize = self.columns.values().map(|c| c.compressed_size).sum();
        
        if total_compressed == 0 {
            return 0.0;
        }
        
        total_uncompressed as f64 / total_compressed as f64
    }
}

pub struct BlockBuilder {
    block_id: u64,
    min_timestamp: Timestamp,
    max_timestamp: Timestamp,
    total_series: u64,
    total_samples: u64,
    compression_level: i32,
}

impl BlockBuilder {
    pub fn new(block_id: u64, compression_level: i32) -> Self {
        Self {
            block_id,
            min_timestamp: i64::MAX,
            max_timestamp: i64::MIN,
            total_series: 0,
            total_samples: 0,
            compression_level,
        }
    }

    pub fn add_series(&mut self, timestamps: &[Timestamp], _values: &[f64]) {
        if let Some(&min) = timestamps.first() {
            self.min_timestamp = self.min_timestamp.min(min);
        }
        if let Some(&max) = timestamps.last() {
            self.max_timestamp = self.max_timestamp.max(max);
        }
        self.total_series += 1;
        self.total_samples += timestamps.len() as u64;
    }

    pub fn build(self, columns: HashMap<String, Column>, index_data: Vec<u8>) -> Block {
        let mut meta = BlockMeta::new(self.block_id);
        meta.min_timestamp = self.min_timestamp;
        meta.max_timestamp = self.max_timestamp;
        meta.total_series = self.total_series;
        meta.total_samples = self.total_samples;
        meta.columns = columns.keys().cloned().collect();
        meta.index_size = index_data.len() as u64;
        
        Block {
            meta,
            columns,
            index_data,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DownsampleLevel {
    L0,
    L1,
    L2,
    L3,
    L4,
}

impl DownsampleLevel {
    pub fn resolution_ms(&self) -> i64 {
        match self {
            DownsampleLevel::L0 => 10_000,
            DownsampleLevel::L1 => 60_000,
            DownsampleLevel::L2 => 300_000,
            DownsampleLevel::L3 => 3_600_000,
            DownsampleLevel::L4 => 86_400_000,
        }
    }

    pub fn retention_days(&self) -> u32 {
        match self {
            DownsampleLevel::L0 => 7,
            DownsampleLevel::L1 => 30,
            DownsampleLevel::L2 => 90,
            DownsampleLevel::L3 => 365,
            DownsampleLevel::L4 => 3650,
        }
    }

    pub fn from_query_range(range_ms: i64) -> Self {
        let range_hours = range_ms / 3_600_000;
        
        if range_hours < 1 {
            DownsampleLevel::L0
        } else if range_hours < 24 {
            DownsampleLevel::L1
        } else if range_hours < 168 {
            DownsampleLevel::L2
        } else if range_hours < 720 {
            DownsampleLevel::L3
        } else {
            DownsampleLevel::L4
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownsampleData {
    pub series_id: u64,
    pub level: DownsampleLevel,
    pub resolution_ms: i64,
    pub min_value: f64,
    pub max_value: f64,
    pub avg_value: f64,
    pub sum_value: f64,
    pub count: u64,
    pub last_value: f64,
    pub first_timestamp: i64,
    pub last_timestamp: i64,
}

impl DownsampleData {
    pub fn new(series_id: u64, level: DownsampleLevel) -> Self {
        Self {
            series_id,
            level,
            resolution_ms: level.resolution_ms(),
            min_value: f64::MAX,
            max_value: f64::MIN,
            avg_value: 0.0,
            sum_value: 0.0,
            count: 0,
            last_value: 0.0,
            first_timestamp: 0,
            last_timestamp: 0,
        }
    }

    pub fn add_sample(&mut self, timestamp: i64, value: f64) {
        if self.count == 0 {
            self.first_timestamp = timestamp;
            self.min_value = value;
            self.max_value = value;
        }
        
        self.min_value = self.min_value.min(value);
        self.max_value = self.max_value.max(value);
        self.sum_value += value;
        self.count += 1;
        self.avg_value = self.sum_value / self.count as f64;
        self.last_value = value;
        self.last_timestamp = timestamp;
    }

    pub fn encode(&self) -> Result<Vec<u8>> {
        Ok(serde_json::to_vec(self)?)
    }

    pub fn decode(data: &[u8]) -> Result<Self> {
        Ok(serde_json::from_slice(data)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_block_meta() {
        let meta = BlockMeta::new(1);
        let encoded = meta.encode().unwrap();
        let decoded = BlockMeta::decode(&encoded).unwrap();
        
        assert_eq!(meta.block_id, decoded.block_id);
        assert_eq!(meta.version, decoded.version);
    }

    #[test]
    fn test_downsample_level() {
        assert_eq!(DownsampleLevel::L0.resolution_ms(), 10_000);
        assert_eq!(DownsampleLevel::L1.resolution_ms(), 60_000);
        assert_eq!(DownsampleLevel::L4.resolution_ms(), 86_400_000);
        
        assert_eq!(DownsampleLevel::from_query_range(1800000), DownsampleLevel::L0);
        assert_eq!(DownsampleLevel::from_query_range(7200000), DownsampleLevel::L1);
        assert_eq!(DownsampleLevel::from_query_range(172800000), DownsampleLevel::L2);
    }

    #[test]
    fn test_downsample_data() {
        let mut data = DownsampleData::new(1, DownsampleLevel::L1);
        
        data.add_sample(1000, 10.0);
        data.add_sample(2000, 20.0);
        data.add_sample(3000, 30.0);
        
        assert_eq!(data.count, 3);
        assert_eq!(data.min_value, 10.0);
        assert_eq!(data.max_value, 30.0);
        assert_eq!(data.avg_value, 20.0);
    }
}
