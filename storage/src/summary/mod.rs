use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SummaryError {
    #[error("Invalid data: {0}")]
    InvalidData(String),
    
    #[error("Calculation error: {0}")]
    CalculationError(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeSeriesSummary {
    pub metric_name: String,
    pub labels: Vec<(String, String)>,
    pub sample_count: usize,
    pub time_range: (i64, i64),
    pub basic_stats: BasicStats,
    pub trend: Option<TrendInfo>,
    pub periodicity: Option<PeriodicityInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BasicStats {
    pub min: f64,
    pub max: f64,
    pub mean: f64,
    pub median: f64,
    pub std_dev: f64,
    pub p25: f64,
    pub p75: f64,
    pub p95: f64,
    pub p99: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrendInfo {
    pub slope: f64,
    pub intercept: f64,
    pub r_squared: f64,
    pub trend_direction: TrendDirection,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TrendDirection {
    Up,
    Down,
    Stable,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeriodicityInfo {
    pub period: u64, // 周期长度（秒）
    pub confidence: f64, // 置信度
    pub is_periodic: bool,
}

#[derive(Debug, Clone)]
pub struct SummaryGenerator {
    // 配置参数
    pub min_samples: usize,
    pub max_samples: usize,
}

impl Default for SummaryGenerator {
    fn default() -> Self {
        Self {
            min_samples: 10,
            max_samples: 10000,
        }
    }
}

impl SummaryGenerator {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_min_samples(mut self, min_samples: usize) -> Self {
        self.min_samples = min_samples;
        self
    }

    pub fn with_max_samples(mut self, max_samples: usize) -> Self {
        self.max_samples = max_samples;
        self
    }

    pub fn generate_summary(
        &self,
        metric_name: &str,
        labels: &[(String, String)],
        samples: &[(i64, f64)],
    ) -> Result<TimeSeriesSummary, SummaryError> {
        if samples.len() < self.min_samples {
            return Err(SummaryError::InvalidData(
                "Insufficient samples for summary generation".to_string(),
            ));
        }

        // 限制样本数量
        let limited_samples = if samples.len() > self.max_samples {
            let step = samples.len() / self.max_samples;
            samples.iter().step_by(step).collect::<Vec<_>>()
        } else {
            samples.iter().collect::<Vec<_>>()
        };

        // 计算基本统计信息
        let basic_stats = self.calculate_basic_stats(&limited_samples)?;

        // 分析趋势
        let trend = self.analyze_trend(&limited_samples);

        // 检测周期性
        let periodicity = self.detect_periodicity(&limited_samples);

        // 计算时间范围
        let time_range = if let (Some(first), Some(last)) = (samples.first(), samples.last()) {
            (first.0, last.0)
        } else {
            (0, 0)
        };

        Ok(TimeSeriesSummary {
            metric_name: metric_name.to_string(),
            labels: labels.to_vec(),
            sample_count: samples.len(),
            time_range,
            basic_stats,
            trend,
            periodicity,
        })
    }

    fn calculate_basic_stats(
        &self,
        samples: &[&(i64, f64)],
    ) -> Result<BasicStats, SummaryError> {
        let values: Vec<f64> = samples.iter().map(|&&(_, v)| v).collect();

        if values.is_empty() {
            return Err(SummaryError::InvalidData("No values to calculate stats".to_string()));
        }

        // 排序用于计算分位数
        let mut sorted_values = values.clone();
        sorted_values.sort_by(|a, b| a.partial_cmp(b).unwrap());

        let min = *sorted_values.first().unwrap();
        let max = *sorted_values.last().unwrap();

        // 计算均值
        let sum: f64 = values.iter().sum();
        let mean = sum / values.len() as f64;

        // 计算中位数
        let median = if values.len() % 2 == 0 {
            (sorted_values[values.len() / 2 - 1] + sorted_values[values.len() / 2]) / 2.0
        } else {
            sorted_values[values.len() / 2]
        };

        // 计算标准差
        let variance: f64 = values
            .iter()
            .map(|&v| (v - mean).powi(2))
            .sum::<f64>() / values.len() as f64;
        let std_dev = variance.sqrt();

        // 计算分位数
        let p25 = self.calculate_percentile(&sorted_values, 25.0);
        let p75 = self.calculate_percentile(&sorted_values, 75.0);
        let p95 = self.calculate_percentile(&sorted_values, 95.0);
        let p99 = self.calculate_percentile(&sorted_values, 99.0);

        Ok(BasicStats {
            min,
            max,
            mean,
            median,
            std_dev,
            p25,
            p75,
            p95,
            p99,
        })
    }

    fn calculate_percentile(&self, sorted_values: &[f64], percentile: f64) -> f64 {
        if sorted_values.is_empty() {
            return 0.0;
        }

        let index = (percentile / 100.0) * (sorted_values.len() - 1) as f64;
        let lower_index = index.floor() as usize;
        let upper_index = index.ceil() as usize;

        if lower_index == upper_index {
            sorted_values[lower_index]
        } else {
            let fraction = index - lower_index as f64;
            sorted_values[lower_index] * (1.0 - fraction) + sorted_values[upper_index] * fraction
        }
    }

    fn analyze_trend(&self, samples: &[&(i64, f64)]) -> Option<TrendInfo> {
        if samples.len() < 2 {
            return None;
        }

        // 线性回归计算趋势
        let n = samples.len() as f64;
        let mut sum_x = 0.0;
        let mut sum_y = 0.0;
        let mut sum_xy = 0.0;
        let mut sum_x2 = 0.0;

        for (i, &(_, y)) in samples.iter().enumerate() {
            let x = i as f64;
            sum_x += x;
            sum_y += y;
            sum_xy += x * y;
            sum_x2 += x * x;
        }

        let numerator = n * sum_xy - sum_x * sum_y;
        let denominator = n * sum_x2 - sum_x * sum_x;

        if denominator == 0.0 {
            return None;
        }

        let slope = numerator / denominator;
        let intercept = (sum_y - slope * sum_x) / n;

        // 计算 R²
        let mean_y = sum_y / n;
        let mut total_sum_squares = 0.0;
        let mut residual_sum_squares = 0.0;

        for (i, &(_, y)) in samples.iter().enumerate() {
            let x = i as f64;
            let predicted = slope * x + intercept;
            total_sum_squares += (y - mean_y).powi(2);
            residual_sum_squares += (y - predicted).powi(2);
        }

        let r_squared = if total_sum_squares == 0.0 {
            1.0
        } else {
            1.0 - (residual_sum_squares / total_sum_squares)
        };

        // 确定趋势方向
        let trend_direction = if slope > 0.001 {
            TrendDirection::Up
        } else if slope < -0.001 {
            TrendDirection::Down
        } else {
            TrendDirection::Stable
        };

        Some(TrendInfo {
            slope,
            intercept,
            r_squared,
            trend_direction,
        })
    }

    fn detect_periodicity(&self, samples: &[&(i64, f64)]) -> Option<PeriodicityInfo> {
        if samples.len() < 10 {
            return None;
        }

        // 简单的周期性检测：计算时间间隔的模式
        let mut time_diffs = Vec::new();
        for i in 1..samples.len() {
            let diff = samples[i].0 - samples[i-1].0;
            if diff > 0 {
                time_diffs.push(diff);
            }
        }

        if time_diffs.is_empty() {
            return None;
        }

        // 找出最常见的时间间隔
        let mut interval_counts = std::collections::HashMap::new();
        for &diff in &time_diffs {
            *interval_counts.entry(diff).or_insert(0) += 1;
        }

        let (most_common_interval, count) = interval_counts
            .into_iter()
            .max_by_key(|&(_, c)| c)
            .unwrap();

        let confidence = count as f64 / time_diffs.len() as f64;
        let is_periodic = confidence > 0.8;

        Some(PeriodicityInfo {
            period: most_common_interval as u64,
            confidence,
            is_periodic,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_summary() {
        let generator = SummaryGenerator::new();
        
        let metric_name = "cpu_usage";
        let labels = vec![("server".to_string(), "server1".to_string()), ("region".to_string(), "us-east-1".to_string())];
        
        // 生成测试数据：模拟 CPU 使用率，有轻微上升趋势
        let mut samples = Vec::new();
        let start_time = 1609459200; // 2021-01-01 00:00:00
        
        for i in 0..100 {
            let timestamp = start_time + i * 60; // 每分钟一个样本
            let value = 50.0 + i as f64 * 0.1; // 从 50 开始，每分钟增加 0.1
            samples.push((timestamp, value));
        }
        
        let result = generator.generate_summary(metric_name, &labels, &samples);
        assert!(result.is_ok());
        
        let summary = result.unwrap();
        assert_eq!(summary.metric_name, metric_name);
        assert_eq!(summary.labels, labels);
        assert_eq!(summary.sample_count, 100);
        
        // 检查基本统计信息
        assert!(summary.basic_stats.min >= 50.0);
        assert!(summary.basic_stats.max <= 60.0);
        assert!(summary.basic_stats.mean >= 54.0 && summary.basic_stats.mean <= 56.0);
        
        // 检查趋势分析
        assert!(summary.trend.is_some());
        if let Some(trend) = &summary.trend {
            assert!(trend.slope > 0.0);
            assert_eq!(trend.trend_direction, TrendDirection::Up);
        }
        
        // 检查周期性检测
        assert!(summary.periodicity.is_some());
        if let Some(periodicity) = &summary.periodicity {
            assert!(periodicity.is_periodic);
            assert_eq!(periodicity.period, 60); // 60秒周期
        }
    }

    #[test]
    fn test_insufficient_samples() {
        let generator = SummaryGenerator::new().with_min_samples(10);
        
        let metric_name = "cpu_usage";
        let labels = vec![("server".to_string(), "server1".to_string())];
        let samples = vec![(1609459200, 50.0), (1609459260, 51.0)];
        
        let result = generator.generate_summary(metric_name, &labels, &samples);
        assert!(result.is_err());
        match result.unwrap_err() {
            SummaryError::InvalidData(msg) => {
                assert!(msg.contains("Insufficient samples"));
            }
            _ => panic!("Expected InvalidData error"),
        }
    }

    #[test]
    fn test_basic_stats_calculation() {
        let generator = SummaryGenerator::new();
        
        let samples = vec![
            &(1609459200, 10.0),
            &(1609459260, 20.0),
            &(1609459320, 30.0),
            &(1609459380, 40.0),
            &(1609459440, 50.0),
        ];
        
        let result = generator.calculate_basic_stats(&samples);
        assert!(result.is_ok());
        
        let stats = result.unwrap();
        assert_eq!(stats.min, 10.0);
        assert_eq!(stats.max, 50.0);
        assert_eq!(stats.mean, 30.0);
        assert_eq!(stats.median, 30.0);
    }

    #[test]
    fn test_trend_analysis() {
        let generator = SummaryGenerator::new();
        
        let samples = vec![
            &(1609459200, 10.0),
            &(1609459260, 20.0),
            &(1609459320, 30.0),
            &(1609459380, 40.0),
            &(1609459440, 50.0),
        ];
        
        let trend = generator.analyze_trend(&samples);
        assert!(trend.is_some());
        if let Some(trend_info) = trend {
            assert!(trend_info.slope > 0.0);
            assert_eq!(trend_info.trend_direction, TrendDirection::Up);
            assert!(trend_info.r_squared > 0.99); // 完全线性，R² 接近 1
        }
    }

    #[test]
    fn test_periodicity_detection() {
        let generator = SummaryGenerator::new();
        
        let mut samples = Vec::new();
        let start_time = 1609459200;
        
        for i in 0..20 {
            let timestamp = start_time + i * 300; // 每 5 分钟一个样本
            samples.push((timestamp, 50.0 + (i % 12) as f64));
        }
        
        // 创建引用向量
        let sample_refs: Vec<&(i64, f64)> = samples.iter().collect();
        
        let periodicity = generator.detect_periodicity(&sample_refs);
        assert!(periodicity.is_some());
        if let Some(period_info) = periodicity {
            assert!(period_info.is_periodic);
            assert_eq!(period_info.period, 300); // 300秒周期
            assert!(period_info.confidence > 0.9);
        }
    }
}