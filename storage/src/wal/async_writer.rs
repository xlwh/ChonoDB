use crate::error::Result;
use crate::wal::entry::{WalEntry, WalHeader};
use tokio::fs::{self, File, OpenOptions};
use tokio::io::{BufWriter, AsyncWriteExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::Mutex;

const DEFAULT_BUFFER_SIZE: usize = 64 * 1024;
const DEFAULT_FLUSH_INTERVAL_MS: u64 = 100;
const DEFAULT_BATCH_SIZE: usize = 100;

#[derive(Debug, Clone)]
pub struct AsyncWalConfig {
    pub buffer_size: usize,
    pub flush_interval_ms: u64,
    pub batch_size: usize,
    pub max_segment_size: u64,
    pub max_segment_entries: u64,
    pub max_segment_duration_secs: u64,
}

impl Default for AsyncWalConfig {
    fn default() -> Self {
        Self {
            buffer_size: DEFAULT_BUFFER_SIZE,
            flush_interval_ms: DEFAULT_FLUSH_INTERVAL_MS,
            batch_size: DEFAULT_BATCH_SIZE,
            max_segment_size: 128 * 1024 * 1024,
            max_segment_entries: 100000,
            max_segment_duration_secs: 3600,
        }
    }
}

pub struct AsyncWalWriter {
    writer: BufWriter<File>,
    current_segment: u64,
    current_size: u64,
    current_entries: u64,
    segment_created_at: u64,
    sequence: AtomicU64,
    path: PathBuf,
    config: AsyncWalConfig,
    pending_batch: Vec<Vec<u8>>,
}

impl AsyncWalWriter {
    pub async fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        Self::with_config(path, AsyncWalConfig::default()).await
    }

    pub async fn with_config<P: AsRef<Path>>(path: P, config: AsyncWalConfig) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        
        fs::create_dir_all(&path).await?;
        
        let (segment, sequence, created_at) = Self::find_latest_segment(&path).await?;
        
        let file_path = Self::segment_path(&path, segment);
        let file_exists = file_path.exists();
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&file_path)
            .await?;
        
        let current_size = file.metadata().await?.len();
        let current_entries = Self::count_entries_in_segment(&file_path).await?;
        let batch_capacity = config.batch_size;
        let buffer_size = config.buffer_size;
        
        let mut writer = BufWriter::with_capacity(buffer_size, file);
        
        if !file_exists || current_size == 0 {
            let header = WalHeader::new(sequence);
            let header_data = header.encode();
            writer.write_all(&header_data).await?;
            writer.flush().await?;
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
            pending_batch: Vec::with_capacity(batch_capacity),
        })
    }

    async fn find_latest_segment(path: &Path) -> Result<(u64, u64, u64)> {
        let mut latest_segment = 0u64;
        let latest_sequence = 0u64;
        let mut created_at = 0u64;
        
        if path.exists() {
            if let Ok(mut entries) = fs::read_dir(path).await {
                while let Some(entry) = entries.next_entry().await? {
                    let file_name = entry.file_name();
                    let name = file_name.to_string_lossy();
                    
                    if let Some(segment) = name.strip_prefix("segment-").and_then(|s| s.parse::<u64>().ok()) {
                        if segment > latest_segment {
                            latest_segment = segment;
                            if let Ok(metadata) = entry.metadata().await {
                                if let Ok(time) = metadata.created() {
                                    created_at = time.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
                                }
                            }
                        }
                    }
                }
            }
        }
        
        Ok((latest_segment, latest_sequence, created_at))
    }

    async fn count_entries_in_segment(file_path: &Path) -> Result<u64> {
        if !file_path.exists() {
            return Ok(0);
        }
        
        let file = File::open(file_path).await?;
        let metadata = file.metadata().await?;
        let file_size = metadata.len();
        
        if file_size <= WalHeader::SIZE as u64 {
            return Ok(0);
        }
        
        Ok((file_size - WalHeader::SIZE as u64) / 1024)
    }

    fn segment_path(path: &Path, segment: u64) -> PathBuf {
        path.join(format!("segment-{:020}", segment))
    }

    pub async fn write(&mut self, entry: &WalEntry) -> Result<()> {
        let data = entry.encode()?;
        let seq = self.sequence.fetch_add(1, Ordering::SeqCst);
        
        let mut item = Vec::with_capacity(8 + data.len());
        item.extend_from_slice(&seq.to_le_bytes());
        item.extend_from_slice(&data);
        
        self.pending_batch.push(item);
        
        if self.pending_batch.len() >= self.config.batch_size {
            while self.flush_batch().await? {
                self.rotate_inner().await?;
            }
        }
        
        Ok(())
    }

    pub async fn write_batch(&mut self, entries: &[WalEntry]) -> Result<()> {
        for entry in entries {
            let data = entry.encode()?;
            let seq = self.sequence.fetch_add(1, Ordering::SeqCst);
            
            let mut item = Vec::with_capacity(8 + data.len());
            item.extend_from_slice(&seq.to_le_bytes());
            item.extend_from_slice(&data);
            
            self.pending_batch.push(item);
            
            if self.pending_batch.len() >= self.config.batch_size {
                while self.flush_batch().await? {
                    self.rotate_inner().await?;
                }
            }
        }
        
        Ok(())
    }

    pub async fn flush_batch(&mut self) -> Result<bool> {
        if self.pending_batch.is_empty() {
            return Ok(false);
        }
        
        let total_size: usize = self.pending_batch.iter().map(|b| b.len()).sum();
        
        for item in &self.pending_batch {
            self.writer.write_all(item).await?;
        }
        
        self.current_size += total_size as u64;
        self.current_entries += self.pending_batch.len() as u64;
        self.pending_batch.clear();
        
        Ok(self.should_rotate())
    }

    pub async fn sync(&mut self) -> Result<()> {
        while self.flush_batch().await? {
            self.rotate_inner().await?;
        }
        self.writer.flush().await?;
        self.writer.get_ref().sync_all().await?;
        Ok(())
    }

    fn should_rotate(&self) -> bool {
        if self.current_size >= self.config.max_segment_size {
            tracing::debug!("Rotating WAL segment: size limit reached");
            return true;
        }
        
        if self.current_entries >= self.config.max_segment_entries {
            tracing::debug!("Rotating WAL segment: entry limit reached");
            return true;
        }
        
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

    pub async fn rotate(&mut self) -> Result<()> {
        while self.flush_batch().await? {
            self.rotate_inner().await?;
        }
        self.writer.flush().await?;
        self.writer.get_ref().sync_all().await?;
        self.rotate_inner().await
    }

    async fn rotate_inner(&mut self) -> Result<()> {
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
            .open(&file_path)
            .await?;
        
        let header = WalHeader::new(self.sequence.load(Ordering::SeqCst));
        let header_data = header.encode();
        
        let mut writer = BufWriter::with_capacity(self.config.buffer_size, file);
        writer.write_all(&header_data).await?;
        writer.flush().await?;
        
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

    pub fn pending_count(&self) -> usize {
        self.pending_batch.len()
    }
}

pub struct SharedAsyncWalWriter {
    inner: Arc<Mutex<AsyncWalWriter>>,
}

impl SharedAsyncWalWriter {
    pub async fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let writer = AsyncWalWriter::new(path).await?;
        Ok(Self {
            inner: Arc::new(Mutex::new(writer)),
        })
    }

    pub async fn write(&self, entry: &WalEntry) -> Result<()> {
        let mut writer = self.inner.lock().await;
        writer.write(entry).await
    }

    pub async fn write_batch(&self, entries: &[WalEntry]) -> Result<()> {
        let mut writer = self.inner.lock().await;
        writer.write_batch(entries).await
    }

    pub async fn sync(&self) -> Result<()> {
        let mut writer = self.inner.lock().await;
        writer.sync().await
    }

    pub async fn flush(&self) -> Result<()> {
        let mut writer = self.inner.lock().await;
        let _ = writer.flush_batch().await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Label, Sample};
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_async_wal_writer_basic() {
        let temp_dir = tempdir().unwrap();
        let mut writer = AsyncWalWriter::new(temp_dir.path()).await.unwrap();
        
        let labels = vec![Label::new("job", "test")];
        let samples = vec![Sample::new(1000, 1.0)];
        let entry = WalEntry::write(1, labels, samples);
        
        writer.write(&entry).await.unwrap();
        writer.sync().await.unwrap();
        
        assert!(writer.current_sequence() > 0);
    }

    #[tokio::test]
    async fn test_async_wal_writer_batch() {
        let temp_dir = tempdir().unwrap();
        let mut writer = AsyncWalWriter::with_config(temp_dir.path(), AsyncWalConfig {
            batch_size: 5,
            max_segment_entries: 100000,
            max_segment_duration_secs: 0,
            ..Default::default()
        }).await.unwrap();
        
        for i in 0..10 {
            let labels = vec![Label::new("job", format!("test_{}", i))];
            let samples = vec![Sample::new(1000 + i as i64, i as f64)];
            let entry = WalEntry::write(i as u64, labels, samples);
            writer.write(&entry).await.unwrap();
        }
        
        writer.sync().await.unwrap();
        
        assert_eq!(writer.pending_count(), 0);
        assert_eq!(writer.current_entries(), 10);
    }

    #[tokio::test]
    async fn test_shared_async_wal_writer() {
        let temp_dir = tempdir().unwrap();
        let writer = SharedAsyncWalWriter::new(temp_dir.path()).await.unwrap();
        
        let labels = vec![Label::new("job", "test")];
        let samples = vec![Sample::new(1000, 1.0)];
        let entry = WalEntry::write(1, labels, samples);
        
        writer.write(&entry).await.unwrap();
        writer.sync().await.unwrap();
    }
}
