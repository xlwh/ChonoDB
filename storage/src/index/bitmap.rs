use crate::model::TimeSeriesId;
use std::collections::HashMap;

/// 位图索引
pub struct BitmapIndex {
    /// 标签名 -> 标签值 -> 位图
    index: HashMap<String, HashMap<String, RoaringBitmap>>,
    /// 系列总数
    series_count: usize,
}

/// 简化版的位图实现
#[derive(Debug, Clone)]
pub struct RoaringBitmap {
    bits: Vec<u64>,
}

impl RoaringBitmap {
    pub fn new() -> Self {
        Self { bits: Vec::new() }
    }

    pub fn add(&mut self, value: u32) {
        let index = (value / 64) as usize;
        let bit = value % 64;
        
        if index >= self.bits.len() {
            self.bits.resize(index + 1, 0);
        }
        
        self.bits[index] |= 1u64 << bit;
    }

    pub fn contains(&self, value: u32) -> bool {
        let index = (value / 64) as usize;
        let bit = value % 64;
        
        if index >= self.bits.len() {
            return false;
        }
        
        (self.bits[index] >> bit) & 1 == 1
    }

    pub fn and(&self, other: &RoaringBitmap) -> RoaringBitmap {
        let min_len = self.bits.len().min(other.bits.len());
        let mut result = RoaringBitmap::new();
        result.bits = vec![0u64; min_len];
        
        for i in 0..min_len {
            result.bits[i] = self.bits[i] & other.bits[i];
        }
        
        result
    }

    pub fn or(&self, other: &RoaringBitmap) -> RoaringBitmap {
        let max_len = self.bits.len().max(other.bits.len());
        let mut result = RoaringBitmap::new();
        result.bits = vec![0u64; max_len];
        
        for i in 0..max_len {
            let left = if i < self.bits.len() { self.bits[i] } else { 0 };
            let right = if i < other.bits.len() { other.bits[i] } else { 0 };
            result.bits[i] = left | right;
        }
        
        result
    }

    pub fn not(&self, max_value: u32) -> RoaringBitmap {
        let max_index = (max_value / 64 + 1) as usize;
        let mut result = RoaringBitmap::new();
        result.bits = vec![!0u64; max_index];
        
        for i in 0..self.bits.len().min(max_index) {
            result.bits[i] = !self.bits[i];
        }
        
        // 清除超出max_value的位
        let last_bit = max_value % 64;
        if max_index > 0 {
            result.bits[max_index - 1] &= (1u64 << (last_bit + 1)) - 1;
        }
        
        result
    }

    pub fn iter(&self) -> BitmapIterator<'_> {
        BitmapIterator {
            bitmap: self,
            current_index: 0,
            current_bit: 0,
        }
    }

    pub fn cardinality(&self) -> usize {
        self.bits.iter().map(|&b| b.count_ones() as usize).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.bits.iter().all(|&b| b == 0)
    }
}

impl Default for RoaringBitmap {
    fn default() -> Self {
        Self::new()
    }
}

pub struct BitmapIterator<'a> {
    bitmap: &'a RoaringBitmap,
    current_index: usize,
    current_bit: u32,
}

impl<'a> Iterator for BitmapIterator<'a> {
    type Item = u32;

    fn next(&mut self) -> Option<Self::Item> {
        while self.current_index < self.bitmap.bits.len() {
            let word = self.bitmap.bits[self.current_index];
            
            while self.current_bit < 64 {
                let bit_mask = 1u64 << self.current_bit;
                if word & bit_mask != 0 {
                    let result = (self.current_index as u32) * 64 + self.current_bit;
                    self.current_bit += 1;
                    return Some(result);
                }
                self.current_bit += 1;
            }
            
            self.current_index += 1;
            self.current_bit = 0;
        }
        
        None
    }
}

impl BitmapIndex {
    pub fn new() -> Self {
        Self {
            index: HashMap::new(),
            series_count: 0,
        }
    }

    /// 添加系列到索引
    pub fn add_series(&mut self, series_id: TimeSeriesId, labels: &[(String, String)]) {
        let id = series_id as u32;
        
        for (name, value) in labels {
            let name_index = self.index.entry(name.clone()).or_insert_with(HashMap::new);
            let bitmap = name_index.entry(value.clone()).or_insert_with(RoaringBitmap::new);
            bitmap.add(id);
        }
        
        self.series_count = self.series_count.max(series_id as usize + 1);
    }

    /// 查询标签等于指定值的系列
    pub fn query_equal(&self, name: &str, value: &str) -> RoaringBitmap {
        self.index
            .get(name)
            .and_then(|values| values.get(value))
            .cloned()
            .unwrap_or_else(RoaringBitmap::new)
    }

    /// 查询标签不等于指定值的系列
    pub fn query_not_equal(&self, name: &str, value: &str) -> RoaringBitmap {
        let all = self.get_all_series();
        let equal = self.query_equal(name, value);
        all.and(&equal.not(self.series_count as u32))
    }

    /// 获取所有系列
    fn get_all_series(&self) -> RoaringBitmap {
        let mut all = RoaringBitmap::new();
        for i in 0..self.series_count {
            all.add(i as u32);
        }
        all
    }

    /// 执行AND查询
    pub fn and(&self, bitmaps: &[RoaringBitmap]) -> RoaringBitmap {
        if bitmaps.is_empty() {
            return RoaringBitmap::new();
        }
        
        let mut result = bitmaps[0].clone();
        for bitmap in &bitmaps[1..] {
            result = result.and(bitmap);
        }
        result
    }

    /// 执行OR查询
    pub fn or(&self, bitmaps: &[RoaringBitmap]) -> RoaringBitmap {
        if bitmaps.is_empty() {
            return RoaringBitmap::new();
        }
        
        let mut result = bitmaps[0].clone();
        for bitmap in &bitmaps[1..] {
            result = result.or(bitmap);
        }
        result
    }

    /// 获取统计信息
    pub fn stats(&self) -> BitmapStats {
        let mut total_bitmaps = 0;
        let mut total_series = 0;
        
        for (_name, values) in &self.index {
            total_bitmaps += values.len();
            for (_, bitmap) in values {
                total_series += bitmap.cardinality();
            }
        }
        
        BitmapStats {
            label_count: self.index.len(),
            bitmap_count: total_bitmaps,
            series_count: total_series,
        }
    }
}

impl Default for BitmapIndex {
    fn default() -> Self {
        Self::new()
    }
}

/// 位图统计信息
#[derive(Debug, Clone)]
pub struct BitmapStats {
    pub label_count: usize,
    pub bitmap_count: usize,
    pub series_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_roaring_bitmap() {
        let mut bitmap = RoaringBitmap::new();
        
        bitmap.add(1);
        bitmap.add(10);
        bitmap.add(100);
        
        assert!(bitmap.contains(1));
        assert!(bitmap.contains(10));
        assert!(bitmap.contains(100));
        assert!(!bitmap.contains(2));
        
        assert_eq!(bitmap.cardinality(), 3);
    }

    #[test]
    fn test_bitmap_and() {
        let mut bitmap1 = RoaringBitmap::new();
        bitmap1.add(1);
        bitmap1.add(2);
        bitmap1.add(3);
        
        let mut bitmap2 = RoaringBitmap::new();
        bitmap2.add(2);
        bitmap2.add(3);
        bitmap2.add(4);
        
        let result = bitmap1.and(&bitmap2);
        
        assert!(result.contains(2));
        assert!(result.contains(3));
        assert!(!result.contains(1));
        assert!(!result.contains(4));
    }

    #[test]
    fn test_bitmap_or() {
        let mut bitmap1 = RoaringBitmap::new();
        bitmap1.add(1);
        bitmap1.add(2);
        
        let mut bitmap2 = RoaringBitmap::new();
        bitmap2.add(3);
        bitmap2.add(4);
        
        let result = bitmap1.or(&bitmap2);
        
        assert!(result.contains(1));
        assert!(result.contains(2));
        assert!(result.contains(3));
        assert!(result.contains(4));
    }

    #[test]
    fn test_bitmap_index() {
        let mut index = BitmapIndex::new();
        
        index.add_series(1, &[
            ("job".to_string(), "prometheus".to_string()),
            ("instance".to_string(), "localhost:9090".to_string()),
        ]);
        
        index.add_series(2, &[
            ("job".to_string(), "grafana".to_string()),
            ("instance".to_string(), "localhost:3000".to_string()),
        ]);
        
        let result = index.query_equal("job", "prometheus");
        assert!(result.contains(1));
        assert!(!result.contains(2));
        
        let stats = index.stats();
        assert_eq!(stats.label_count, 2);
        assert_eq!(stats.bitmap_count, 4);
    }
}
