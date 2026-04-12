use crate::error::Result;
use crate::wal::entry::{WalEntry, WalHeader};
use std::fs::{self, File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

const DEFAULT_SEGMENT_SIZE: u64 = 128 * 1024 * 1024;
const DEFAULT_SEGMENT_ENTRIES: u64 = 100000;
const DEFAULT_SEGMENT_DURATION_SECS: u64 = 3600;

#[derive(Debug, Clone)]
pub struct WalSegmentConfig {
    pub max_segment_size: u64,
    pub max_segment_entries: u64,
    pub max_segment_duration_secs: u64,
}

impl Default for WalSegmentConfig {
    fn default() -> Self {
        Self {
            max_segment_size: DEFAULT_SEGMENT_SIZE,
            max_segment_entries: DEFAULT_SEGMENT_ENTRIES,
            max_segment_duration_secs: DEFAULT_SEGMENT_DURATION_SECS,
        }
    }
}

pub struct WalWriter {
    writer: BufWriter<File>,
    current_segment: u64,
    current_size: u64,
    current_entries: u64,
    segment_created_at: u64,
    sequence: AtomicU64,
    path: PathBuf,
    config: WalSegmentConfig,
}

impl WalWriter {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        Self::with_config(path, WalSegmentConfig::default())
    }

    pub fn with_config<P: AsRef<Path>>(path: P, config: WalSegmentConfig) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        
        fs::create_dir_all(&path)?;
        
        let (segment, sequence, created_at) = Self::find_latest_segment(&path)?;
        
        let file_path = Self::segment_path(&path, segment);
        let file_exists = file_path.exists();
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&file_path)?;
        
        let current_size = file.metadata()?.len();
        let current_entries = Self::count_entries_in_segment(&file_path)?;
        
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
            current_entries,
            segment_created_at: created_at,
            sequence: AtomicU64::new(sequence),
            path,
            config,
        })
    }

    fn find_latest_segment(path: &Path) -> Result<(u64, u64, u64)> {
        let mut latest_segment = 0u64;
        let latest_sequence = 0u64;
        let mut created_at = 0u64;
        
        if path.exists() {
            for entry in fs::read_dir(path)? {
                let entry = entry?;
                let file_name = entry.file_name();
                let name = file_name.to_string_lossy();
                
                if let Some(segment) = name.strip_prefix("segment-").and_then(|s| s.parse::<u64>().ok()) {
                    if segment > latest_segment {
                        latest_segment = segment;
                        if let Ok(metadata) = entry.metadata() {
                            if let Ok(time) = metadata.created() {
                                created_at = time.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
                            }
                        }
                    }
                }
            }
        }
        
        Ok((latest_segment, latest_sequence, created_at))
    }

    fn count_entries_in_segment(file_path: &Path) -> Result<u64> {
        if !file_path.exists() {
            return Ok(0);
        }
        
        let file = File::open(file_path)?;
        let metadata = file.metadata()?;
        let file_size = metadata.len();
        
        if file_size <= WalHeader::SIZE as u64 {
            return Ok(0);
        }
        
        Ok((file_size - WalHeader::SIZE as u64) / 1024)
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
        self.current_entries += 1;
        
        if self.should_rotate() {
            self.rotate()?;
        }
        
        Ok(())
    }

    fn should_rotate(&self) -> bool {
        // 按大小分段
        if self.current_size >= self.config.max_segment_size {
            tracing::debug!("Rotating WAL segment: size limit reached");
            return true;
        }
        
        // 按条目数分段
        if self.current_entries >= self.config.max_segment_entries {
            tracing::debug!("Rotating WAL segment: entry limit reached");
            return true;
        }
        
        // 按时间分段
        if self.config.max_segment_duration_secs > 0 {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            
            if now - self.segment_created_at >= self.config.max_segment_duration_secs {
                tracing::debug!("Rotating WAL segment: time limit reached");
                return true;
            }
        }
        
        false
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
        self.current_entries = 0;
        self.segment_created_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        
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
        
        tracing::info!("Rotated WAL segment to {}", self.current_segment);
        
        Ok(())
    }

    pub fn current_entries(&self) -> u64 {
        self.current_entries
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
