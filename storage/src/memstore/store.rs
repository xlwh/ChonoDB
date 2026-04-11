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
use std::collections::HashMap;

const DEFAULT_BLOOM_FILTER_CAPACITY: usize = 1000000;
const DEFAULT_BLOOM_FALSE_POSITIVE_RATE: f64 = 0.01;

pub struct MemStore {
    head: Arc<HeadBlock>,
    bloom: RwLock<BloomFilter>,
    wal: Option<Arc<Wal>>,
    config: StorageConfig,
    stats: RwLock<MemStoreStats>,
    write_buffer: RwLock<WriteBuffer>,
}

#[derive(Debug, Clone, Default)]
pub struct MemStoreStats {
    pub total_series: u64,
    pub total_samples: u64,
    pub total_bytes: u64,
    pub writes: u64,
    pub reads: u64,
    pub batch_writes: u64,
    pub write_buffer_flushes: u64,
}

#[derive(Default)]
struct WriteBuffer {
    entries: HashMap<Labels, Vec<Sample>>,
    size: usize,
}

impl WriteBuffer {
    fn add(&mut self, labels: Labels, samples: Vec<Sample>) {
        let entry = self.entries.entry(labels).or_default();
        let sample_count = samples.len();
        entry.extend(samples);
        self.size += sample_count;
    }

    fn is_full(&self, threshold: usize) -> bool {
        self.size >= threshold
    }

    fn clear(&mut self) {
        self.entries.clear();
        self.size = 0;
    }
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
            write_buffer: RwLock::new(WriteBuffer::default()),
        })
    }

    pub fn write(&self, labels: Labels, samples: Vec<Sample>) -> Result<()> {
        // 检查写入缓冲区大小
        let buffer_threshold = 1000; // 缓冲区阈值，可配置
        
        let mut write_buffer = self.write_buffer.write();
        write_buffer.add(labels, samples);
        
        if write_buffer.is_full(buffer_threshold) {
            self.flush_write_buffer(&mut write_buffer)?;
        }
        
        Ok(())
    }

    pub fn write_single(&self, labels: Labels, sample: Sample) -> Result<()> {
        self.write(labels, vec![sample])
    }

    pub fn write_batch(&self, batch: Vec<(Labels, Vec<Sample>)>) -> Result<()> {
        let mut write_buffer = self.write_buffer.write();
        
        for (labels, samples) in batch {
            write_buffer.add(labels, samples);
        }
        
        self.flush_write_buffer(&mut write_buffer)?;
        
        let mut stats = self.stats.write();
        stats.batch_writes += 1;
        
        Ok(())
    }

    fn flush_write_buffer(&self, buffer: &mut WriteBuffer) -> Result<()> {
        if buffer.entries.is_empty() {
            return Ok(());
        }
        
        // 收集所有需要写入的系列
        let entries = std::mem::take(&mut buffer.entries);
        buffer.clear();
        
        // 批量处理写入
        let mut total_samples = 0;
        let mut wal_entries = Vec::new();
        
        for (labels, samples) in entries {
            let series_id = self.head.get_or_create_series(labels.clone())?;
            let sample_count = samples.len();
            
            // 排序样本
            let mut sorted_samples = samples;
            sorted_samples.sort_by_key(|s| s.timestamp);
            
            // 收集 WAL 条目
            if self.wal.is_some() {
                wal_entries.push((series_id, labels.clone(), sorted_samples.clone()));
            }
            
            // 批量添加样本
            self.head.add_samples(series_id, sorted_samples)?;
            total_samples += sample_count;
        }
        
        // 批量写入 WAL
        if let Some(ref wal) = self.wal {
            for (series_id, labels, samples) in wal_entries {
                wal.log_write(series_id, &labels, &samples)?;
            }
        }
        
        // 批量更新统计信息
        {
            let mut stats = self.stats.write();
            stats.total_samples += total_samples as u64;
            stats.writes += total_samples as u64;
            stats.write_buffer_flushes += 1;
            stats.total_series = self.head.series_count() as u64;
        }
        
        Ok(())
    }

    pub fn flush(&self) -> Result<()> {
        let mut write_buffer = self.write_buffer.write();
        self.flush_write_buffer(&mut write_buffer)
    }

    pub fn query(
        &self,
        label_matchers: &[(String, String)],
        start: i64,
        end: i64,
    ) -> Result<Vec<TimeSeries>> {
        // 先刷新缓冲区，确保查询结果包含最新数据
        self.flush()?;
        
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
        _level: DownsampleLevel,
    ) -> Result<Vec<TimeSeries>> {
        let series_ids = self.find_series(label_matchers)?;
        
        let mut result = Vec::with_capacity(series_ids.len());
        
        for series_id in series_ids {
            if let Some(labels) = self.head.get_series_labels(series_id) {
                if let Some(samples) = self.head.query(series_id, start, end) {
                    if !samples.is_empty() {
                        let mut ts = TimeSeries::new(series_id, labels);
                        ts.add_samples(samples);
                        result.push(ts);
                    }
                }
            }
        }
        
        let mut stats = self.stats.write();
        stats.reads += 1;
        
        Ok(result)
    }

    // 公开方法，供其他模块使用
    pub fn find_series(&self, label_matchers: &[(String, String)]) -> Result<Vec<TimeSeriesId>> {
        // 简化实现，实际应该使用索引
        // 这里返回所有系列，实际应该根据匹配器过滤
        Ok(self.get_all_series_ids())
    }

    // 公开方法，供其他模块使用
    pub fn get_series(&self, series_id: TimeSeriesId) -> Option<TimeSeries> {
        let labels = self.head.get_series_labels(series_id)?;
        let samples = self.head.query(series_id, 0, i64::MAX)?;
        
        let mut ts = TimeSeries::new(series_id, labels);
        ts.add_samples(samples);
        Some(ts)
    }

    // 公开方法，供其他模块使用
    pub fn get_all_series_ids(&self) -> Vec<TimeSeriesId> {
        self.head.get_all_series_ids()
    }

    fn apply_downsampling(&self, samples: Vec<Sample>, _level: DownsampleLevel) -> Vec<Sample> {
        // 实现降采样逻辑
        // 这里简化处理，实际应该根据降采样级别进行聚合
        samples
    }

    pub fn stats(&self) -> MemStoreStats {
        self.stats.read().clone()
    }

    pub fn series_count(&self) -> usize {
        self.head.series_count()
    }

    pub fn total_samples(&self) -> usize {
        self.head.total_samples()
    }
}
