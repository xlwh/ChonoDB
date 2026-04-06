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
            Some(last) => value - last,
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
            Some(last) => value - last,
            None => value,
        };
        
        let dod = match self.last_delta {
            Some(last_delta) => delta - last_delta,
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
