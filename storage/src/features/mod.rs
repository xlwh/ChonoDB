use thiserror::Error;
use std::f64::consts::PI;

#[derive(Debug, Error)]
pub enum FeatureError {
    #[error("Insufficient data: {0}")]
    InsufficientData(String),
    
    #[error("Calculation error: {0}")]
    CalculationError(String),
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TimeDomainFeatures {
    pub mean: f64,
    pub variance: f64,
    pub std_dev: f64,
    pub skewness: f64,
    pub kurtosis: f64,
    pub min: f64,
    pub max: f64,
    pub median: f64,
    pub q1: f64, // 25th percentile
    pub q3: f64, // 75th percentile
    pub range: f64,
    pub sum: f64,
    pub count: usize,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FrequencyDomainFeatures {
    pub dominant_frequency: f64,
    pub spectral_centroid: f64,
    pub spectral_spread: f64,
    pub spectral_entropy: f64,
    pub total_power: f64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MorphologicalFeatures {
    pub slope: f64,
    pub trend: f64,
    pub linearity: f64, // R² value
    pub zero_crossing_rate: f64,
    pub mean_absolute_deviation: f64,
    pub root_mean_square: f64,
    pub crest_factor: f64,
    pub shape_factor: f64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TimeSeriesFeatures {
    pub time_domain: TimeDomainFeatures,
    pub frequency_domain: Option<FrequencyDomainFeatures>,
    pub morphological: MorphologicalFeatures,
}

pub struct FeatureExtractor {
    min_samples: usize,
}

impl Default for FeatureExtractor {
    fn default() -> Self {
        Self {
            min_samples: 10,
        }
    }
}

impl FeatureExtractor {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_min_samples(mut self, min_samples: usize) -> Self {
        self.min_samples = min_samples;
        self
    }

    pub fn extract_features(&self, values: &[f64]) -> Result<TimeSeriesFeatures, FeatureError> {
        if values.len() < self.min_samples {
            return Err(FeatureError::InsufficientData(
                format!("Need at least {} samples, got {}", self.min_samples, values.len())
            ));
        }

        let time_domain = self.extract_time_domain_features(values)?;
        let frequency_domain = self.extract_frequency_domain_features(values)?;
        let morphological = self.extract_morphological_features(values)?;

        Ok(TimeSeriesFeatures {
            time_domain,
            frequency_domain,
            morphological,
        })
    }

    pub fn extract_time_domain_features(&self, values: &[f64]) -> Result<TimeDomainFeatures, FeatureError> {
        let count = values.len();
        let sum: f64 = values.iter().sum();
        let mean = sum / count as f64;

        let variance: f64 = values.iter().map(|&v| (v - mean).powi(2)).sum::<f64>() / count as f64;
        let std_dev = variance.sqrt();

        let skewness: f64 = values.iter().map(|&v| (v - mean).powi(3)).sum::<f64>() / (count as f64 * std_dev.powi(3));
        let kurtosis: f64 = values.iter().map(|&v| (v - mean).powi(4)).sum::<f64>() / (count as f64 * std_dev.powi(4)) - 3.0;

        let mut sorted_values = values.to_vec();
        sorted_values.sort_by(|a, b| a.partial_cmp(b).unwrap());

        let min = sorted_values[0];
        let max = sorted_values[count - 1];
        let range = max - min;

        let median = if count % 2 == 0 {
            (sorted_values[count / 2 - 1] + sorted_values[count / 2]) / 2.0
        } else {
            sorted_values[count / 2]
        };

        let q1 = sorted_values[(count * 1 / 4)];
        let q3 = sorted_values[(count * 3 / 4)];

        Ok(TimeDomainFeatures {
            mean,
            variance,
            std_dev,
            skewness,
            kurtosis,
            min,
            max,
            median,
            q1,
            q3,
            range,
            sum,
            count,
        })
    }

    pub fn extract_frequency_domain_features(&self, values: &[f64]) -> Result<Option<FrequencyDomainFeatures>, FeatureError> {
        if values.len() < 32 { // 需要足够的样本进行 FFT
            return Ok(None);
        }

        // 计算 FFT（简化实现）
        let n = values.len();
        let mut power_spectrum = vec![0.0; n / 2];

        for k in 0..n / 2 {
            let mut real = 0.0;
            let mut imag = 0.0;

            for (i, &value) in values.iter().enumerate() {
                let angle = 2.0 * PI * k as f64 * i as f64 / n as f64;
                real += value * angle.cos();
                imag += value * angle.sin();
            }

            power_spectrum[k] = real * real + imag * imag;
        }

        let total_power: f64 = power_spectrum.iter().sum();
        if total_power == 0.0 {
            return Ok(None);
        }

        let dominant_frequency = power_spectrum
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
            .map(|(k, _)| k as f64)
            .unwrap();

        let spectral_centroid: f64 = power_spectrum
            .iter()
            .enumerate()
            .map(|(k, &p)| k as f64 * p)
            .sum::<f64>() / total_power;

        let spectral_spread: f64 = power_spectrum
            .iter()
            .enumerate()
            .map(|(k, &p)| ((k as f64 - spectral_centroid).powi(2)) * p)
            .sum::<f64>() / total_power;

        let spectral_entropy: f64 = power_spectrum
            .iter()
            .map(|&p| {
                let prob = p / total_power;
                if prob > 0.0 {
                    -prob * prob.ln()
                } else {
                    0.0
                }
            })
            .sum();

        Ok(Some(FrequencyDomainFeatures {
            dominant_frequency,
            spectral_centroid,
            spectral_spread,
            spectral_entropy,
            total_power,
        }))
    }

    pub fn extract_morphological_features(&self, values: &[f64]) -> Result<MorphologicalFeatures, FeatureError> {
        let n = values.len() as f64;
        let indices: Vec<f64> = (0..values.len()).map(|i| i as f64).collect();

        // 计算线性回归
        let sum_x: f64 = indices.iter().sum();
        let sum_y: f64 = values.iter().sum();
        let sum_xy: f64 = indices.iter().zip(values.iter()).map(|(x, y)| x * y).sum();
        let sum_x2: f64 = indices.iter().map(|&x| x * x).sum();

        let slope = (n * sum_xy - sum_x * sum_y) / (n * sum_x2 - sum_x * sum_x);
        let intercept = (sum_y - slope * sum_x) / n;

        // 计算 R²
        let mean_y = sum_y / n;
        let total_sum_squares: f64 = values.iter().map(|&y| (y - mean_y).powi(2)).sum();
        let residual_sum_squares: f64 = indices.iter().zip(values.iter()).map(|(x, &y)| {
            let predicted = slope * x + intercept;
            (y - predicted).powi(2)
        }).sum();

        let linearity = if total_sum_squares > 0.0 {
            1.0 - (residual_sum_squares / total_sum_squares)
        } else {
            1.0
        };

        // 计算零交叉率
        let mut zero_crossings = 0;
        for i in 1..values.len() {
            if (values[i-1] * values[i]) < 0.0 {
                zero_crossings += 1;
            }
        }
        let zero_crossing_rate = zero_crossings as f64 / (values.len() - 1) as f64;

        // 计算平均绝对偏差
        let mean_absolute_deviation: f64 = values.iter().map(|&v| (v - mean_y).abs()).sum::<f64>() / n;

        // 计算均方根
        let root_mean_square: f64 = (values.iter().map(|&v| v * v).sum::<f64>() / n).sqrt();

        // 计算峰值因子
        let crest_factor = if root_mean_square > 0.0 {
            values.iter().map(|&v| v.abs()).max_by(|a, b| a.partial_cmp(b).unwrap()).unwrap() / root_mean_square
        } else {
            0.0
        };

        // 计算形状因子
        let shape_factor = if mean_absolute_deviation > 0.0 {
            root_mean_square / mean_absolute_deviation
        } else {
            0.0
        };

        // 计算趋势
        let trend = if values.len() > 1 {
            values[values.len() - 1] - values[0]
        } else {
            0.0
        };

        Ok(MorphologicalFeatures {
            slope,
            trend,
            linearity,
            zero_crossing_rate,
            mean_absolute_deviation,
            root_mean_square,
            crest_factor,
            shape_factor,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_time_domain_features() {
        let extractor = FeatureExtractor::new();
        let values = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0];
        
        let features = extractor.extract_time_domain_features(&values).unwrap();
        assert_eq!(features.mean, 5.5);
        assert_eq!(features.min, 1.0);
        assert_eq!(features.max, 10.0);
        assert_eq!(features.median, 5.5);
    }

    #[test]
    fn test_morphological_features() {
        let extractor = FeatureExtractor::new();
        let values = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        
        let features = extractor.extract_morphological_features(&values).unwrap();
        assert_eq!(features.slope, 1.0); // 线性增长，斜率为 1
        assert_eq!(features.trend, 4.0); // 从 1 到 5，趋势为 4
        assert_eq!(features.linearity, 1.0); // 完全线性，R² 为 1
    }

    #[test]
    fn test_extract_features() {
        let extractor = FeatureExtractor::new();
        let values = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0];
        
        let features = extractor.extract_features(&values).unwrap();
        assert_eq!(features.time_domain.mean, 5.5);
        assert_eq!(features.morphological.slope, 1.0);
    }

    #[test]
    fn test_insufficient_data() {
        let extractor = FeatureExtractor::new().with_min_samples(10);
        let values = vec![1.0, 2.0, 3.0];
        
        let result = extractor.extract_features(&values);
        assert!(result.is_err());
        match result.unwrap_err() {
            FeatureError::InsufficientData(msg) => {
                assert!(msg.contains("Need at least 10 samples"));
            }
            _ => panic!("Expected InsufficientData error"),
        }
    }
}
