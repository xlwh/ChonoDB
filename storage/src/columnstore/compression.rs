use crate::error::Result;
use crate::columnstore::block_format::CompressionType;

/// 压缩算法选择器
/// 根据数据特征自动选择最优压缩算法
pub struct CompressionSelector;

/// 压缩统计信息
#[derive(Debug, Clone, Default)]
pub struct CompressionStats {
    pub algorithm: CompressionType,
    pub original_size: usize,
    pub compressed_size: usize,
    pub compression_ratio: f64,
    pub compression_time_ms: f64,
}

impl CompressionSelector {
    /// 根据数据特征选择最优压缩算法
    pub fn select_algorithm(data: &[u8], data_type: DataType) -> CompressionType {
        // 小数据直接使用 Snappy（速度快）
        if data.len() < 1024 {
            return CompressionType::Snappy;
        }

        match data_type {
            DataType::Timestamp => {
                // 时间戳通常是递增的，使用 Delta + Snappy 效果最好
                CompressionType::Snappy
            }
            DataType::Value => {
                // 数值数据根据熵选择
                let entropy = Self::calculate_entropy(data);
                if entropy < 2.0 {
                    // 低熵数据（有规律），Zstd 压缩比更高
                    CompressionType::Zstd
                } else {
                    // 高熵数据（随机），Snappy 速度更快
                    CompressionType::Snappy
                }
            }
            DataType::Label => {
                // 标签通常重复率高，使用 Zstd
                CompressionType::Zstd
            }
            DataType::Generic => {
                // 通用数据，根据大小选择
                if data.len() > 1024 * 1024 {
                    CompressionType::Zstd
                } else {
                    CompressionType::Snappy
                }
            }
        }
    }

    /// 计算数据熵（简化版）
    fn calculate_entropy(data: &[u8]) -> f64 {
        if data.is_empty() {
            return 0.0;
        }

        // 采样计算（避免大数据量时太慢）
        let sample_size = data.len().min(4096);
        let step = data.len() / sample_size.max(1);
        
        let mut freq = [0u64; 256];
        let mut count = 0u64;
        
        for i in (0..data.len()).step_by(step.max(1)) {
            freq[data[i] as usize] += 1;
            count += 1;
        }

        let mut entropy = 0.0;
        for &f in &freq {
            if f > 0 {
                let p = f as f64 / count as f64;
                entropy -= p * p.log2();
            }
        }

        entropy
    }

    /// 压缩数据
    pub fn compress(data: &[u8], algorithm: CompressionType) -> Result<Vec<u8>> {
        match algorithm {
            CompressionType::None => Ok(data.to_vec()),
            CompressionType::Zstd => Self::compress_zstd(data),
            CompressionType::Snappy => Self::compress_snappy(data),
            CompressionType::Lz4 => Self::compress_lz4(data),
        }
    }

    /// 解压数据
    pub fn decompress(data: &[u8], algorithm: CompressionType, uncompressed_size: usize) -> Result<Vec<u8>> {
        match algorithm {
            CompressionType::None => Ok(data.to_vec()),
            CompressionType::Zstd => Self::decompress_zstd(data, uncompressed_size),
            CompressionType::Snappy => Self::decompress_snappy(data),
            CompressionType::Lz4 => Self::decompress_lz4(data, uncompressed_size),
        }
    }

    fn compress_zstd(data: &[u8]) -> Result<Vec<u8>> {
        // 使用 level 1 获得速度和压缩比的平衡
        zstd::encode_all(data, 1).map_err(|e| crate::error::Error::InvalidData(format!("Zstd compression failed: {}", e)))
    }

    fn decompress_zstd(data: &[u8], _uncompressed_size: usize) -> Result<Vec<u8>> {
        zstd::decode_all(data).map_err(|e| crate::error::Error::InvalidData(format!("Zstd decompression failed: {}", e)))
    }

    fn compress_snappy(data: &[u8]) -> Result<Vec<u8>> {
        let mut encoder = snap::raw::Encoder::new();
        encoder.compress_vec(data).map_err(|e| crate::error::Error::InvalidData(format!("Snappy compression failed: {}", e)))
    }

    fn decompress_snappy(data: &[u8]) -> Result<Vec<u8>> {
        let mut decoder = snap::raw::Decoder::new();
        decoder.decompress_vec(data).map_err(|e| crate::error::Error::InvalidData(format!("Snappy decompression failed: {}", e)))
    }

    fn compress_lz4(data: &[u8]) -> Result<Vec<u8>> {
        // 如果没有 lz4 支持，使用 snappy 作为备选
        Self::compress_snappy(data)
    }

    fn decompress_lz4(data: &[u8], uncompressed_size: usize) -> Result<Vec<u8>> {
        // 如果没有 lz4 支持，使用 snappy 作为备选
        Self::decompress_snappy(data)
    }
}

/// 数据类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataType {
    Timestamp,
    Value,
    Label,
    Generic,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compression_selector() {
        // 测试小数据选择 Snappy
        let small_data = vec![1u8; 100];
        let algo = CompressionSelector::select_algorithm(&small_data, DataType::Value);
        assert_eq!(algo, CompressionType::Snappy);

        // 测试时间戳数据选择 Snappy
        let timestamp_data = vec![0u8; 1024];
        let algo = CompressionSelector::select_algorithm(&timestamp_data, DataType::Timestamp);
        assert_eq!(algo, CompressionType::Snappy);
    }

    #[test]
    fn test_snappy_roundtrip() {
        let data = vec![1u8, 2, 3, 4, 5, 100, 200, 255];
        let compressed = CompressionSelector::compress(&data, CompressionType::Snappy).unwrap();
        let decompressed = CompressionSelector::decompress(&compressed, CompressionType::Snappy, data.len()).unwrap();
        assert_eq!(data, decompressed);
    }

    #[test]
    fn test_zstd_roundtrip() {
        let data = vec![1u8, 2, 3, 4, 5, 100, 200, 255];
        let compressed = CompressionSelector::compress(&data, CompressionType::Zstd).unwrap();
        let decompressed = CompressionSelector::decompress(&compressed, CompressionType::Zstd, data.len()).unwrap();
        assert_eq!(data, decompressed);
    }
}
