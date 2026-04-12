use crate::error::{Error, Result};

pub struct DeltaEncoder {
    last_value: Option<i64>,
}

impl DeltaEncoder {
    pub fn new() -> Self {
        Self { last_value: None }
    }

    pub fn encode(&mut self, value: i64) -> Result<Vec<u8>> {
        let delta = match self.last_value {
            Some(last) => {
                let (result, overflow) = value.overflowing_sub(last);
                if overflow {
                    return Err(Error::Overflow("Delta encoding overflow".to_string()));
                }
                result
            }
            None => value,
        };
        self.last_value = Some(value);
        Ok(encode_varint(delta))
    }

    pub fn encode_batch(&mut self, values: &[i64]) -> Result<Vec<u8>> {
        let mut result = Vec::with_capacity(values.len() * 4);
        for value in values {
            result.extend(self.encode(*value)?);
        }
        Ok(result)
    }
}

impl Default for DeltaEncoder {
    fn default() -> Self {
        Self::new()
    }
}

pub struct DeltaDecoder {
    last_value: Option<i64>,
}

impl DeltaDecoder {
    pub fn new() -> Self {
        Self { last_value: None }
    }

    pub fn decode(&mut self, data: &[u8]) -> Result<Vec<i64>> {
        let mut values = Vec::new();
        let mut pos = 0;
        
        while pos < data.len() {
            let (delta, bytes_read) = decode_varint(&data[pos..])?;
            let value = match self.last_value {
                Some(last) => last + delta,
                None => delta,
            };
            self.last_value = Some(value);
            values.push(value);
            pos += bytes_read;
        }
        
        Ok(values)
    }
}

impl Default for DeltaDecoder {
    fn default() -> Self {
        Self::new()
    }
}

pub struct DeltaOfDeltaEncoder {
    last_value: Option<i64>,
    last_delta: Option<i64>,
}

impl DeltaOfDeltaEncoder {
    pub fn new() -> Self {
        Self {
            last_value: None,
            last_delta: None,
        }
    }

    pub fn encode(&mut self, value: i64) -> Result<Vec<u8>> {
        let delta = match self.last_value {
            Some(last) => {
                let (result, overflow) = value.overflowing_sub(last);
                if overflow {
                    return Err(Error::Overflow("Delta encoding overflow".to_string()));
                }
                result
            }
            None => value,
        };
        
        let dod = match self.last_delta {
            Some(last_delta) => {
                let (result, overflow) = delta.overflowing_sub(last_delta);
                if overflow {
                    return Err(Error::Overflow("Delta of delta encoding overflow".to_string()));
                }
                result
            }
            None => delta,
        };
        
        self.last_value = Some(value);
        self.last_delta = Some(delta);
        
        Ok(encode_varint(dod))
    }

    pub fn encode_batch(&mut self, values: &[i64]) -> Result<Vec<u8>> {
        let mut result = Vec::with_capacity(values.len() * 2);
        for value in values {
            result.extend(self.encode(*value)?);
        }
        Ok(result)
    }

    pub fn encode_batch_optimized(&self, values: &[i64]) -> Result<Vec<u8>> {
        if values.is_empty() {
            return Ok(Vec::new());
        }
        
        let mut deltas = Vec::with_capacity(values.len());
        let mut prev_value = values[0];
        deltas.push(values[0]);
        
        for &value in values.iter().skip(1) {
            let delta = value - prev_value;
            deltas.push(delta);
            prev_value = value;
        }
        
        let mut dods = Vec::with_capacity(deltas.len());
        let mut prev_delta = deltas[0];
        dods.push(prev_delta);
        
        for &delta in deltas.iter().skip(1) {
            let dod = delta - prev_delta;
            dods.push(dod);
            prev_delta = delta;
        }
        
        let zigzag_values: Vec<u64> = dods.iter().map(|&d| zigzag_encode(d)).collect();
        let simple8b_encoded = encode_simple8b(&zigzag_values);
        
        let mut result = Vec::with_capacity(simple8b_encoded.len() * 8);
        for &word in &simple8b_encoded {
            result.extend_from_slice(&word.to_le_bytes());
        }
        
        Ok(result)
    }
}

impl Default for DeltaOfDeltaEncoder {
    fn default() -> Self {
        Self::new()
    }
}

pub struct DeltaOfDeltaDecoder {
    last_value: Option<i64>,
    last_delta: Option<i64>,
}

impl DeltaOfDeltaDecoder {
    pub fn new() -> Self {
        Self {
            last_value: None,
            last_delta: None,
        }
    }

    pub fn decode(&mut self, data: &[u8]) -> Result<Vec<i64>> {
        let mut values = Vec::new();
        let mut pos = 0;
        
        while pos < data.len() {
            let (dod, bytes_read) = decode_varint(&data[pos..])?;
            
            let delta = match self.last_delta {
                Some(last_delta) => last_delta + dod,
                None => dod,
            };
            
            let value = match self.last_value {
                Some(last) => last + delta,
                None => delta,
            };
            
            self.last_value = Some(value);
            self.last_delta = Some(delta);
            values.push(value);
            pos += bytes_read;
        }
        
        Ok(values)
    }
}

impl Default for DeltaOfDeltaDecoder {
    fn default() -> Self {
        Self::new()
    }
}

pub fn encode_varint(value: i64) -> Vec<u8> {
    let mut result = Vec::new();
    let mut v = ((value << 1) ^ (value >> 63)) as u64;
    
    while v >= 0x80 {
        result.push((v as u8) | 0x80);
        v >>= 7;
    }
    result.push(v as u8);
    
    result
}

/// ZigZag 编码 - 将有符号整数编码为无符号整数，使小负数也能高效编码
pub fn zigzag_encode(value: i64) -> u64 {
    ((value << 1) ^ (value >> 63)) as u64
}

pub fn zigzag_decode(value: u64) -> i64 {
    ((value >> 1) as i64) ^ (-((value & 1) as i64))
}

/// Simple8b 编码 - 高效编码多个小整数
/// 将多个整数打包到一个 64 位字中
pub fn encode_simple8b(values: &[u64]) -> Vec<u64> {
    let mut result = Vec::new();
    let mut i = 0;
    
    while i < values.len() {
        let (encoded, count) = encode_simple8b_chunk(&values[i..]);
        result.push(encoded);
        i += count;
    }
    
    result
}

fn encode_simple8b_chunk(values: &[u64]) -> (u64, usize) {
    if values.is_empty() {
        return (0, 0);
    }
    
    let max_value = *values.iter().max().unwrap_or(&0);
    
    let (bits_per_value, count) = match max_value {
        0..=1 => (0, 60),
        2..=3 => (1, 30),
        4..=15 => (2, 20),
        16..=255 => (4, 15),
        256..=65535 => (8, 7),
        65536..=4294967295 => (16, 3),
        _ => (32, 1),
    };
    
    let count = count.min(values.len());
    let mut result: u64 = (bits_per_value as u64) << 60;
    
    for j in 0..count {
        result |= (values[j] & ((1u64 << bits_per_value) - 1)) << (j * bits_per_value);
    }
    
    (result, count)
}

pub fn decode_simple8b(encoded: u64) -> Vec<u64> {
    let selector = (encoded >> 60) as u8;
    
    let (bits_per_value, count) = match selector {
        0 => (0, 60),
        1 => (1, 30),
        2 => (2, 20),
        4 => (4, 15),
        8 => (8, 7),
        16 => (16, 3),
        32 => (32, 1),
        _ => return Vec::new(),
    };
    
    let mut result = Vec::with_capacity(count);
    
    for i in 0..count {
        let value = (encoded >> (i * bits_per_value)) & ((1u64 << bits_per_value) - 1);
        result.push(value);
    }
    
    result
}

pub fn decode_varint(data: &[u8]) -> Result<(i64, usize)> {
    let mut result: u64 = 0;
    let mut shift = 0;
    let mut pos = 0;
    
    loop {
        if pos >= data.len() {
            return Err(Error::InvalidData("Unexpected end of varint data".to_string()));
        }
        
        let byte = data[pos];
        pos += 1;
        
        result |= ((byte & 0x7F) as u64) << shift;
        shift += 7;
        
        if byte & 0x80 == 0 {
            break;
        }
        
        if shift >= 64 {
            return Err(Error::InvalidData("Varint too long".to_string()));
        }
    }
    
    let value = ((result >> 1) as i64) ^ (-((result & 1) as i64));
    Ok((value, pos))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_varint_roundtrip() {
        let test_values = vec![0, 1, -1, 127, -127, 128, -128, 255, -255, 1000, -1000];
        
        for value in test_values {
            let encoded = encode_varint(value);
            let (decoded, _) = decode_varint(&encoded).unwrap();
            assert_eq!(value, decoded);
        }
    }

    #[test]
    fn test_delta_encoder() {
        let mut encoder = DeltaEncoder::new();
        let values = vec![100, 105, 110, 120, 130];
        
        let mut encoded = Vec::new();
        for v in &values {
            encoded.extend(encoder.encode(*v).unwrap());
        }
        
        let mut decoder = DeltaDecoder::new();
        let decoded = decoder.decode(&encoded).unwrap();
        
        assert_eq!(values, decoded);
    }

    #[test]
    fn test_delta_of_delta_encoder() {
        let mut encoder = DeltaOfDeltaEncoder::new();
        let timestamps: Vec<i64> = (0..10).map(|i| 1000 + i * 10).collect();
        
        let mut encoded = Vec::new();
        for ts in &timestamps {
            encoded.extend(encoder.encode(*ts).unwrap());
        }
        
        let mut decoder = DeltaOfDeltaDecoder::new();
        let decoded = decoder.decode(&encoded).unwrap();
        
        assert_eq!(timestamps, decoded);
    }
}
