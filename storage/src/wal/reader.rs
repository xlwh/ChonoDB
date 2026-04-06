use crate::error::{Error, Result};
use crate::wal::entry::{WalEntry, WalHeader};
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};

pub struct WalReader {
    path: PathBuf,
    segments: Vec<u64>,
}

impl WalReader {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let segments = Self::list_segments(&path)?;
        
        Ok(Self { path, segments })
    }

    fn list_segments(path: &Path) -> Result<Vec<u64>> {
        let mut segments = Vec::new();
        
        if path.exists() {
            for entry in std::fs::read_dir(path)? {
                let entry = entry?;
                let file_name = entry.file_name();
                let name = file_name.to_string_lossy();
                
                if let Some(segment) = name.strip_prefix("segment-").and_then(|s| s.parse::<u64>().ok()) {
                    segments.push(segment);
                }
            }
        }
        
        segments.sort();
        Ok(segments)
    }

    fn segment_path(&self, segment: u64) -> PathBuf {
        self.path.join(format!("segment-{:020}", segment))
    }

    pub fn read_all(&self) -> Result<Vec<WalEntry>> {
        let mut entries = Vec::new();
        
        for segment in &self.segments {
            let segment_entries = self.read_segment(*segment)?;
            entries.extend(segment_entries);
        }
        
        Ok(entries)
    }

    pub fn read_segment(&self, segment: u64) -> Result<Vec<WalEntry>> {
        let file_path = self.segment_path(segment);
        let file = File::open(&file_path)?;
        let mut reader = BufReader::new(file);
        
        let mut header_data = [0u8; WalHeader::SIZE];
        reader.read_exact(&mut header_data)?;
        let _header = WalHeader::decode(&header_data)?;
        
        let mut entries = Vec::new();
        
        loop {
            let mut seq_bytes = [0u8; 8];
            match reader.read_exact(&mut seq_bytes) {
                Ok(_) => {}
                Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
                Err(e) => return Err(Error::from(e)),
            }
            
            let mut header = [0u8; 16];
            reader.read_exact(&mut header)?;
            
            let data_len = u32::from_le_bytes([header[8], header[9], header[10], header[11]]) as usize;
            
            let mut entry_data = vec![0u8; 16 + data_len];
            entry_data[..16].copy_from_slice(&header);
            reader.read_exact(&mut entry_data[16..])?;
            
            match WalEntry::decode(&entry_data) {
                Ok(entry) => entries.push(entry),
                Err(e) => {
                    tracing::warn!("Failed to decode WAL entry: {}", e);
                    break;
                }
            }
        }
        
        Ok(entries)
    }

    pub fn segments(&self) -> &[u64] {
        &self.segments
    }

    pub fn segment_count(&self) -> usize {
        self.segments.len()
    }
}

pub struct WalIterator {
    reader: WalReader,
    current_segment_idx: usize,
    current_entries: Vec<WalEntry>,
    current_entry_idx: usize,
}

impl WalIterator {
    pub fn new(reader: WalReader) -> Result<Self> {
        let segments = reader.segments().to_vec();
        let current_entries = if !segments.is_empty() {
            reader.read_segment(segments[0])?
        } else {
            Vec::new()
        };
        
        Ok(Self {
            reader,
            current_segment_idx: 0,
            current_entries,
            current_entry_idx: 0,
        })
    }
}

impl Iterator for WalIterator {
    type Item = Result<WalEntry>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current_entry_idx < self.current_entries.len() {
            let entry = self.current_entries[self.current_entry_idx].clone();
            self.current_entry_idx += 1;
            return Some(Ok(entry));
        }
        
        self.current_segment_idx += 1;
        let segments = self.reader.segments();
        
        if self.current_segment_idx >= segments.len() {
            return None;
        }
        
        match self.reader.read_segment(segments[self.current_segment_idx]) {
            Ok(entries) => {
                self.current_entries = entries;
                self.current_entry_idx = 0;
                self.next()
            }
            Err(e) => Some(Err(e)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Label, Sample};
    use crate::wal::WalWriter;
    use tempfile::tempdir;

    #[test]
    fn test_wal_read_write() {
        let temp_dir = tempdir().unwrap();
        
        {
            let mut writer = WalWriter::new(temp_dir.path()).unwrap();
            
            for i in 0..5u64 {
                let labels = vec![Label::new("job", format!("test_{}", i))];
                let samples = vec![Sample::new(1000 + i as i64, i as f64)];
                let entry = WalEntry::write(i, labels, samples);
                writer.write(&entry).unwrap();
            }
            
            writer.sync().unwrap();
        }
        
        let reader = WalReader::new(temp_dir.path()).unwrap();
        let entries = reader.read_all().unwrap();
        
        assert_eq!(entries.len(), 5);
    }
}
