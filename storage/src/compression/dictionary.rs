use crate::error::{Error, Result};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct Dictionary {
    strings: Vec<String>,
    indices: HashMap<String, u32>,
}

impl Dictionary {
    pub fn new() -> Self {
        Self {
            strings: Vec::new(),
            indices: HashMap::new(),
        }
    }

    pub fn insert(&mut self, s: &str) -> u32 {
        if let Some(&idx) = self.indices.get(s) {
            return idx;
        }
        
        let idx = self.strings.len() as u32;
        self.strings.push(s.to_string());
        self.indices.insert(s.to_string(), idx);
        idx
    }

    pub fn get(&self, idx: u32) -> Option<&str> {
        self.strings.get(idx as usize).map(|s| s.as_str())
    }

    pub fn get_index(&self, s: &str) -> Option<u32> {
        self.indices.get(s).copied()
    }

    pub fn len(&self) -> usize {
        self.strings.len()
    }

    pub fn is_empty(&self) -> bool {
        self.strings.is_empty()
    }

    pub fn encode(&self, strings: &[String]) -> Result<Vec<u8>> {
        let mut result = Vec::with_capacity(strings.len() * 4);
        
        for s in strings {
            let idx = self.indices.get(s).ok_or_else(|| {
                Error::InvalidData(format!("String not found in dictionary: {}", s))
            })?;
            result.extend_from_slice(&idx.to_le_bytes());
        }
        
        Ok(result)
    }

    pub fn decode(&self, data: &[u8]) -> Result<Vec<String>> {
        if data.len() % 4 != 0 {
            return Err(Error::InvalidData("Invalid dictionary encoded data length".to_string()));
        }
        
        let mut result = Vec::with_capacity(data.len() / 4);
        for chunk in data.chunks(4) {
            let idx = u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
            let s = self.get(idx).ok_or_else(|| {
                Error::InvalidData(format!("Invalid dictionary index: {}", idx))
            })?;
            result.push(s.to_string());
        }
        
        Ok(result)
    }

    pub fn serialize(&self) -> Result<Vec<u8>> {
        let mut result = Vec::new();
        
        let len = self.strings.len() as u32;
        result.extend_from_slice(&len.to_le_bytes());
        
        for s in &self.strings {
            let bytes = s.as_bytes();
            let str_len = bytes.len() as u32;
            result.extend_from_slice(&str_len.to_le_bytes());
            result.extend_from_slice(bytes);
        }
        
        Ok(result)
    }

    pub fn deserialize(data: &[u8]) -> Result<Self> {
        if data.len() < 4 {
            return Err(Error::InvalidData("Dictionary data too short".to_string()));
        }
        
        let len = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;
        let mut dict = Self::new();
        let mut pos = 4;
        
        for _ in 0..len {
            if pos + 4 > data.len() {
                return Err(Error::InvalidData("Unexpected end of dictionary data".to_string()));
            }
            
            let str_len = u32::from_le_bytes([
                data[pos],
                data[pos + 1],
                data[pos + 2],
                data[pos + 3],
            ]) as usize;
            pos += 4;
            
            if pos + str_len > data.len() {
                return Err(Error::InvalidData("Unexpected end of dictionary data".to_string()));
            }
            
            let s = String::from_utf8(data[pos..pos + str_len].to_vec())
                .map_err(|e| Error::InvalidData(format!("Invalid UTF-8 string: {}", e)))?;
            pos += str_len;
            
            dict.insert(&s);
        }
        
        Ok(dict)
    }
}

impl Default for Dictionary {
    fn default() -> Self {
        Self::new()
    }
}

pub struct DictionaryBuilder {
    dictionaries: HashMap<String, Dictionary>,
}

impl DictionaryBuilder {
    pub fn new() -> Self {
        Self {
            dictionaries: HashMap::new(),
        }
    }

    pub fn get_or_create(&mut self, name: &str) -> &mut Dictionary {
        self.dictionaries.entry(name.to_string()).or_insert_with(Dictionary::new)
    }

    pub fn get(&self, name: &str) -> Option<&Dictionary> {
        self.dictionaries.get(name)
    }

    pub fn serialize_all(&self) -> Result<HashMap<String, Vec<u8>>> {
        let mut result = HashMap::new();
        for (name, dict) in &self.dictionaries {
            result.insert(name.clone(), dict.serialize()?);
        }
        Ok(result)
    }
}

impl Default for DictionaryBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dictionary_basic() {
        let mut dict = Dictionary::new();
        
        let idx1 = dict.insert("job");
        let idx2 = dict.insert("instance");
        let idx3 = dict.insert("job");
        
        assert_eq!(idx1, idx3);
        assert_ne!(idx1, idx2);
        
        assert_eq!(dict.get(idx1), Some("job"));
        assert_eq!(dict.get(idx2), Some("instance"));
    }

    #[test]
    fn test_dictionary_serialize() {
        let mut dict = Dictionary::new();
        dict.insert("job");
        dict.insert("instance");
        dict.insert("method");
        
        let serialized = dict.serialize().unwrap();
        let deserialized = Dictionary::deserialize(&serialized).unwrap();
        
        assert_eq!(deserialized.get_index("job"), Some(0));
        assert_eq!(deserialized.get_index("instance"), Some(1));
        assert_eq!(deserialized.get_index("method"), Some(2));
    }

    #[test]
    fn test_dictionary_encode_decode() {
        let mut dict = Dictionary::new();
        let strings = vec!["job".to_string(), "instance".to_string(), "job".to_string()];
        
        for s in &strings {
            dict.insert(s);
        }
        
        let encoded = dict.encode(&strings).unwrap();
        let decoded = dict.decode(&encoded).unwrap();
        
        assert_eq!(strings, decoded);
    }
}
