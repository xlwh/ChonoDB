use crate::columnstore::{Block, BlockMeta, Column, ColumnType};
use crate::error::{Error, Result};
use crate::index::BloomFilter;
use crate::model::{Label, Labels, Sample, TimeSeriesId};
use memmap2::Mmap;
use std::collections::HashMap;
use std::fs::File;
use std::path::{Path, PathBuf};

pub struct BlockReader {
    path: PathBuf,
    meta: BlockMeta,
    bloom_filter: BloomFilter,
    mmap: Option<Mmap>,
}

impl BlockReader {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        
        let meta_path = path.join("meta.json");
        let meta_json = std::fs::read_to_string(&meta_path)?;
        let meta: BlockMeta = serde_json::from_str(&meta_json)?;
        
        if meta.magic != crate::columnstore::BLOCK_MAGIC {
            return Err(Error::InvalidData("Invalid block magic".to_string()));
        }
        
        let bloom_path = path.join("bloom.bf");
        let bloom_data = std::fs::read(&bloom_path)?;
        let bloom_filter = BloomFilter::deserialize(&bloom_data)?;
        
        Ok(Self {
            path,
            meta,
            bloom_filter,
            mmap: None,
        })
    }

    pub fn meta(&self) -> &BlockMeta {
        &self.meta
    }

    pub fn contains_series(&self, labels: &Labels) -> bool {
        let label_str = crate::model::labels_to_string(labels);
        self.bloom_filter.contains(label_str.as_bytes())
    }

    pub fn read_column(&self, column_type: ColumnType) -> Result<Column> {
        let extension = column_type.file_extension();
        let column_path = self.path.join(extension);
        
        let data = std::fs::read(&column_path)?;
        let len = data.len();
        
        Ok(Column {
            column_type,
            data,
            uncompressed_size: 0,
            compressed_size: len,
            num_values: 0,
        })
    }

    pub fn read_index(&self) -> Result<Vec<SeriesIndexEntry>> {
        let index_path = self.path.join("index.idx");
        let data = std::fs::read(&index_path)?;
        
        let mut entries = Vec::new();
        let mut pos = 0;
        
        if data.len() < 4 {
            return Ok(entries);
        }
        
        let count = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;
        pos = 4;
        
        for _ in 0..count {
            if pos + 24 > data.len() {
                break;
            }
            
            let series_id = u64::from_le_bytes([
                data[pos], data[pos + 1], data[pos + 2], data[pos + 3],
                data[pos + 4], data[pos + 5], data[pos + 6], data[pos + 7],
            ]);
            pos += 8;
            
            let offset = u64::from_le_bytes([
                data[pos], data[pos + 1], data[pos + 2], data[pos + 3],
                data[pos + 4], data[pos + 5], data[pos + 6], data[pos + 7],
            ]);
            pos += 8;
            
            let count = u64::from_le_bytes([
                data[pos], data[pos + 1], data[pos + 2], data[pos + 3],
                data[pos + 4], data[pos + 5], data[pos + 6], data[pos + 7],
            ]);
            pos += 8;
            
            if pos + 4 > data.len() {
                break;
            }
            
            let labels_len = u32::from_le_bytes([
                data[pos], data[pos + 1], data[pos + 2], data[pos + 3],
            ]) as usize;
            pos += 4;
            
            if pos + labels_len > data.len() {
                break;
            }
            
            let labels: Labels = serde_json::from_slice(&data[pos..pos + labels_len])?;
            pos += labels_len;
            
            entries.push(SeriesIndexEntry {
                series_id,
                labels,
                offset,
                count,
            });
        }
        
        Ok(entries)
    }

    pub fn query(&self, series_id: TimeSeriesId, start: i64, end: i64) -> Result<Vec<Sample>> {
        let index = self.read_index()?;
        
        let entry = index
            .iter()
            .find(|e| e.series_id == series_id)
            .ok_or_else(|| Error::SeriesNotFound(series_id))?;
        
        let time_column = self.read_column(ColumnType::Timestamp)?;
        let value_column = self.read_column(ColumnType::Value)?;
        
        let timestamps = super::column::decode_timestamp_column(&time_column)?;
        let values = super::column::decode_value_column(&value_column)?;
        
        let start_idx = entry.offset as usize;
        let end_idx = start_idx + entry.count as usize;
        
        let mut samples = Vec::new();
        for i in start_idx..end_idx.min(timestamps.len()).min(values.len()) {
            let ts = timestamps[i];
            if ts >= start && ts <= end {
                samples.push(Sample::new(ts, values[i]));
            }
        }
        
        Ok(samples)
    }

    pub fn query_by_labels(&self, labels: &Labels, start: i64, end: i64) -> Result<Vec<(TimeSeriesId, Vec<Sample>)>> {
        if !self.contains_series(labels) {
            return Ok(Vec::new());
        }
        
        let index = self.read_index()?;
        let mut results = Vec::new();
        
        for entry in &index {
            if labels_match(&entry.labels, labels) {
                let samples = self.query(entry.series_id, start, end)?;
                if !samples.is_empty() {
                    results.push((entry.series_id, samples));
                }
            }
        }
        
        Ok(results)
    }
}

#[derive(Debug, Clone)]
struct SeriesIndexEntry {
    series_id: TimeSeriesId,
    labels: Labels,
    offset: u64,
    count: u64,
}

fn labels_match(series_labels: &Labels, query_labels: &Labels) -> bool {
    for query_label in query_labels {
        if !series_labels.iter().any(|l| l.name == query_label.name && l.value == query_label.value) {
            return false;
        }
    }
    true
}

pub struct BlockManager {
    base_path: PathBuf,
    blocks: HashMap<u64, BlockReader>,
}

impl BlockManager {
    pub fn new<P: AsRef<Path>>(base_path: P) -> Result<Self> {
        let base_path = base_path.as_ref().to_path_buf();
        let mut blocks = HashMap::new();
        
        if base_path.exists() {
            for entry in std::fs::read_dir(&base_path)? {
                let entry = entry?;
                let path = entry.path();
                
                if path.is_dir() {
                    if let Some(block_id) = path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .and_then(|s| s.strip_prefix("block-"))
                        .and_then(|s| s.parse::<u64>().ok())
                    {
                        if let Ok(reader) = BlockReader::open(&path) {
                            blocks.insert(block_id, reader);
                        }
                    }
                }
            }
        }
        
        Ok(Self { base_path, blocks })
    }

    pub fn blocks(&self) -> &HashMap<u64, BlockReader> {
        &self.blocks
    }

    pub fn get_block(&self, block_id: u64) -> Option<&BlockReader> {
        self.blocks.get(&block_id)
    }

    pub fn find_blocks_in_range(&self, start: i64, end: i64) -> Vec<&BlockReader> {
        self.blocks
            .values()
            .filter(|b| {
                b.meta().min_timestamp <= end && b.meta().max_timestamp >= start
            })
            .collect()
    }

    pub fn total_blocks(&self) -> usize {
        self.blocks.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::columnstore::BlockWriter;
    use tempfile::tempdir;

    #[test]
    fn test_block_read_write() {
        let temp_dir = tempdir().unwrap();
        
        let block_path = temp_dir.path().join("block-00000000000000000001");
        
        {
            let mut writer = BlockWriter::new(temp_dir.path(), 1, 3);
            
            let labels = vec![Label::new("job", "test")];
            let samples = vec![
                Sample::new(1000, 1.0),
                Sample::new(2000, 2.0),
                Sample::new(3000, 3.0),
            ];
            
            writer.add_series(1, labels, samples);
            writer.write().unwrap();
        }
        
        let reader = BlockReader::open(&block_path).unwrap();
        
        assert_eq!(reader.meta().total_series, 1);
        assert_eq!(reader.meta().total_samples, 3);
    }
}
