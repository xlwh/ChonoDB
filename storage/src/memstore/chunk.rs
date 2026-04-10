use crate::compression::{DeltaEncoder, DeltaOfDeltaEncoder, DeltaDecoder, DeltaOfDeltaDecoder};
use crate::error::{Error, Result};
use crate::model::{Sample, Timestamp};

const DEFAULT_CHUNK_CAPACITY: usize = 120;

#[derive(Debug, Clone)]
pub struct Chunk {
    timestamps: Vec<Timestamp>,
    values: Vec<f64>,
    min_timestamp: Timestamp,
    max_timestamp: Timestamp,
}

impl Chunk {
    pub fn new() -> Self {
        Self {
            timestamps: Vec::with_capacity(DEFAULT_CHUNK_CAPACITY),
            values: Vec::with_capacity(DEFAULT_CHUNK_CAPACITY),
            min_timestamp: i64::MAX,
            max_timestamp: i64::MIN,
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            timestamps: Vec::with_capacity(capacity),
            values: Vec::with_capacity(capacity),
            min_timestamp: i64::MAX,
            max_timestamp: i64::MIN,
        }
    }

    pub fn add(&mut self, sample: Sample) -> Result<()> {
        if !self.timestamps.is_empty() && sample.timestamp <= self.max_timestamp {
            self.timestamps.push(sample.timestamp);
            self.values.push(sample.value);
            self.sort_by_timestamp();
        } else {
            self.timestamps.push(sample.timestamp);
            self.values.push(sample.value);
        }

        self.min_timestamp = self.min_timestamp.min(sample.timestamp);
        self.max_timestamp = self.max_timestamp.max(sample.timestamp);

        Ok(())
    }

    fn sort_by_timestamp(&mut self) {
        let mut indexed: Vec<(usize, Timestamp, f64)> = self
            .timestamps
            .iter()
            .zip(self.values.iter())
            .enumerate()
            .map(|(i, (&t, &v))| (i, t, v))
            .collect();
        indexed.sort_by_key(|&(_, t, _)| t);
        self.timestamps = indexed.iter().map(|&(_, t, _)| t).collect();
        self.values = indexed.iter().map(|&(_, _, v)| v).collect();
    }

    pub fn is_empty(&self) -> bool {
        self.timestamps.is_empty()
    }

    pub fn len(&self) -> usize {
        self.timestamps.len()
    }

    pub fn is_full(&self) -> bool {
        self.timestamps.len() >= DEFAULT_CHUNK_CAPACITY
    }

    pub fn samples(&self) -> impl Iterator<Item = Sample> + '_ {
        self.timestamps
            .iter()
            .zip(self.values.iter())
            .map(|(&t, &v)| Sample::new(t, v))
    }

    pub fn samples_in_range(&self, start: Timestamp, end: Timestamp) -> Vec<Sample> {
        self.timestamps
            .iter()
            .zip(self.values.iter())
            .filter(|(&t, _)| t >= start && t <= end)
            .map(|(&t, &v)| Sample::new(t, v))
            .collect()
    }

    pub fn min_timestamp(&self) -> Option<Timestamp> {
        if self.is_empty() {
            None
        } else {
            Some(self.min_timestamp)
        }
    }

    pub fn max_timestamp(&self) -> Option<Timestamp> {
        if self.is_empty() {
            None
        } else {
            Some(self.max_timestamp)
        }
    }
}

impl Default for Chunk {
    fn default() -> Self {
        Self::new()
    }
}

pub struct ChunkEncoder {
    time_encoder: DeltaOfDeltaEncoder,
    value_encoder: DeltaEncoder,
}

impl ChunkEncoder {
    pub fn new() -> Self {
        Self {
            time_encoder: DeltaOfDeltaEncoder::new(),
            value_encoder: DeltaEncoder::new(),
        }
    }

    pub fn encode(&mut self, chunk: &Chunk) -> Result<EncodedChunk> {
        let mut time_data = Vec::new();
        let mut value_data = Vec::new();
        
        for ts in &chunk.timestamps {
            time_data.extend(self.time_encoder.encode(*ts)?);
        }
        
        let values_as_int: Vec<i64> = chunk.values.iter().map(|v| v.to_bits() as i64).collect();
        value_data = self.value_encoder.encode_batch(&values_as_int)?;
        
        Ok(EncodedChunk {
            time_data,
            value_data,
            min_timestamp: chunk.min_timestamp,
            max_timestamp: chunk.max_timestamp,
            count: chunk.len(),
        })
    }
}

impl Default for ChunkEncoder {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub struct EncodedChunk {
    pub time_data: Vec<u8>,
    pub value_data: Vec<u8>,
    pub min_timestamp: Timestamp,
    pub max_timestamp: Timestamp,
    pub count: usize,
}

pub struct ChunkDecoder {
    time_decoder: DeltaOfDeltaDecoder,
    value_decoder: DeltaDecoder,
}

impl ChunkDecoder {
    pub fn new() -> Self {
        Self {
            time_decoder: DeltaOfDeltaDecoder::new(),
            value_decoder: DeltaDecoder::new(),
        }
    }

    pub fn decode(&mut self, encoded: &EncodedChunk) -> Result<Chunk> {
        let timestamps = self.time_decoder.decode(&encoded.time_data)?;
        let values_bits = self.value_decoder.decode(&encoded.value_data)?;
        
        let values: Vec<f64> = values_bits.iter().map(|&v| f64::from_bits(v as u64)).collect();
        
        if timestamps.len() != values.len() {
            return Err(Error::InvalidData(
                "Timestamp and value count mismatch".to_string(),
            ));
        }
        
        let mut chunk = Chunk::with_capacity(timestamps.len());
        for (t, v) in timestamps.iter().zip(values.iter()) {
            chunk.add(Sample::new(*t, *v))?;
        }
        
        Ok(chunk)
    }
}

impl Default for ChunkDecoder {
    fn default() -> Self {
        Self::new()
    }
}

pub struct ChunkIterator<'a> {
    chunk: &'a Chunk,
    pos: usize,
}

impl<'a> ChunkIterator<'a> {
    pub fn new(chunk: &'a Chunk) -> Self {
        Self { chunk, pos: 0 }
    }
}

impl<'a> Iterator for ChunkIterator<'a> {
    type Item = Sample;

    fn next(&mut self) -> Option<Self::Item> {
        if self.pos >= self.chunk.timestamps.len() {
            return None;
        }
        
        let sample = Sample::new(
            self.chunk.timestamps[self.pos],
            self.chunk.values[self.pos],
        );
        self.pos += 1;
        Some(sample)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunk_basic() {
        let mut chunk = Chunk::new();
        
        chunk.add(Sample::new(1000, 1.0)).unwrap();
        chunk.add(Sample::new(2000, 2.0)).unwrap();
        chunk.add(Sample::new(3000, 3.0)).unwrap();
        
        assert_eq!(chunk.len(), 3);
        assert_eq!(chunk.min_timestamp(), Some(1000));
        assert_eq!(chunk.max_timestamp(), Some(3000));
    }

    #[test]
    fn test_chunk_range_query() {
        let mut chunk = Chunk::new();
        
        for i in 0..10 {
            chunk.add(Sample::new(i * 1000, i as f64)).unwrap();
        }
        
        let samples = chunk.samples_in_range(3000, 7000);
        assert_eq!(samples.len(), 5);
    }

    #[test]
    fn test_chunk_encode_decode() {
        let mut chunk = Chunk::new();
        for i in 0..100 {
            chunk.add(Sample::new(i * 1000, i as f64 * 1.5)).unwrap();
        }
        
        let mut encoder = ChunkEncoder::new();
        let encoded = encoder.encode(&chunk).unwrap();
        
        let mut decoder = ChunkDecoder::new();
        let decoded = decoder.decode(&encoded).unwrap();
        
        assert_eq!(chunk.len(), decoded.len());
        
        let orig: Vec<_> = chunk.samples().collect();
        let dec: Vec<_> = decoded.samples().collect();
        
        for (o, d) in orig.iter().zip(dec.iter()) {
            assert_eq!(o.timestamp, d.timestamp);
            assert!((o.value - d.value).abs() < f64::EPSILON);
        }
    }

    #[test]
    fn test_chunk_out_of_order_timestamps() {
        let mut chunk = Chunk::new();

        chunk.add(Sample::new(3000, 3.0)).unwrap();
        chunk.add(Sample::new(1000, 1.0)).unwrap();
        chunk.add(Sample::new(2000, 2.0)).unwrap();

        assert_eq!(chunk.len(), 3);
        assert_eq!(chunk.min_timestamp(), Some(1000));
        assert_eq!(chunk.max_timestamp(), Some(3000));

        let samples: Vec<_> = chunk.samples().collect();
        assert_eq!(samples[0].timestamp, 1000);
        assert_eq!(samples[0].value, 1.0);
        assert_eq!(samples[1].timestamp, 2000);
        assert_eq!(samples[1].value, 2.0);
        assert_eq!(samples[2].timestamp, 3000);
        assert_eq!(samples[2].value, 3.0);
    }

    #[test]
    fn test_chunk_duplicate_timestamp() {
        let mut chunk = Chunk::new();

        chunk.add(Sample::new(1000, 1.0)).unwrap();
        chunk.add(Sample::new(1000, 2.0)).unwrap();
        chunk.add(Sample::new(2000, 3.0)).unwrap();

        assert_eq!(chunk.len(), 3);
    }
}
