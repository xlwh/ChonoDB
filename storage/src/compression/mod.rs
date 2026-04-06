mod delta;
mod dictionary;
mod zstd;
mod prediction;

pub use delta::{DeltaDecoder, DeltaEncoder, DeltaOfDeltaDecoder, DeltaOfDeltaEncoder};
pub use dictionary::{Dictionary, DictionaryBuilder};
pub use zstd::{ZstdCompressor, ZstdDecompressor};
pub use prediction::{PredictionEncoder, DoubleExponentialSmoothing};

use crate::error::Result;

pub trait Encoder<T> {
    fn encode(&mut self, value: T) -> Result<Vec<u8>>;
    fn flush(&mut self) -> Result<Vec<u8>>;
}

pub trait Decoder<T> {
    fn decode(&mut self, data: &[u8]) -> Result<Vec<T>>;
}

pub fn compress_zstd(data: &[u8], level: i32) -> Result<Vec<u8>> {
    zstd::compress(data, level)
}

pub fn decompress_zstd(data: &[u8]) -> Result<Vec<u8>> {
    zstd::decompress(data)
}
