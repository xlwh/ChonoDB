mod writer;
mod reader;
mod entry;
mod async_writer;

pub use writer::{WalWriter, WalSegmentConfig};
pub use reader::WalReader;
pub use entry::{WalEntry, WalEntryType};
pub use async_writer::{AsyncWalWriter, AsyncWalConfig, SharedAsyncWalWriter};

use crate::error::Result;
use crate::model::{Labels, Sample, TimeSeriesId};
use std::path::PathBuf;
use std::sync::Arc;
use parking_lot::Mutex;

#[derive(Debug, Clone)]
pub struct WalConfig {
    pub segment_config: WalSegmentConfig,
    pub retention_hours: u64,
    pub max_retention_bytes: u64,
    pub min_segments_to_keep: usize,
}

impl Default for WalConfig {
    fn default() -> Self {
        Self {
            segment_config: WalSegmentConfig::default(),
            retention_hours: 24,
            max_retention_bytes: 10 * 1024 * 1024 * 1024,
            min_segments_to_keep: 2,
        }
    }
}

pub struct Wal {
    writer: Arc<Mutex<WalWriter>>,
    path: PathBuf,
    config: WalConfig,
}

impl Wal {
    pub fn new<P: Into<PathBuf>>(path: P) -> Result<Self> {
        Self::with_config(path, WalConfig::default())
    }

    pub fn with_config<P: Into<PathBuf>>(path: P, config: WalConfig) -> Result<Self> {
        let path = path.into();
        let writer = WalWriter::with_config(&path, config.segment_config.clone())?;
        
        Ok(Self {
            writer: Arc::new(Mutex::new(writer)),
            path,
            config,
        })
    }

    pub fn log_write(&self, series_id: TimeSeriesId, labels: &Labels, samples: &[Sample]) -> Result<()> {
        let entry = WalEntry::write(series_id, labels.clone(), samples.to_vec());
        let mut writer = self.writer.lock();
        writer.write(&entry)
    }

    pub fn log_delete(&self, series_id: TimeSeriesId) -> Result<()> {
        let entry = WalEntry::delete(series_id);
        let mut writer = self.writer.lock();
        writer.write(&entry)
    }

    pub fn sync(&self) -> Result<()> {
        let mut writer = self.writer.lock();
        writer.sync()
    }

    pub fn rotate(&self) -> Result<()> {
        let mut writer = self.writer.lock();
        writer.rotate()
    }

    pub fn reader(&self) -> Result<WalReader> {
        WalReader::new(&self.path)
    }

    pub fn path(&self) -> &std::path::Path {
        &self.path
    }

    pub fn cleanup_old_segments(&self) -> Result<usize> {
        let reader = self.reader()?;
        let segments = reader.segments().to_vec();
        
        if segments.len() <= self.config.min_segments_to_keep {
            return Ok(0);
        }
        
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        
        let retention_secs = self.config.retention_hours * 3600;
        
        let mut bytes_total = 0u64;
        let mut segments_by_time = Vec::new();
        
        for &segment in &segments {
            let segment_path = self.path.join(format!("segment-{:020}", segment));
            if let Ok(metadata) = std::fs::metadata(&segment_path) {
                let created = metadata.created().unwrap_or(std::time::SystemTime::UNIX_EPOCH);
                let created_secs = created.duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs();
                bytes_total += metadata.len();
                segments_by_time.push((segment, created_secs, metadata.len()));
            }
        }
        
        segments_by_time.sort_by_key(|(_, created, _)| *created);
        
        let mut deleted_count = 0;
        let mut bytes_to_delete = 0u64;
        let total_segments = segments_by_time.len();
        
        for (segment, created_secs, size) in segments_by_time {
            if total_segments - deleted_count <= self.config.min_segments_to_keep {
                break;
            }
            
            let age_secs = now - created_secs;
            let should_delete_by_time = age_secs > retention_secs;
            let should_delete_by_size = bytes_total - bytes_to_delete > self.config.max_retention_bytes;
            
            if should_delete_by_time || should_delete_by_size {
                let segment_path = self.path.join(format!("segment-{:020}", segment));
                if std::fs::remove_file(&segment_path).is_ok() {
                    deleted_count += 1;
                    bytes_to_delete += size;
                    tracing::info!("Deleted old WAL segment: {}", segment);
                }
            }
        }
        
        Ok(deleted_count)
    }

    pub fn recover(&self) -> Result<Vec<WalEntry>> {
        let reader = self.reader()?;
        let mut entries = Vec::new();
        
        for &segment in reader.segments() {
            match reader.read_segment(segment) {
                Ok(mut segment_entries) => {
                    entries.append(&mut segment_entries);
                }
                Err(e) => {
                    tracing::warn!("Failed to read WAL segment {}: {}", segment, e);
                }
            }
        }
        
        entries.sort_by_key(|e| e.timestamp);
        
        tracing::info!("Recovered {} WAL entries from {} segments", entries.len(), reader.segment_count());
        
        Ok(entries)
    }

    pub fn validate(&self) -> Result<()> {
        let reader = self.reader()?;
        
        for &segment in reader.segments() {
            match reader.read_segment(segment) {
                Ok(entries) => {
                    tracing::debug!("Validated segment {}: {} entries", segment, entries.len());
                }
                Err(e) => {
                    tracing::error!("WAL segment {} validation failed: {}", segment, e);
                    return Err(e);
                }
            }
        }
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Label, Sample};
    use tempfile::tempdir;

    #[test]
    fn test_wal_new() {
        let temp_dir = tempdir().unwrap();
        let wal_path = temp_dir.path().join("wal");
        let wal = Wal::new(&wal_path).unwrap();
        assert_eq!(wal.path(), wal_path.as_path());
    }

    #[test]
    fn test_wal_log_write() {
        let temp_dir = tempdir().unwrap();
        let wal_path = temp_dir.path().join("wal");
        let wal = Wal::new(&wal_path).unwrap();

        let labels = vec![Label::new("job", "test")];
        let samples = vec![Sample::new(1000, 42.0)];

        let result = wal.log_write(1, &labels, &samples);
        assert!(result.is_ok());
    }

    #[test]
    fn test_wal_log_delete() {
        let temp_dir = tempdir().unwrap();
        let wal_path = temp_dir.path().join("wal");
        let wal = Wal::new(&wal_path).unwrap();

        let result = wal.log_delete(1);
        assert!(result.is_ok());
    }

    #[test]
    fn test_wal_sync() {
        let temp_dir = tempdir().unwrap();
        let wal_path = temp_dir.path().join("wal");
        let wal = Wal::new(&wal_path).unwrap();

        let labels = vec![Label::new("job", "test")];
        let samples = vec![Sample::new(1000, 42.0)];
        wal.log_write(1, &labels, &samples).unwrap();

        let result = wal.sync();
        assert!(result.is_ok());
    }

    #[test]
    fn test_wal_rotate() {
        let temp_dir = tempdir().unwrap();
        let wal_path = temp_dir.path().join("wal");
        let wal = Wal::new(&wal_path).unwrap();

        let labels = vec![Label::new("job", "test")];
        let samples = vec![Sample::new(1000, 42.0)];
        wal.log_write(1, &labels, &samples).unwrap();

        let result = wal.rotate();
        assert!(result.is_ok());
    }

    #[test]
    fn test_wal_reader() {
        let temp_dir = tempdir().unwrap();
        let wal_path = temp_dir.path().join("wal");
        let wal = Wal::new(&wal_path).unwrap();

        let labels = vec![Label::new("job", "test")];
        let samples = vec![Sample::new(1000, 42.0)];
        wal.log_write(1, &labels, &samples).unwrap();
        wal.sync().unwrap();

        let reader = wal.reader();
        assert!(reader.is_ok());
    }

    #[test]
    fn test_wal_multiple_writes() {
        let temp_dir = tempdir().unwrap();
        let wal_path = temp_dir.path().join("wal");
        let wal = Wal::new(&wal_path).unwrap();

        let labels = vec![Label::new("job", "test")];

        for i in 0..10 {
            let samples = vec![Sample::new(1000 + i, i as f64)];
            wal.log_write(i as u64, &labels, &samples).unwrap();
        }

        wal.sync().unwrap();
    }
}
