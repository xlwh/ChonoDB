use crate::columnstore::{Block, BlockMeta, ColumnBuilder, BLOCK_MAGIC, BLOCK_VERSION};
use crate::error::Result;
use crate::index::BloomFilter;
use crate::model::{Labels, Sample, TimeSeriesId};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

pub struct BlockWriter {
    path: PathBuf,
    block_id: u64,
    compression_level: i32,
    series_data: HashMap<TimeSeriesId, SeriesData>,
    bloom_filter: BloomFilter,
}

struct SeriesData {
    labels: Labels,
    timestamps: Vec<i64>,
    values: Vec<f64>,
}

impl BlockWriter {
    pub fn new<P: AsRef<Path>>(path: P, block_id: u64, compression_level: i32) -> Self {
        let bloom_filter = BloomFilter::new(100000, 0.01);
        
        Self {
            path: path.as_ref().to_path_buf(),
            block_id,
            compression_level,
            series_data: HashMap::new(),
            bloom_filter,
        }
    }

    pub fn add_series(&mut self, series_id: TimeSeriesId, labels: Labels, samples: Vec<Sample>) {
        let data = self.series_data.entry(series_id).or_insert_with(|| {
            SeriesData {
                labels,
                timestamps: Vec::new(),
                values: Vec::new(),
            }
        });
        
        for sample in samples {
            data.timestamps.push(sample.timestamp);
            data.values.push(sample.value);
            
            let label_str = crate::model::labels_to_string(&data.labels);
            self.bloom_filter.insert(label_str.as_bytes());
        }
    }

    pub fn write(self) -> Result<Block> {
        fs::create_dir_all(&self.path)?;
        
        let block_dir = self.path.join(format!("block-{:020}", self.block_id));
        fs::create_dir_all(&block_dir)?;
        
        let mut all_timestamps = Vec::new();
        let mut all_values = Vec::new();
        let mut series_index = Vec::new();
        
        let mut offset = 0u64;
        for (series_id, data) in &self.series_data {
            let count = data.timestamps.len();
            series_index.push(SeriesIndexEntry {
                series_id: *series_id,
                labels: data.labels.clone(),
                offset,
                count: count as u64,
            });
            
            all_timestamps.extend(&data.timestamps);
            all_values.extend(&data.values);
            offset += count as u64;
        }
        
        let mut columns = HashMap::new();
        
        if !all_timestamps.is_empty() {
            let mut time_builder = ColumnBuilder::timestamps(self.compression_level);
            time_builder.add_timestamps(&all_timestamps);
            columns.insert("time".to_string(), time_builder.build()?);
        }
        
        if !all_values.is_empty() {
            let mut value_builder = ColumnBuilder::values(self.compression_level);
            value_builder.add_values(&all_values);
            columns.insert("value".to_string(), value_builder.build()?);
        }
        
        for (name, column) in &columns {
            let column_path = block_dir.join(format!("{}.col", name));
            let file = File::create(&column_path)?;
            let mut writer = BufWriter::new(file);
            writer.write_all(&column.data)?;
            writer.flush()?;
        }
        
        let index_data = self.build_index(&series_index)?;
        let index_path = block_dir.join("index.idx");
        let file = File::create(&index_path)?;
        let mut writer = BufWriter::new(file);
        writer.write_all(&index_data)?;
        writer.flush()?;
        
        let bloom_data = self.bloom_filter.serialize();
        let bloom_path = block_dir.join("bloom.bf");
        let file = File::create(&bloom_path)?;
        let mut writer = BufWriter::new(file);
        writer.write_all(&bloom_data)?;
        writer.flush()?;
        
        let total_samples: u64 = self.series_data.values().map(|d| d.timestamps.len() as u64).sum();
        let total_series = self.series_data.len() as u64;
        
        let min_ts = all_timestamps.iter().copied().min().unwrap_or(0);
        let max_ts = all_timestamps.iter().copied().max().unwrap_or(0);
        
        let meta = BlockMeta {
            magic: BLOCK_MAGIC,
            version: BLOCK_VERSION,
            block_id: self.block_id,
            min_timestamp: min_ts,
            max_timestamp: max_ts,
            total_series,
            total_samples,
            created_at: chrono::Utc::now().timestamp_millis(),
            compaction_level: 0,
            columns: columns.keys().cloned().collect(),
            index_offset: 0,
            index_size: index_data.len() as u64,
        };
        
        let meta_path = block_dir.join("meta.json");
        let meta_json = serde_json::to_string_pretty(&meta)?;
        fs::write(&meta_path, meta_json)?;
        
        Ok(Block {
            meta,
            columns,
            index_data,
        })
    }

    fn build_index(&self, entries: &[SeriesIndexEntry]) -> Result<Vec<u8>> {
        let mut data = Vec::new();
        
        let count = entries.len() as u32;
        data.extend_from_slice(&count.to_le_bytes());
        
        for entry in entries {
            data.extend_from_slice(&entry.series_id.to_le_bytes());
            data.extend_from_slice(&entry.offset.to_le_bytes());
            data.extend_from_slice(&entry.count.to_le_bytes());
            
            let labels_json = serde_json::to_vec(&entry.labels)?;
            let labels_len = labels_json.len() as u32;
            data.extend_from_slice(&labels_len.to_le_bytes());
            data.extend_from_slice(&labels_json);
        }
        
        Ok(data)
    }
}

#[derive(Debug, Clone)]
struct SeriesIndexEntry {
    series_id: TimeSeriesId,
    labels: Labels,
    offset: u64,
    count: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_block_writer() {
        let temp_dir = tempdir().unwrap();
        
        let mut writer = BlockWriter::new(temp_dir.path(), 1, 3);
        
        let labels = vec![Label::new("job", "test")];
        let samples = vec![
            Sample::new(1000, 1.0),
            Sample::new(2000, 2.0),
            Sample::new(3000, 3.0),
        ];
        
        writer.add_series(1, labels, samples);
        
        let block = writer.write().unwrap();
        
        assert_eq!(block.meta.total_series, 1);
        assert_eq!(block.meta.total_samples, 3);
    }
}
