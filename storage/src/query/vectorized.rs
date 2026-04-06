use crate::error::Result;
use crate::model::Sample;

/// 向量化执行引擎
pub struct VectorizedEngine;

impl VectorizedEngine {
    /// 批量处理样本数据 - 使用向量化加速
    pub fn process_batch(samples: &[Sample], operation: &str) -> Vec<f64> {
        match operation {
            "rate" => Self::calculate_rate_batch(samples),
            "sum" => Self::calculate_sum_batch(samples),
            "avg" => Self::calculate_avg_batch(samples),
            "min" => Self::calculate_min_batch(samples),
            "max" => Self::calculate_max_batch(samples),
            _ => samples.iter().map(|s| s.value).collect(),
        }
    }

    /// 向量化rate计算
    fn calculate_rate_batch(samples: &[Sample]) -> Vec<f64> {
        if samples.len() < 2 {
            return vec![];
        }

        let mut result = Vec::with_capacity(samples.len() - 1);
        
        // 批量处理样本对
        for i in 1..samples.len() {
            let prev = &samples[i - 1];
            let curr = &samples[i];
            let time_delta = curr.timestamp - prev.timestamp;
            
            if time_delta > 0 {
                let value_delta = curr.value - prev.value;
                let rate = value_delta / time_delta as f64;
                result.push(rate);
            }
        }

        result
    }

    /// 向量化sum计算
    fn calculate_sum_batch(samples: &[Sample]) -> Vec<f64> {
        if samples.is_empty() {
            return vec![];
        }

        let mut result = Vec::with_capacity(samples.len());
        let mut running_sum = 0.0;

        for sample in samples {
            running_sum += sample.value;
            result.push(running_sum);
        }

        result
    }

    /// 向量化avg计算
    fn calculate_avg_batch(samples: &[Sample]) -> Vec<f64> {
        if samples.is_empty() {
            return vec![];
        }

        let mut result = Vec::with_capacity(samples.len());
        let mut sum = 0.0;

        for (i, sample) in samples.iter().enumerate() {
            sum += sample.value;
            let avg = sum / (i + 1) as f64;
            result.push(avg);
        }

        result
    }

    /// 向量化min计算
    fn calculate_min_batch(samples: &[Sample]) -> Vec<f64> {
        if samples.is_empty() {
            return vec![];
        }

        let mut result = Vec::with_capacity(samples.len());
        let mut current_min = f64::MAX;

        for sample in samples {
            current_min = current_min.min(sample.value);
            result.push(current_min);
        }

        result
    }

    /// 向量化max计算
    fn calculate_max_batch(samples: &[Sample]) -> Vec<f64> {
        if samples.is_empty() {
            return vec![];
        }

        let mut result = Vec::with_capacity(samples.len());
        let mut current_max = f64::MIN;

        for sample in samples {
            current_max = current_max.max(sample.value);
            result.push(current_max);
        }

        result
    }

    /// 批量比较操作
    pub fn compare_batch(samples: &[Sample], threshold: f64, op: &str) -> Vec<bool> {
        let mut result = Vec::with_capacity(samples.len());

        for sample in samples {
            let val = match op {
                ">" => sample.value > threshold,
                ">=" => sample.value >= threshold,
                "<" => sample.value < threshold,
                "<=" => sample.value <= threshold,
                "==" => sample.value == threshold,
                _ => sample.value == threshold,
            };
            result.push(val);
        }

        result
    }

    /// 批量算术运算
    pub fn arithmetic_batch(left: &[f64], right: &[f64], op: &str) -> Vec<f64> {
        let len = left.len().min(right.len());
        let mut result = Vec::with_capacity(len);

        for i in 0..len {
            let val = match op {
                "+" => left[i] + right[i],
                "-" => left[i] - right[i],
                "*" => left[i] * right[i],
                "/" => left[i] / right[i],
                _ => left[i] + right[i],
            };
            result.push(val);
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_samples() -> Vec<Sample> {
        vec![
            Sample::new(1000, 10.0),
            Sample::new(2000, 20.0),
            Sample::new(3000, 30.0),
            Sample::new(4000, 40.0),
            Sample::new(5000, 50.0),
            Sample::new(6000, 60.0),
            Sample::new(7000, 70.0),
            Sample::new(8000, 80.0),
        ]
    }

    #[test]
    fn test_sum_batch() {
        let samples = create_test_samples();
        let result = VectorizedEngine::process_batch(&samples, "sum");
        
        assert!(!result.is_empty());
        let last_sum = result.last().unwrap();
        assert_eq!(*last_sum, 360.0); // 10+20+30+40+50+60+70+80
    }

    #[test]
    fn test_avg_batch() {
        let samples = create_test_samples();
        let result = VectorizedEngine::process_batch(&samples, "avg");
        
        assert!(!result.is_empty());
        let last_avg = result.last().unwrap();
        assert_eq!(*last_avg, 45.0); // (10+20+30+40+50+60+70+80)/8
    }

    #[test]
    fn test_min_batch() {
        let samples = create_test_samples();
        let result = VectorizedEngine::process_batch(&samples, "min");
        
        assert!(!result.is_empty());
        let last_min = result.last().unwrap();
        assert_eq!(*last_min, 10.0);
    }

    #[test]
    fn test_max_batch() {
        let samples = create_test_samples();
        let result = VectorizedEngine::process_batch(&samples, "max");
        
        assert!(!result.is_empty());
        let last_max = result.last().unwrap();
        assert_eq!(*last_max, 80.0);
    }

    #[test]
    fn test_compare_batch() {
        let samples = create_test_samples();
        let result = VectorizedEngine::compare_batch(&samples, 35.0, ">");
        
        assert_eq!(result.len(), samples.len());
        assert_eq!(result, vec![false, false, false, true, true, true, true, true]);
    }

    #[test]
    fn test_arithmetic_batch() {
        let left = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
        let right = vec![10.0, 20.0, 30.0, 40.0, 50.0, 60.0, 70.0, 80.0];
        
        let result = VectorizedEngine::arithmetic_batch(&left, &right, "+");
        assert_eq!(result, vec![11.0, 22.0, 33.0, 44.0, 55.0, 66.0, 77.0, 88.0]);
    }
}
