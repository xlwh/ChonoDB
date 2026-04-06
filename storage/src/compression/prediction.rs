use crate::error::Result;
use crate::model::Sample;

/// 预测编码器
pub struct PredictionEncoder;

impl PredictionEncoder {
    /// 使用线性预测编码样本值
    pub fn encode(samples: &[Sample]) -> Result<Vec<f64>> {
        if samples.len() < 2 {
            return Ok(samples.iter().map(|s| s.value).collect());
        }

        let mut residuals = Vec::with_capacity(samples.len());
        
        // 第一个值直接存储
        residuals.push(samples[0].value);
        
        if samples.len() >= 2 {
            // 第二个值也直接存储
            residuals.push(samples[1].value);
        }

        // 使用线性预测：预测值 = 2 * 前一个值 - 前两个值
        for i in 2..samples.len() {
            let predicted = 2.0 * samples[i - 1].value - samples[i - 2].value;
            let residual = samples[i].value - predicted;
            residuals.push(residual);
        }

        Ok(residuals)
    }

    /// 解码预测编码的值
    pub fn decode(residuals: &[f64]) -> Result<Vec<f64>> {
        if residuals.len() < 2 {
            return Ok(residuals.to_vec());
        }

        let mut values = Vec::with_capacity(residuals.len());
        
        // 第一个值
        values.push(residuals[0]);
        
        if residuals.len() >= 2 {
            // 第二个值
            values.push(residuals[1]);
        }

        // 使用线性预测解码
        for i in 2..residuals.len() {
            let predicted = 2.0 * values[i - 1] - values[i - 2];
            let value = predicted + residuals[i];
            values.push(value);
        }

        Ok(values)
    }

    /// 计算预测编码的压缩率
    pub fn compression_ratio(original: &[Sample], encoded: &[f64]) -> f64 {
        if original.is_empty() {
            return 1.0;
        }

        let original_size = original.len() * std::mem::size_of::<Sample>();
        let encoded_size = encoded.len() * std::mem::size_of::<f64>();

        original_size as f64 / encoded_size as f64
    }
}

/// 双指数平滑预测
pub struct DoubleExponentialSmoothing {
    alpha: f64,
    beta: f64,
}

impl DoubleExponentialSmoothing {
    pub fn new(alpha: f64, beta: f64) -> Self {
        Self { alpha, beta }
    }

    /// 预测下一个值
    pub fn predict(&self, values: &[f64]) -> f64 {
        if values.len() < 2 {
            return values.last().copied().unwrap_or(0.0);
        }

        let mut level = values[0];
        let mut trend = values[1] - values[0];

        for i in 1..values.len() {
            let value = values[i];
            let prev_level = level;
            level = self.alpha * value + (1.0 - self.alpha) * (level + trend);
            trend = self.beta * (level - prev_level) + (1.0 - self.beta) * trend;
        }

        level + trend
    }

    /// 编码值序列
    pub fn encode(&self, values: &[f64]) -> Vec<f64> {
        if values.len() < 2 {
            return values.to_vec();
        }

        let mut residuals = Vec::with_capacity(values.len());
        let mut level = values[0];
        let mut trend = values[1] - values[0];

        // 存储初始值
        residuals.push(values[0]);
        residuals.push(values[1]);

        for i in 2..values.len() {
            let predicted = level + trend;
            let residual = values[i] - predicted;
            residuals.push(residual);

            let prev_level = level;
            level = self.alpha * values[i] + (1.0 - self.alpha) * (level + trend);
            trend = self.beta * (level - prev_level) + (1.0 - self.beta) * trend;
        }

        residuals
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prediction_encoder() {
        let samples = vec![
            Sample::new(1000, 10.0),
            Sample::new(2000, 20.0),
            Sample::new(3000, 30.0),
            Sample::new(4000, 40.0),
        ];

        let encoded = PredictionEncoder::encode(&samples).unwrap();
        let decoded = PredictionEncoder::decode(&encoded).unwrap();

        // 验证解码后的值与原始值一致
        for (i, sample) in samples.iter().enumerate() {
            assert!((decoded[i] - sample.value).abs() < 1e-10);
        }
    }

    #[test]
    fn test_double_exponential_smoothing() {
        let values = vec![10.0, 20.0, 30.0, 40.0, 50.0];
        let des = DoubleExponentialSmoothing::new(0.3, 0.1);

        let prediction = des.predict(&values);
        assert!(prediction > 50.0);

        let encoded = des.encode(&values);
        assert_eq!(encoded.len(), values.len());
    }
}
