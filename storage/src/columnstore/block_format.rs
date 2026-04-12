use crate::error::Result;
use crate::model::{Label, Sample};
use bytes::{Buf, BufMut, Bytes, BytesMut};

/// ChronoDB Block Format Version
pub const BLOCK_FORMAT_VERSION: u8 = 1;

/// Block Header
#[derive(Debug, Clone)]
pub struct BlockHeader {
    pub version: u8,
    pub block_type: BlockType,
    pub compression: CompressionType,
    pub series_count: u32,
    pub sample_count: u32,
    pub min_timestamp: i64,
    pub max_timestamp: i64,
    pub metadata_offset: u64,
    pub metadata_size: u64,
    pub checksum: u32,
}

impl BlockHeader {
    pub const SIZE: usize = 64;

    pub fn new(block_type: BlockType) -> Self {
        Self {
            version: BLOCK_FORMAT_VERSION,
            block_type,
            compression: CompressionType::Zstd,
            series_count: 0,
            sample_count: 0,
            min_timestamp: 0,
            max_timestamp: 0,
            metadata_offset: 0,
            metadata_size: 0,
            checksum: 0,
        }
    }

    pub fn encode(&self) -> Bytes {
        let mut buf = BytesMut::with_capacity(Self::SIZE);
        buf.put_u8(self.version);
        buf.put_u8(self.block_type as u8);
        buf.put_u8(self.compression as u8);
        buf.put_u8(0); // reserved
        buf.put_u32(self.series_count);
        buf.put_u32(self.sample_count);
        buf.put_i64(self.min_timestamp);
        buf.put_i64(self.max_timestamp);
        buf.put_u64(self.metadata_offset);
        buf.put_u64(self.metadata_size);
        buf.put_u32(self.checksum);
        buf.put_bytes(0, Self::SIZE - buf.len()); // padding
        buf.freeze()
    }

    pub fn decode(data: &[u8]) -> Result<Self> {
        if data.len() < Self::SIZE {
            return Err(crate::error::Error::InvalidData("Block header too short".to_string()));
        }

        let mut buf = data;
        let version = buf.get_u8();
        let block_type = BlockType::from(buf.get_u8());
        let compression = CompressionType::from(buf.get_u8());
        let _reserved = buf.get_u8(); // skip reserved byte
        
        Ok(Self {
            version,
            block_type,
            compression,
            series_count: buf.get_u32(),
            sample_count: buf.get_u32(),
            min_timestamp: buf.get_i64(),
            max_timestamp: buf.get_i64(),
            metadata_offset: buf.get_u64(),
            metadata_size: buf.get_u64(),
            checksum: buf.get_u32(),
        })
    }
}

/// Block Type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum BlockType {
    Data = 0,
    Index = 1,
    Metadata = 2,
    Downsample = 3,
}

impl From<u8> for BlockType {
    fn from(v: u8) -> Self {
        match v {
            0 => BlockType::Data,
            1 => BlockType::Index,
            2 => BlockType::Metadata,
            3 => BlockType::Downsample,
            _ => BlockType::Data,
        }
    }
}

/// Compression Type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum CompressionType {
    #[default]
    None = 0,
    Zstd = 1,
    Snappy = 2,
    Lz4 = 3,
}

impl From<u8> for CompressionType {
    fn from(v: u8) -> Self {
        match v {
            0 => CompressionType::None,
            1 => CompressionType::Zstd,
            2 => CompressionType::Snappy,
            3 => CompressionType::Lz4,
            _ => CompressionType::Zstd,
        }
    }
}

/// Column Data
#[derive(Debug, Clone)]
pub struct ColumnData {
    pub column_id: u32,
    pub column_type: ColumnType,
    pub data: Bytes,
    pub compression: CompressionType,
    pub uncompressed_size: u64,
}

impl ColumnData {
    pub fn encode(&self) -> Bytes {
        let mut buf = BytesMut::new();
        buf.put_u32(self.column_id);
        buf.put_u8(self.column_type as u8);
        buf.put_u8(self.compression as u8);
        buf.put_u64(self.uncompressed_size);
        buf.put_u64(self.data.len() as u64);
        buf.extend_from_slice(&self.data);
        buf.freeze()
    }
}

/// Column Type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ColumnType {
    Timestamp = 0,
    Value = 1,
    Label = 2,
    SeriesId = 3,
}

impl From<u8> for ColumnType {
    fn from(v: u8) -> Self {
        match v {
            0 => ColumnType::Timestamp,
            1 => ColumnType::Value,
            2 => ColumnType::Label,
            3 => ColumnType::SeriesId,
            _ => ColumnType::Value,
        }
    }
}

/// Block Builder
pub struct BlockBuilder {
    header: BlockHeader,
    columns: Vec<ColumnData>,
    series_data: Vec<SeriesBlockData>,
}

impl BlockBuilder {
    pub fn new(block_type: BlockType) -> Self {
        Self {
            header: BlockHeader::new(block_type),
            columns: Vec::new(),
            series_data: Vec::new(),
        }
    }

    pub fn add_series(&mut self, series_id: u64, samples: Vec<Sample>, labels: Vec<Label>) {
        self.series_data.push(SeriesBlockData {
            series_id,
            samples,
            labels,
        });
    }

    pub fn build(mut self) -> Result<Bytes> {
        // Build columns
        self.build_columns()?;

        // Update header
        self.header.series_count = self.series_data.len() as u32;
        self.header.sample_count = self.series_data.iter()
            .map(|s| s.samples.len() as u32)
            .sum();

        if !self.series_data.is_empty() {
            self.header.min_timestamp = self.series_data.iter()
                .flat_map(|s| s.samples.iter().map(|sample| sample.timestamp))
                .min()
                .unwrap_or(0);
            self.header.max_timestamp = self.series_data.iter()
                .flat_map(|s| s.samples.iter().map(|sample| sample.timestamp))
                .max()
                .unwrap_or(0);
        }

        // Encode block
        let mut buf = BytesMut::new();
        buf.extend_from_slice(&self.header.encode());

        for column in &self.columns {
            buf.extend_from_slice(&column.encode());
        }

        Ok(buf.freeze())
    }

    fn build_columns(&mut self) -> Result<()> {
        // Build timestamp column
        let timestamps: Vec<i64> = self.series_data.iter()
            .flat_map(|s| s.samples.iter().map(|sample| sample.timestamp))
            .collect();

        let timestamp_data = self.encode_timestamps(&timestamps)?;
        self.columns.push(ColumnData {
            column_id: 0,
            column_type: ColumnType::Timestamp,
            data: timestamp_data,
            compression: CompressionType::Zstd,
            uncompressed_size: (timestamps.len() * 8) as u64,
        });

        // Build value column
        let values: Vec<f64> = self.series_data.iter()
            .flat_map(|s| s.samples.iter().map(|sample| sample.value))
            .collect();

        let value_data = self.encode_values(&values)?;
        self.columns.push(ColumnData {
            column_id: 1,
            column_type: ColumnType::Value,
            data: value_data,
            compression: CompressionType::Zstd,
            uncompressed_size: (values.len() * 8) as u64,
        });

        Ok(())
    }

    fn encode_timestamps(&self, timestamps: &[i64]) -> Result<Bytes> {
        // Delta-of-delta encoding
        if timestamps.len() < 2 {
            let mut buf = BytesMut::new();
            for &ts in timestamps {
                buf.put_i64(ts);
            }
            return Ok(buf.freeze());
        }

        let mut deltas = Vec::with_capacity(timestamps.len() - 1);
        let mut prev_delta = timestamps[1] - timestamps[0];

        for i in 2..timestamps.len() {
            let delta = timestamps[i] - timestamps[i - 1];
            deltas.push(delta - prev_delta);
            prev_delta = delta;
        }

        // Compress deltas
        let mut buf = BytesMut::new();
        buf.put_i64(timestamps[0]);
        buf.put_i64(timestamps[1] - timestamps[0]);

        for delta in deltas {
            // Variable length encoding for small deltas
            if delta >= -128 && delta <= 127 {
                buf.put_i8(delta as i8);
            } else if delta >= -32768 && delta <= 32767 {
                buf.put_u8(0x80);
                buf.put_i16(delta as i16);
            } else {
                buf.put_u8(0x81);
                buf.put_i64(delta);
            }
        }

        Ok(buf.freeze())
    }

    fn encode_values(&self, values: &[f64]) -> Result<Bytes> {
        // XOR-based compression for float values
        let mut buf = BytesMut::new();

        if values.is_empty() {
            return Ok(buf.freeze());
        }

        // Store first value as-is
        buf.put_f64(values[0]);

        if values.len() == 1 {
            return Ok(buf.freeze());
        }

        // XOR compression
        let mut prev_bits = values[0].to_bits();

        for &value in &values[1..] {
            let bits = value.to_bits();
            let xor = prev_bits ^ bits;

            if xor == 0 {
                // Same value, store single 0 bit
                buf.put_u8(0);
            } else {
                // Different value, store 1 bit + XOR value
                buf.put_u8(1);
                buf.put_u64(xor);
            }

            prev_bits = bits;
        }

        Ok(buf.freeze())
    }
}

/// Series Block Data
#[derive(Debug, Clone)]
pub struct SeriesBlockData {
    pub series_id: u64,
    pub samples: Vec<Sample>,
    pub labels: Vec<Label>,
}

/// Block Reader
pub struct BlockReader {
    data: Bytes,
    header: BlockHeader,
}

impl BlockReader {
    pub fn new(data: Bytes) -> Result<Self> {
        let header = BlockHeader::decode(&data)?;
        Ok(Self { data, header })
    }

    pub fn header(&self) -> &BlockHeader {
        &self.header
    }

    pub fn read_timestamps(&self) -> Result<Vec<i64>> {
        // Find timestamp column
        let mut offset = BlockHeader::SIZE;

        while offset < self.data.len() {
            let mut buf = &self.data[offset..];
            let _column_id = buf.get_u32();
            let column_type = ColumnType::from(buf.get_u8());
            let _compression = CompressionType::from(buf.get_u8());
            let _uncompressed_size = buf.get_u64();
            let data_size = buf.get_u64() as usize;

            if column_type == ColumnType::Timestamp {
                let data = &self.data[offset + 22..offset + 22 + data_size];
                return BlockReader::decode_timestamps(data);
            }

            offset += 22 + data_size;
        }

        Err(crate::error::Error::InvalidData("Timestamp column not found".to_string()))
    }

    pub fn decode_timestamps(data: &[u8]) -> Result<Vec<i64>> {
        if data.len() < 16 {
            return Err(crate::error::Error::InvalidData("Timestamp data too short".to_string()));
        }

        let mut buf = data;
        let first = buf.get_i64();
        let first_delta = buf.get_i64();

        let mut timestamps = vec![first, first + first_delta];
        let mut prev_delta = first_delta;

        while !buf.is_empty() {
            let flag = buf.get_u8();

            let delta = if flag == 0x80 {
                buf.get_i16() as i64
            } else if flag == 0x81 {
                buf.get_i64()
            } else {
                flag as i8 as i64
            };

            prev_delta += delta;
            let last = *timestamps.last().unwrap();
            timestamps.push(last + prev_delta);
        }

        Ok(timestamps)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_block_header_encode_decode() {
        let header = BlockHeader::new(BlockType::Data);
        let encoded = header.encode();
        assert_eq!(encoded.len(), BlockHeader::SIZE);

        let decoded = BlockHeader::decode(&encoded).unwrap();
        assert_eq!(decoded.version, BLOCK_FORMAT_VERSION);
        assert_eq!(decoded.block_type, BlockType::Data);
    }

    #[test]
    fn test_block_builder() {
        let mut builder = BlockBuilder::new(BlockType::Data);

        let samples = vec![
            Sample::new(1000, 10.0),
            Sample::new(2000, 20.0),
            Sample::new(3000, 30.0),
        ];

        builder.add_series(1, samples, vec![]);

        let block = builder.build().unwrap();
        assert!(!block.is_empty());
    }

    #[test]
    fn test_timestamp_encoding() {
        let timestamps = vec![1000, 2000, 3000, 4000, 5000];
        let builder = BlockBuilder::new(BlockType::Data);
        let encoded = builder.encode_timestamps(&timestamps).unwrap();

        // 直接测试 decode_timestamps 方法
        let decoded = BlockReader::decode_timestamps(&encoded).unwrap();
        assert_eq!(decoded, timestamps);
    }
}
