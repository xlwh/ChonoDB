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
