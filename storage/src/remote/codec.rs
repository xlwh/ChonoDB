use crate::error::Result;
use snap::raw::{Decoder, Encoder};

/// Snappy编解码器
pub struct SnappyCodec;

impl SnappyCodec {
    /// 压缩数据
    pub fn compress(data: &[u8]) -> Result<Vec<u8>> {
        let mut encoder = Encoder::new();
        encoder.compress_vec(data)
            .map_err(|e| crate::error::Error::CompressionError(format!("Snappy compression failed: {}", e)))
    }

    /// 解压数据
    pub fn decompress(data: &[u8]) -> Result<Vec<u8>> {
        let mut decoder = Decoder::new();
        decoder.decompress_vec(data)
            .map_err(|e| crate::error::Error::CompressionError(format!("Snappy decompression failed: {}", e)))
    }
}

/// Protobuf编解码器（简化版本，实际应该使用prost）
pub struct ProtoCodec;

impl ProtoCodec {
    /// 编码为protobuf格式（简化实现）
    pub fn encode<T: serde::Serialize>(data: &T) -> Result<Vec<u8>> {
        // 使用JSON作为中间格式，实际应该使用protobuf
        serde_json::to_vec(data)
            .map_err(|e| crate::error::Error::SerializationError(format!("Protobuf encoding failed: {}", e)))
    }

    /// 从protobuf格式解码（简化实现）
    pub fn decode<T: serde::de::DeserializeOwned>(data: &[u8]) -> Result<T> {
        // 使用JSON作为中间格式，实际应该使用protobuf
        serde_json::from_slice(data)
            .map_err(|e| crate::error::Error::SerializationError(format!("Protobuf decoding failed: {}", e)))
    }
}

/// 组合编解码器：先protobuf编码，再snappy压缩
pub struct CompressedProtoCodec;

impl CompressedProtoCodec {
    /// 编码（不压缩）
    pub fn encode<T: serde::Serialize>(data: &T) -> Result<Vec<u8>> {
        ProtoCodec::encode(data)
    }

    /// 解码（不解压）
    pub fn decode<T: serde::de::DeserializeOwned>(data: &[u8]) -> Result<T> {
        ProtoCodec::decode(data)
    }

    /// 编码并压缩
    pub fn encode_and_compress<T: serde::Serialize>(data: &T) -> Result<Vec<u8>> {
        let proto_bytes = ProtoCodec::encode(data)?;
        SnappyCodec::compress(&proto_bytes)
    }

    /// 解压并解码
    pub fn decompress_and_decode<T: serde::de::DeserializeOwned>(data: &[u8]) -> Result<T> {
        let proto_bytes = SnappyCodec::decompress(data)?;
        ProtoCodec::decode(&proto_bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_snappy_codec() {
        let original = b"Hello, World! This is a test string for snappy compression.";
        
        let compressed = SnappyCodec::compress(original).unwrap();
        assert!(!compressed.is_empty());
        
        let decompressed = SnappyCodec::decompress(&compressed).unwrap();
        assert_eq!(decompressed, original);
    }

    #[test]
    fn test_proto_codec() {
        #[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
        struct TestData {
            name: String,
            value: i32,
        }

        let data = TestData {
            name: "test".to_string(),
            value: 42,
        };

        let encoded = ProtoCodec::encode(&data).unwrap();
        let decoded: TestData = ProtoCodec::decode(&encoded).unwrap();
        
        assert_eq!(data, decoded);
    }

    #[test]
    fn test_compressed_proto_codec() {
        #[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
        struct TestData {
            name: String,
            value: i32,
        }

        let data = TestData {
            name: "test".to_string(),
            value: 42,
        };

        let compressed = CompressedProtoCodec::encode_and_compress(&data).unwrap();
        let decoded: TestData = CompressedProtoCodec::decompress_and_decode(&compressed).unwrap();
        
        assert_eq!(data, decoded);
    }
}
