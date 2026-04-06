use crate::error::{Error, Result};

pub struct ZstdCompressor {
    level: i32,
}

impl ZstdCompressor {
    pub fn new(level: i32) -> Self {
        Self { level }
    }

    pub fn compress(&self, data: &[u8]) -> Result<Vec<u8>> {
        compress(data, self.level)
    }
}

pub struct ZstdDecompressor;

impl ZstdDecompressor {
    pub fn new() -> Self {
        Self
    }

    pub fn decompress(&self, data: &[u8]) -> Result<Vec<u8>> {
        decompress(data)
    }
}

impl Default for ZstdDecompressor {
    fn default() -> Self {
        Self::new()
    }
}

pub fn compress(data: &[u8], level: i32) -> Result<Vec<u8>> {
    zstd::encode_all(data, level)
        .map_err(|e| Error::Compression(format!("ZSTD compression failed: {}", e)))
}

pub fn decompress(data: &[u8]) -> Result<Vec<u8>> {
    zstd::decode_all(data)
        .map_err(|e| Error::Compression(format!("ZSTD decompression failed: {}", e)))
}

pub fn compress_into(data: &[u8], level: i32, output: &mut Vec<u8>) -> Result<usize> {
    let compressed = compress(data, level)?;
    let len = compressed.len();
    output.extend_from_slice(&compressed);
    Ok(len)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zstd_roundtrip() {
        let data = b"Hello, World! This is a test string for compression.";
        
        let compressed = compress(data, 3).unwrap();
        let decompressed = decompress(&compressed).unwrap();
        
        assert_eq!(data.to_vec(), decompressed);
    }

    #[test]
    fn test_zstd_compressor() {
        let compressor = ZstdCompressor::new(3);
        let decompressor = ZstdDecompressor::new();
        
        let data = b"Test data for compression";
        let compressed = compressor.compress(data).unwrap();
        let decompressed = decompressor.decompress(&compressed).unwrap();
        
        assert_eq!(data.to_vec(), decompressed);
    }

    #[test]
    fn test_compression_levels() {
        let data = vec![0u8; 10000];
        
        for level in 1..=19 {
            let compressed = compress(&data, level).unwrap();
            let decompressed = decompress(&compressed).unwrap();
            assert_eq!(data, decompressed);
        }
    }
}
