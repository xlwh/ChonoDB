use crate::error::{Error, Result};
use crate::wal::entry::{WalEntry, WalHeader, WAL_ENTRY_HEADER_SIZE};
use std::fs::{self, File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

const DEFAULT_SEGMENT_SIZE: u64 = 128 * 1024 * 1024;

pub struct WalWriter {
    writer: BufWriter<File>,
    current_segment: u64,
    current_size: u64,
    sequence: AtomicU64,
    path: PathBuf,
    max_segment_size: u64,
}

impl WalWriter {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        
        fs::create_dir_all(&path)?;
        
        let (segment, sequence) = Self::find_latest_segment(&path)?;
        
        let file_path = Self::segment_path(&path, segment);
        let file_exists = file_path.exists();
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&file_path)?;
        
        let current_size = file.metadata()?.len();
        let mut writer = BufWriter::new(file);
        
        if !file_exists || current_size == 0 {
            let header = WalHeader::new(sequence);
            let header_data = header.encode();
            writer.write_all(&header_data)?;
            writer.flush()?;
        }
        
        Ok(Self {
            writer,
            current_segment: segment,
            current_size,
            sequence: AtomicU64::new(sequence),
            path,
            max_segment_size: DEFAULT_SEGMENT_SIZE,
        })
    }

    fn find_latest_segment(path: &Path) -> Result<(u64, u64)> {
        let mut latest_segment = 0u64;
        let mut latest_sequence = 0u64;
        
        if path.exists() {
            for entry in fs::read_dir(path)? {
                let entry = entry?;
                let file_name = entry.file_name();
                let name = file_name.to_string_lossy();
                
                if let Some(segment) = name.strip_prefix("segment-").and_then(|s| s.parse::<u64>().ok()) {
                    if segment > latest_segment {
                        latest_segment = segment;
                    }
                }
            }
        }
        
        Ok((latest_segment, latest_sequence))
    }

    fn segment_path(path: &Path, segment: u64) -> PathBuf {
        path.join(format!("segment-{:020}", segment))
    }

    pub fn write(&mut self, entry: &WalEntry) -> Result<()> {
        let data = entry.encode()?;
        let seq = self.sequence.fetch_add(1, Ordering::SeqCst);
        
        let mut header = Vec::with_capacity(8);
        header.extend_from_slice(&seq.to_le_bytes());
        
        self.writer.write_all(&header)?;
        self.writer.write_all(&data)?;
        self.writer.flush()?;
        
        self.current_size += header.len() as u64 + data.len() as u64;
        
        if self.current_size >= self.max_segment_size {
            self.rotate()?;
        }
        
        Ok(())
    }

    pub fn sync(&mut self) -> Result<()> {
        self.writer.flush()?;
        self.writer.get_ref().sync_all()?;
        Ok(())
    }

    pub fn rotate(&mut self) -> Result<()> {
        self.sync()?;
        
        self.current_segment += 1;
        self.current_size = 0;
        
        let file_path = Self::segment_path(&self.path, self.current_segment);
        let file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&file_path)?;
        
        let header = WalHeader::new(self.sequence.load(Ordering::SeqCst));
        let header_data = header.encode();
        
        let mut writer = BufWriter::new(file);
        writer.write_all(&header_data)?;
        writer.flush()?;
        
        self.writer = writer;
        
        Ok(())
    }

    pub fn current_sequence(&self) -> u64 {
        self.sequence.load(Ordering::SeqCst)
    }

    pub fn current_segment(&self) -> u64 {
        self.current_segment
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Label, Sample};
    use tempfile::tempdir;

    #[test]
    fn test_wal_writer_basic() {
        let temp_dir = tempdir().unwrap();
        let mut writer = WalWriter::new(temp_dir.path()).unwrap();
        
        let labels = vec![Label::new("job", "test")];
        let samples = vec![Sample::new(1000, 1.0)];
        let entry = WalEntry::write(1, labels, samples);
        
        writer.write(&entry).unwrap();
        writer.sync().unwrap();
        
        assert!(writer.current_sequence() > 0);
    }
}
