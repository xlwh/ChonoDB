use crate::error::{Error, Result};
use crate::model::{Labels, Sample, TimeSeriesId};
use serde::{Deserialize, Serialize};

pub const WAL_ENTRY_HEADER_SIZE: usize = 16;
pub const WAL_MAGIC: u32 = 0x43_48_52_4F;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum WalEntryType {
    Write = 1,
    Delete = 2,
    Series = 3,
    Tombstone = 4,
    Checkpoint = 5,
}

impl TryFrom<u8> for WalEntryType {
    type Error = Error;

    fn try_from(value: u8) -> Result<Self> {
        match value {
            1 => Ok(WalEntryType::Write),
            2 => Ok(WalEntryType::Delete),
            3 => Ok(WalEntryType::Series),
            4 => Ok(WalEntryType::Tombstone),
            5 => Ok(WalEntryType::Checkpoint),
            _ => Err(Error::Wal(format!("Invalid WAL entry type: {}", value))),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalEntry {
    pub entry_type: WalEntryType,
    pub series_id: TimeSeriesId,
    pub labels: Option<Labels>,
    pub samples: Vec<Sample>,
    pub timestamp: i64,
}

impl WalEntry {
    pub fn write(series_id: TimeSeriesId, labels: Labels, samples: Vec<Sample>) -> Self {
        Self {
            entry_type: WalEntryType::Write,
            series_id,
            labels: Some(labels),
            samples,
            timestamp: chrono::Utc::now().timestamp_millis(),
        }
    }

    pub fn delete(series_id: TimeSeriesId) -> Self {
        Self {
            entry_type: WalEntryType::Delete,
            series_id,
            labels: None,
            samples: Vec::new(),
            timestamp: chrono::Utc::now().timestamp_millis(),
        }
    }

    pub fn series(series_id: TimeSeriesId, labels: Labels) -> Self {
        Self {
            entry_type: WalEntryType::Series,
            series_id,
            labels: Some(labels),
            samples: Vec::new(),
            timestamp: chrono::Utc::now().timestamp_millis(),
        }
    }

    pub fn checkpoint(sequence: u64) -> Self {
        Self {
            entry_type: WalEntryType::Checkpoint,
            series_id: sequence,
            labels: None,
            samples: Vec::new(),
            timestamp: chrono::Utc::now().timestamp_millis(),
        }
    }

    pub fn encode(&self) -> Result<Vec<u8>> {
        let data = serde_json::to_vec(self)?;
        
        let mut result = Vec::with_capacity(WAL_ENTRY_HEADER_SIZE + data.len());
        
        result.extend_from_slice(&WAL_MAGIC.to_le_bytes());
        result.push(self.entry_type as u8);
        result.extend_from_slice(&[0u8; 3]);
        result.extend_from_slice(&(data.len() as u32).to_le_bytes());
        result.extend_from_slice(&(crc32fast::hash(&data)).to_le_bytes());
        result.extend_from_slice(&data);
        
        Ok(result)
    }

    pub fn decode(data: &[u8]) -> Result<Self> {
        if data.len() < WAL_ENTRY_HEADER_SIZE {
            return Err(Error::Wal("WAL entry too short".to_string()));
        }
        
        let magic = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        if magic != WAL_MAGIC {
            return Err(Error::Wal("Invalid WAL magic number".to_string()));
        }
        
        let entry_type = WalEntryType::try_from(data[4])?;
        let data_len = u32::from_le_bytes([data[8], data[9], data[10], data[11]]) as usize;
        let expected_crc = u32::from_le_bytes([data[12], data[13], data[14], data[15]]);
        
        if data.len() < WAL_ENTRY_HEADER_SIZE + data_len {
            return Err(Error::Wal("WAL entry truncated".to_string()));
        }
        
        let entry_data = &data[WAL_ENTRY_HEADER_SIZE..WAL_ENTRY_HEADER_SIZE + data_len];
        let actual_crc = crc32fast::hash(entry_data);
        
        if expected_crc != actual_crc {
            return Err(Error::Wal("WAL entry CRC mismatch".to_string()));
        }
        
        let entry: WalEntry = serde_json::from_slice(entry_data)?;
        Ok(entry)
    }
}

#[derive(Debug, Clone)]
pub struct WalHeader {
    pub sequence: u64,
    pub created_at: i64,
    pub version: u32,
}

impl WalHeader {
    pub const SIZE: usize = 20;
    pub const VERSION: u32 = 1;

    pub fn new(sequence: u64) -> Self {
        Self {
            sequence,
            created_at: chrono::Utc::now().timestamp_millis(),
            version: Self::VERSION,
        }
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut result = Vec::with_capacity(Self::SIZE);
        result.extend_from_slice(&self.sequence.to_le_bytes());
        result.extend_from_slice(&self.created_at.to_le_bytes());
        result.extend_from_slice(&self.version.to_le_bytes());
        result
    }

    pub fn decode(data: &[u8]) -> Result<Self> {
        if data.len() < Self::SIZE {
            return Err(Error::Wal("WAL header too short".to_string()));
        }
        
        Ok(Self {
            sequence: u64::from_le_bytes([
                data[0], data[1], data[2], data[3],
                data[4], data[5], data[6], data[7],
            ]),
            created_at: i64::from_le_bytes([
                data[8], data[9], data[10], data[11],
                data[12], data[13], data[14], data[15],
            ]),
            version: u32::from_le_bytes([data[16], data[17], data[18], data[19]]),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Label;

    #[test]
    fn test_wal_entry_encode_decode() {
        let labels = vec![Label::new("job", "test")];
        let samples = vec![Sample::new(1000, 1.0)];
        
        let entry = WalEntry::write(1, labels, samples);
        let encoded = entry.encode().unwrap();
        let decoded = WalEntry::decode(&encoded).unwrap();
        
        assert_eq!(entry.entry_type, decoded.entry_type);
        assert_eq!(entry.series_id, decoded.series_id);
        assert_eq!(entry.samples.len(), decoded.samples.len());
    }

    #[test]
    fn test_wal_header() {
        let header = WalHeader::new(12345);
        let encoded = header.encode();
        assert_eq!(encoded.len(), WalHeader::SIZE);
        let decoded = WalHeader::decode(&encoded).unwrap();
        
        assert_eq!(header.sequence, decoded.sequence);
        assert_eq!(header.version, decoded.version);
    }
}
