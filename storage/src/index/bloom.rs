use crate::error::Result;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

const LN_2: f64 = std::f64::consts::LN_2;

#[derive(Debug, Clone)]
pub struct BloomFilter {
    bits: Vec<u8>,
    num_bits: usize,
    num_hashes: usize,
}

impl BloomFilter {
    pub fn new(expected_items: usize, false_positive_rate: f64) -> Self {
        let num_bits = Self::optimal_num_bits(expected_items, false_positive_rate);
        let num_hashes = Self::optimal_num_hashes(num_bits, expected_items);
        let num_bytes = (num_bits + 7) / 8;
        
        Self {
            bits: vec![0u8; num_bytes],
            num_bits,
            num_hashes,
        }
    }

    pub fn with_size(num_bits: usize, num_hashes: usize) -> Self {
        let num_bytes = (num_bits + 7) / 8;
        Self {
            bits: vec![0u8; num_bytes],
            num_bits,
            num_hashes,
        }
    }

    fn optimal_num_bits(n: usize, p: f64) -> usize {
        let m = -((n as f64) * p.ln() / (LN_2 * LN_2));
        m.ceil() as usize
    }

    fn optimal_num_hashes(m: usize, n: usize) -> usize {
        let k = ((m as f64) / (n as f64) * LN_2).ceil() as usize;
        k.max(1)
    }

    pub fn insert(&mut self, item: &[u8]) {
        let (hash1, hash2) = self.hashes(item);
        
        for i in 0..self.num_hashes {
            let index = self.bit_index(hash1, hash2, i);
            self.set_bit(index);
        }
    }

    pub fn contains(&self, item: &[u8]) -> bool {
        let (hash1, hash2) = self.hashes(item);
        
        for i in 0..self.num_hashes {
            let index = self.bit_index(hash1, hash2, i);
            if !self.get_bit(index) {
                return false;
            }
        }
        true
    }

    fn hashes(&self, item: &[u8]) -> (u64, u64) {
        let mut hasher1 = DefaultHasher::new();
        item.hash(&mut hasher1);
        let hash1 = hasher1.finish();
        
        let mut hasher2 = DefaultHasher::new();
        hash1.hash(&mut hasher2);
        let hash2 = hasher2.finish();
        
        (hash1, hash2)
    }

    fn bit_index(&self, hash1: u64, hash2: u64, i: usize) -> usize {
        let combined = hash1.wrapping_add((i as u64).wrapping_mul(hash2));
        (combined as usize) % self.num_bits
    }

    fn set_bit(&mut self, index: usize) {
        let byte_index = index / 8;
        let bit_offset = index % 8;
        self.bits[byte_index] |= 1 << bit_offset;
    }

    fn get_bit(&self, index: usize) -> bool {
        let byte_index = index / 8;
        let bit_offset = index % 8;
        (self.bits[byte_index] >> bit_offset) & 1 == 1
    }

    pub fn clear(&mut self) {
        self.bits.fill(0);
    }

    pub fn union(&mut self, other: &BloomFilter) -> Result<()> {
        if self.num_bits != other.num_bits {
            return Err(crate::error::Error::InvalidData(
                "Bloom filters have different sizes".to_string(),
            ));
        }
        
        for i in 0..self.bits.len() {
            self.bits[i] |= other.bits[i];
        }
        Ok(())
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut result = Vec::with_capacity(16 + self.bits.len());
        result.extend_from_slice(&(self.num_bits as u64).to_le_bytes());
        result.extend_from_slice(&(self.num_hashes as u64).to_le_bytes());
        result.extend_from_slice(&self.bits);
        result
    }

    pub fn deserialize(data: &[u8]) -> Result<Self> {
        if data.len() < 16 {
            return Err(crate::error::Error::InvalidData(
                "Bloom filter data too short".to_string(),
            ));
        }
        
        let num_bits = u64::from_le_bytes([
            data[0], data[1], data[2], data[3],
            data[4], data[5], data[6], data[7],
        ]) as usize;
        
        let num_hashes = u64::from_le_bytes([
            data[8], data[9], data[10], data[11],
            data[12], data[13], data[14], data[15],
        ]) as usize;
        
        let bits = data[16..].to_vec();
        
        Ok(Self {
            bits,
            num_bits,
            num_hashes,
        })
    }

    pub fn estimated_count(&self) -> usize {
        let bits_set = self.bits.iter().map(|b| b.count_ones() as usize).sum::<usize>();
        if bits_set == 0 {
            return 0;
        }
        
        let m = self.num_bits as f64;
        let k = self.num_hashes as f64;
        let x = bits_set as f64;
        
        ((m / k) * (-((m - x) / m).ln())).ceil() as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bloom_filter_basic() {
        let mut filter = BloomFilter::new(1000, 0.01);
        
        filter.insert(b"test_key_1");
        filter.insert(b"test_key_2");
        
        assert!(filter.contains(b"test_key_1"));
        assert!(filter.contains(b"test_key_2"));
        assert!(!filter.contains(b"test_key_3"));
    }

    #[test]
    fn test_bloom_filter_serialize() {
        let mut filter = BloomFilter::new(1000, 0.01);
        filter.insert(b"test_key");
        
        let serialized = filter.serialize();
        let deserialized = BloomFilter::deserialize(&serialized).unwrap();
        
        assert!(deserialized.contains(b"test_key"));
        assert!(!deserialized.contains(b"other_key"));
    }

    #[test]
    fn test_bloom_filter_false_positive_rate() {
        let n = 10000;
        let p = 0.01;
        let mut filter = BloomFilter::new(n, p);
        
        for i in 0..n {
            filter.insert(format!("key_{}", i).as_bytes());
        }
        
        let mut false_positives = 0;
        let test_count = 10000;
        for i in n..(n + test_count) {
            if filter.contains(format!("key_{}", i).as_bytes()) {
                false_positives += 1;
            }
        }
        
        let actual_rate = false_positives as f64 / test_count as f64;
        assert!(actual_rate < p * 2.0, "False positive rate {} exceeds threshold", actual_rate);
    }
}
