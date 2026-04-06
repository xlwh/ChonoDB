use crate::columnstore::{Block, BlockMeta, ColumnBuilder, BLOCK_MAGIC, BLOCK_VERSION};
use crate::columnstore::DownsampleLevel;
use crate::error::Result;
use crate::index::BloomFilter;
use crate::model::{Labels, Sample, TimeSeriesId};
use crate::downsample::DownsamplePoint;
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

pub struct BlockWriter {
    path: PathBuf,
    block_id: u64,
    compression_level: i32,
    series_data: HashMap<TimeSeriesId, SeriesData>,
    downsample_data: HashMap<TimeSeriesId, DownsampleSeriesData>,
    bloom_filter: BloomFilter,
    downsample_level: Option<DownsampleLevel>,
}

struct SeriesData {
    labels: Labels,
    timestamps: Vec<i64>,
    values: Vec<f64>,
}

struct DownsampleSeriesData {
    labels: Labels,
    points: Vec<DownsamplePoint>,
}

impl BlockWriter {
    pub fn new<P: AsRef<Path>>(path: P, block_id: u64, compression_level: i32) -> Self {
        let bloom_filter = BloomFilter::new(100000, 0.01);
        
        Self {
            path: path.as_ref().to_path_buf(),
            block_id,
            compression_level,
            series_data: HashMap::new(),
            downsample_data: HashMap::new(),
            bloom_filter,
            downsample_level: None,
        }
    }

    pub fn new_downsample<P: AsRef<Path>>(path: P, block_id: u64, compression_level: i32, level: DownsampleLevel) -> Self {
        let bloom_filter = BloomFilter::new(100000, 0.01);
        
        Self {
            path: path.as_ref().to_path_buf(),
            block_id,
            compression_level,
            series_data: HashMap::new(),
            downsample_data: HashMap::new(),
            bloom_filter,
            downsample_level: Some(level),
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

    pub fn add_downsample_series(&mut self, series_id: TimeSeriesId, labels: Labels, points: Vec<DownsamplePoint>) {
        let data = self.downsample_data.entry(series_id).or_insert_with(|| {
            DownsampleSeriesData {
                labels,
                points: Vec::new(),
            }
        });
        
        data.points.extend(points);
        
        let label_str = crate::model::labels_to_string(&data.labels);
        self.bloom_filter.insert(label_str.as_bytes());
    }

    pub fn write(self) -> Result<Block> {
        fs::create_dir_all(&self.path)?;
        
        let block_dir = self.path.join(format!("block-{:020}", self.block_id));
        fs::create_dir_all(&block_dir)?;
        
        let mut columns = HashMap::new();
        let mut series_index = Vec::new();
        let mut total_samples = 0u64;
        let mut total_series = 0u64;
        let mut min_ts = 0i64;
        let mut max_ts = 0i64;

        if !self.series_data.is_empty() {
            let mut all_timestamps = Vec::new();
            let mut all_values = Vec::new();
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
            
            total_series += self.series_data.len() as u64;
            total_samples += all_timestamps.len() as u64;
            
            if !all_timestamps.is_empty() {
                min_ts = all_timestamps.iter().copied().min().unwrap_or(0);
                max_ts = all_timestamps.iter().copied().max().unwrap_or(0);
                
                let mut time_builder = ColumnBuilder::timestamps(self.compression_level);
                time_builder.add_timestamps(&all_timestamps);
                columns.insert("time".to_string(), time_builder.build()?);
            }
            
            if !all_values.is_empty() {
                let mut value_builder = ColumnBuilder::values(self.compression_level);
                value_builder.add_values(&all_values);
                columns.insert("value".to_string(), value_builder.build()?);
            }
        }

        if !self.downsample_data.is_empty() {
            let mut all_timestamps = Vec::new();
            let mut all_min_values = Vec::new();
            let mut all_max_values = Vec::new();
            let mut all_avg_values = Vec::new();
            let mut all_sum_values = Vec::new();
            let mut all_counts = Vec::new();
            let mut all_last_values = Vec::new();
            let mut offset = 0u64;

            for (series_id, data) in &self.downsample_data {
                let count = data.points.len();
                series_index.push(SeriesIndexEntry {
                    series_id: *series_id,
                    labels: data.labels.clone(),
                    offset,
                    count: count as u64,
                });
                
                for point in &data.points {
                    all_timestamps.push(point.timestamp);
                    all_min_values.push(point.min_value);
                    all_max_values.push(point.max_value);
                    all_avg_values.push(point.avg_value);
                    all_sum_values.push(point.sum_value);
                    all_counts.push(point.count as f64);
                    all_last_values.push(point.last_value);
                }
                offset += count as u64;
            }
            
            total_series += self.downsample_data.len() as u64;
            total_samples += all_timestamps.len() as u64;
            
            if !all_timestamps.is_empty() {
                let current_min = all_timestamps.iter().copied().min().unwrap_or(0);
                let current_max = all_timestamps.iter().copied().max().unwrap_or(0);
                
                if min_ts == 0 || current_min < min_ts {
                    min_ts = current_min;
                }
                if max_ts == 0 || current_max > max_ts {
                    max_ts = current_max;
                }
                
                let mut time_builder = ColumnBuilder::timestamps(self.compression_level);
                time_builder.add_timestamps(&all_timestamps);
                columns.insert("time".to_string(), time_builder.build()?);
            }
            
            if !all_min_values.is_empty() {
                let mut min_builder = ColumnBuilder::min(self.compression_level);
                min_builder.add_values(&all_min_values);
                columns.insert("min".to_string(), min_builder.build()?);
            }
            
            if !all_max_values.is_empty() {
                let mut max_builder = ColumnBuilder::max(self.compression_level);
                max_builder.add_values(&all_max_values);
                columns.insert("max".to_string(), max_builder.build()?);
            }
            
            if !all_avg_values.is_empty() {
                let mut avg_builder = ColumnBuilder::avg(self.compression_level);
                avg_builder.add_values(&all_avg_values);
                columns.insert("avg".to_string(), avg_builder.build()?);
            }
            
            if !all_sum_values.is_empty() {
                let mut sum_builder = ColumnBuilder::sum(self.compression_level);
                sum_builder.add_values(&all_sum_values);
                columns.insert("sum".to_string(), sum_builder.build()?);
            }
            
            if !all_counts.is_empty() {
                let mut count_builder = ColumnBuilder::count(self.compression_level);
                count_builder.add_values(&all_counts);
                columns.insert("count".to_string(), count_builder.build()?);
            }
            
            if !all_last_values.is_empty() {
                let mut last_builder = ColumnBuilder::last(self.compression_level);
                last_builder.add_values(&all_last_values);
                columns.insert("last".to_string(), last_builder.build()?);
            }
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
            downsample_level: self.downsample_level,
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
    use crate::model::Label;
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
