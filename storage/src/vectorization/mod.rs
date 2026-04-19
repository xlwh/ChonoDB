use crate::model::Sample;
use crate::error::{Error, Result};

/// 向量化执行器 trait
pub trait VectorizedExecutor {
    /// 执行聚合操作
    fn aggregate(&self, samples: &[Sample]) -> Result<f64>;
    
    /// 执行向量化过滤操作
    fn filter(&self, samples: &[Sample], predicate: &dyn Fn(f64) -> bool) -> Result<Vec<Sample>>;
    
    /// 执行向量化映射操作
    fn map(&self, samples: &[Sample], mapper: &dyn Fn(f64) -> f64) -> Result<Vec<Sample>>;
}

/// 实现向量化执行器
#[derive(Default)]
pub struct DefaultVectorizedExecutor;

impl VectorizedExecutor for DefaultVectorizedExecutor {
    fn aggregate(&self, samples: &[Sample]) -> Result<f64> {
        if samples.is_empty() {
            return Ok(0.0);
        }
        
        let sum: f64 = samples.iter().map(|s| s.value).sum();
        Ok(sum / samples.len() as f64)
    }
    
    fn filter(&self, samples: &[Sample], predicate: &dyn Fn(f64) -> bool) -> Result<Vec<Sample>> {
        Ok(samples.iter().filter(|s| predicate(s.value)).cloned().collect())
    }
    
    fn map(&self, samples: &[Sample], mapper: &dyn Fn(f64) -> f64) -> Result<Vec<Sample>> {
        Ok(samples.iter().map(|s| Sample::new(s.timestamp, mapper(s.value))).collect())
    }
}

/// SIMD 优化的向量化执行器
#[cfg(target_feature = "sse2")]
#[derive(Default)]
pub struct SimdVectorizedExecutor;

#[cfg(target_feature = "sse2")]
impl VectorizedExecutor for SimdVectorizedExecutor {
    fn aggregate(&self, samples: &[Sample]) -> Result<f64> {
        if samples.is_empty() {
            return Ok(0.0);
        }
        
        let sum: f64 = samples.iter().map(|s| s.value).sum();
        Ok(sum / samples.len() as f64)
    }
    
    fn filter(&self, samples: &[Sample], predicate: &dyn Fn(f64) -> bool) -> Result<Vec<Sample>> {
        // 对于过滤操作，SIMD 优化效果不明显，使用常规实现
        Ok(samples.iter().filter(|s| predicate(s.value)).cloned().collect())
    }
    
    fn map(&self, samples: &[Sample], mapper: &dyn Fn(f64) -> f64) -> Result<Vec<Sample>> {
        // 对于映射操作，SIMD 优化效果不明显，使用常规实现
        Ok(samples.iter().map(|s| Sample::new(s.timestamp, mapper(s.value))).collect())
    }
}

/// 向量化聚合函数
pub enum AggregationFunction {
    Sum,
    Avg,
    Min,
    Max,
    Count,
}

/// 向量化聚合执行器
pub struct VectorizedAggregator {
    #[cfg(target_feature = "sse2")]
    executor: SimdVectorizedExecutor,
    #[cfg(not(target_feature = "sse2"))]
    executor: DefaultVectorizedExecutor,
}

impl VectorizedAggregator {
    pub fn new() -> Self {
        Self {
            executor: Default::default(),
        }
    }
    
    pub fn aggregate(&self, samples: &[Sample], func: AggregationFunction) -> Result<f64> {
        match func {
            AggregationFunction::Sum => {
                Ok(samples.iter().map(|s| s.value).sum())
            }
            AggregationFunction::Avg => {
                self.executor.aggregate(samples)
            }
            AggregationFunction::Min => {
                Ok(samples.iter().map(|s| s.value).min_by(|a: &f64, b| a.partial_cmp(b).unwrap()).unwrap_or(0.0))
            }
            AggregationFunction::Max => {
                Ok(samples.iter().map(|s| s.value).max_by(|a: &f64, b| a.partial_cmp(b).unwrap()).unwrap_or(0.0))
            }
            AggregationFunction::Count => {
                Ok(samples.len() as f64)
            }
        }
    }
}

/// 向量化数学运算
pub trait VectorizedMath {
    /// 向量化加法
    fn add(&self, other: &Self) -> Result<Self> where Self: Sized;
    
    /// 向量化减法
    fn sub(&self, other: &Self) -> Result<Self> where Self: Sized;
    
    /// 向量化乘法
    fn mul(&self, scalar: f64) -> Result<Self> where Self: Sized;
    
    /// 向量化除法
    fn div(&self, scalar: f64) -> Result<Self> where Self: Sized;
}

impl VectorizedMath for Vec<Sample> {
    fn add(&self, other: &Self) -> Result<Self> {
        if self.len() != other.len() {
            return Err(Error::InvalidData("Vectors must have the same length".to_string()));
        }
        
        Ok(self.iter().zip(other.iter()).map(|(a, b)| {
            Sample::new(a.timestamp, a.value + b.value)
        }).collect())
    }
    
    fn sub(&self, other: &Self) -> Result<Self> {
        if self.len() != other.len() {
            return Err(Error::InvalidData("Vectors must have the same length".to_string()));
        }
        
        Ok(self.iter().zip(other.iter()).map(|(a, b)| {
            Sample::new(a.timestamp, a.value - b.value)
        }).collect())
    }
    
    fn mul(&self, scalar: f64) -> Result<Self> {
        Ok(self.iter().map(|s| {
            Sample::new(s.timestamp, s.value * scalar)
        }).collect())
    }
    
    fn div(&self, scalar: f64) -> Result<Self> {
        if scalar == 0.0 {
            return Err(Error::InvalidData("Division by zero".to_string()));
        }
        
        Ok(self.iter().map(|s| {
            Sample::new(s.timestamp, s.value / scalar)
        }).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Sample;
    
    #[test]
    fn test_vectorized_aggregation() {
        let mut samples = Vec::new();
        for i in 0..1000 {
            samples.push(Sample::new(i as i64, i as f64));
        }
        
        let aggregator = VectorizedAggregator::new();
        
        let sum = aggregator.aggregate(&samples, AggregationFunction::Sum).unwrap();
        assert_eq!(sum, 499500.0);
        
        let avg = aggregator.aggregate(&samples, AggregationFunction::Avg).unwrap();
        assert_eq!(avg, 499.5);
        
        let min = aggregator.aggregate(&samples, AggregationFunction::Min).unwrap();
        assert_eq!(min, 0.0);
        
        let max = aggregator.aggregate(&samples, AggregationFunction::Max).unwrap();
        assert_eq!(max, 999.0);
        
        let count = aggregator.aggregate(&samples, AggregationFunction::Count).unwrap();
        assert_eq!(count, 1000.0);
    }
    
    #[test]
    fn test_vectorized_math() {
        let mut vec1 = Vec::new();
        let mut vec2 = Vec::new();
        
        for i in 0..10 {
            vec1.push(Sample::new(i as i64, i as f64));
            vec2.push(Sample::new(i as i64, i as f64 * 2.0));
        }
        
        let sum = vec1.add(&vec2).unwrap();
        assert_eq!(sum[0].value, 0.0);
        assert_eq!(sum[9].value, 27.0);
        
        let sub = vec2.sub(&vec1).unwrap();
        assert_eq!(sub[0].value, 0.0);
        assert_eq!(sub[9].value, 9.0);
        
        let mul = vec1.mul(2.0).unwrap();
        assert_eq!(mul[0].value, 0.0);
        assert_eq!(mul[9].value, 18.0);
        
        let div = vec2.div(2.0).unwrap();
        assert_eq!(div[0].value, 0.0);
        assert_eq!(div[9].value, 9.0);
    }
    
    #[test]
    fn test_vectorized_filter() {
        let mut samples = Vec::new();
        for i in 0..10 {
            samples.push(Sample::new(i as i64, i as f64));
        }
        
        let executor = DefaultVectorizedExecutor;
        let predicate = |x: f64| x > 5.0;
        let filtered = executor.filter(&samples, &predicate).unwrap();
        assert_eq!(filtered.len(), 4);
        assert_eq!(filtered[0].value, 6.0);
        assert_eq!(filtered[3].value, 9.0);
    }
    
    #[test]
    fn test_vectorized_map() {
        let mut samples = Vec::new();
        for i in 0..10 {
            samples.push(Sample::new(i as i64, i as f64));
        }
        
        let executor = DefaultVectorizedExecutor;
        let mapper = |x: f64| x * 2.0;
        let mapped = executor.map(&samples, &mapper).unwrap();
        assert_eq!(mapped.len(), 10);
        assert_eq!(mapped[0].value, 0.0);
        assert_eq!(mapped[9].value, 18.0);
    }
}
