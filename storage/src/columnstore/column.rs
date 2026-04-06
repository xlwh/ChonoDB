use crate::compression::{compress_zstd, decompress_zstd, DeltaEncoder, DeltaOfDeltaEncoder};
use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ColumnType {
    Timestamp,
    Value,
    Label,
    DownsampleL1,
    DownsampleL2,
    DownsampleL3,
    DownsampleL4,
}

impl ColumnType {
    pub fn file_extension(&self) -> &'static str {
        match self {
            ColumnType::Timestamp => "time.col",
            ColumnType::Value => "value.col",
            ColumnType::Label => "labels.col",
            ColumnType::DownsampleL1 => "downsample_L1.col",
            ColumnType::DownsampleL2 => "downsample_L2.col",
            ColumnType::DownsampleL3 => "downsample_L3.col",
            ColumnType::DownsampleL4 => "downsample_L4.col",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Column {
    pub column_type: ColumnType,
    pub data: Vec<u8>,
    pub uncompressed_size: usize,
    pub compressed_size: usize,
    pub num_values: usize,
}

impl Column {
    pub fn new(column_type: ColumnType) -> Self {
        Self {
            column_type,
            data: Vec::new(),
            uncompressed_size: 0,
            compressed_size: 0,
            num_values: 0,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    pub fn compression_ratio(&self) -> f64 {
        if self.uncompressed_size == 0 {
            return 0.0;
        }
        self.uncompressed_size as f64 / self.compressed_size as f64
    }
}

#[derive(Debug, Clone)]
pub struct ColumnBuilder {
    column_type: ColumnType,
    compression_level: i32,
    timestamps: Vec<i64>,
    values: Vec<f64>,
    labels: Vec<String>,
}

impl ColumnBuilder {
    pub fn timestamps(compression_level: i32) -> Self {
        Self {
            column_type: ColumnType::Timestamp,
            compression_level,
            timestamps: Vec::new(),
            values: Vec::new(),
            labels: Vec::new(),
        }
    }

    pub fn values(compression_level: i32) -> Self {
        Self {
            column_type: ColumnType::Value,
            compression_level,
            timestamps: Vec::new(),
            values: Vec::new(),
            labels: Vec::new(),
        }
    }

    pub fn labels(compression_level: i32) -> Self {
        Self {
            column_type: ColumnType::Label,
            compression_level,
            timestamps: Vec::new(),
            values: Vec::new(),
            labels: Vec::new(),
        }
    }

    pub fn add_timestamp(&mut self, timestamp: i64) {
        self.timestamps.push(timestamp);
    }

    pub fn add_value(&mut self, value: f64) {
        self.values.push(value);
    }

    pub fn add_label(&mut self, label: String) {
        self.labels.push(label);
    }

    pub fn add_timestamps(&mut self, timestamps: &[i64]) {
        self.timestamps.extend(timestamps);
    }

    pub fn add_values(&mut self, values: &[f64]) {
        self.values.extend(values);
    }

    pub fn build(self) -> Result<Column> {
        match self.column_type {
            ColumnType::Timestamp => self.build_timestamp_column(),
            ColumnType::Value => self.build_value_column(),
            ColumnType::Label => self.build_label_column(),
            _ => Err(Error::InvalidData("Unsupported column type".to_string())),
        }
    }

    fn build_timestamp_column(self) -> Result<Column> {
        let mut encoder = DeltaOfDeltaEncoder::new();
        let encoded = encoder.encode_batch(&self.timestamps)?;
        
        let uncompressed_size = self.timestamps.len() * 8;
        let compressed = compress_zstd(&encoded, self.compression_level)?;
        
        Ok(Column {
            column_type: ColumnType::Timestamp,
            data: compressed,
            uncompressed_size,
            compressed_size: encoded.len(),
            num_values: self.timestamps.len(),
        })
    }

    fn build_value_column(self) -> Result<Column> {
        let mut encoder = DeltaEncoder::new();
        let values_as_int: Vec<i64> = self.values.iter().map(|v| v.to_bits() as i64).collect();
        let encoded = encoder.encode_batch(&values_as_int)?;
        
        let uncompressed_size = self.values.len() * 8;
        let compressed = compress_zstd(&encoded, self.compression_level)?;
        
        Ok(Column {
            column_type: ColumnType::Value,
            data: compressed,
            uncompressed_size,
            compressed_size: encoded.len(),
            num_values: self.values.len(),
        })
    }

    fn build_label_column(self) -> Result<Column> {
        let mut data = Vec::new();
        
        for label in &self.labels {
            let bytes = label.as_bytes();
            let len = bytes.len() as u32;
            data.extend_from_slice(&len.to_le_bytes());
            data.extend_from_slice(bytes);
        }
        
        let uncompressed_size = data.len();
        let compressed = compress_zstd(&data, self.compression_level)?;
        
        Ok(Column {
            column_type: ColumnType::Label,
            data: compressed,
            uncompressed_size,
            compressed_size: data.len(),
            num_values: self.labels.len(),
        })
    }

    pub fn len(&self) -> usize {
        match self.column_type {
            ColumnType::Timestamp => self.timestamps.len(),
            ColumnType::Value => self.values.len(),
            ColumnType::Label => self.labels.len(),
            _ => 0,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

pub fn decode_timestamp_column(column: &Column) -> Result<Vec<i64>> {
    let decompressed = decompress_zstd(&column.data)?;
    let mut decoder = DeltaOfDeltaEncoder::new();
    let timestamps = decoder.encode_batch(&decompressed.chunks(8).map(|c| {
        i64::from_le_bytes([c[0], c[1], c[2], c[3], c[4], c[5], c[6], c[7]])
    }).collect::<Vec<_>>())?;
    Ok(timestamps.chunks(8).map(|c| {
        i64::from_le_bytes([c[0], c[1], c[2], c[3], c[4], c[5], c[6], c[7]])
    }).collect())
}

pub fn decode_value_column(column: &Column) -> Result<Vec<f64>> {
    let decompressed = decompress_zstd(&column.data)?;
    let values: Vec<f64> = decompressed
        .chunks(8)
        .map(|c| f64::from_bits(u64::from_le_bytes([c[0], c[1], c[2], c[3], c[4], c[5], c[6], c[7]])))
        .collect();
    Ok(values)
}

pub fn decode_label_column(column: &Column) -> Result<Vec<String>> {
    let decompressed = decompress_zstd(&column.data)?;
    let mut labels = Vec::new();
    let mut pos = 0;
    
    while pos < decompressed.len() {
        if pos + 4 > decompressed.len() {
            break;
        }
        
        let len = u32::from_le_bytes([
            decompressed[pos],
            decompressed[pos + 1],
            decompressed[pos + 2],
            decompressed[pos + 3],
        ]) as usize;
        pos += 4;
        
        if pos + len > decompressed.len() {
            break;
        }
        
        let label = String::from_utf8(decompressed[pos..pos + len].to_vec())
            .map_err(|e| Error::InvalidData(format!("Invalid UTF-8: {}", e)))?;
        labels.push(label);
        pos += len;
    }
    
    Ok(labels)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timestamp_column() {
        let mut builder = ColumnBuilder::timestamps(3);
        
        for i in 0..100 {
            builder.add_timestamp(1000 + i * 10);
        }
        
        let column = builder.build().unwrap();
        
        assert_eq!(column.num_values, 100);
        assert!(column.compression_ratio() > 1.0);
    }

    #[test]
    fn test_value_column() {
        let mut builder = ColumnBuilder::values(3);
        
        for i in 0..100 {
            builder.add_value(i as f64 * 1.5);
        }
        
        let column = builder.build().unwrap();
        
        assert_eq!(column.num_values, 100);
        assert!(column.compression_ratio() > 1.0);
    }

    #[test]
    fn test_label_column() {
        let mut builder = ColumnBuilder::labels(3);
        
        builder.add_label("job=prometheus".to_string());
        builder.add_label("instance=localhost:9090".to_string());
        
        let column = builder.build().unwrap();
        
        assert_eq!(column.num_values, 2);
    }
}
