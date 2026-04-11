mod writer;
mod reader;
mod entry;

pub use writer::WalWriter;
pub use reader::WalReader;
pub use entry::{WalEntry, WalEntryType};

use crate::error::Result;
use crate::model::{Labels, Sample, TimeSeriesId};
use std::path::PathBuf;
use std::sync::Arc;
use parking_lot::Mutex;

pub struct Wal {
    writer: Arc<Mutex<WalWriter>>,
    path: PathBuf,
}

impl Wal {
    pub fn new<P: Into<PathBuf>>(path: P) -> Result<Self> {
        let path = path.into();
        let writer = WalWriter::new(&path)?;
        
        Ok(Self {
            writer: Arc::new(Mutex::new(writer)),
            path,
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
