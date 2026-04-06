pub mod task;
pub mod manager;
pub mod worker;

pub use task::{DownsampleTask, TaskConfig, TaskStatus, TaskResult, TaskBatch};
pub use manager::DownsampleManager;
pub use worker::DownsampleWorker;

use crate::columnstore::DownsampleLevel;
use crate::error::Result;
use crate::model::{Sample, TimeSeries};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{interval, Duration};
use tracing::{info, error, warn};

/// 降采样统计信息
#[derive(Debug, Clone, Default)]
pub struct DownsampleStats {
    pub total_tasks: u64,
    pub completed_tasks: u64,
    pub failed_tasks: u64,
    pub total_samples_processed: u64,
    pub total_samples_generated: u64,
    pub last_run_timestamp: Option<i64>,
    pub level_stats: HashMap<DownsampleLevel, LevelStats>,
}

#[derive(Debug, Clone, Default)]
pub struct LevelStats {
    pub samples_processed: u64,
    pub samples_generated: u64,
    pub task_count: u64,
}

/// 降采样配置
#[derive(Debug, Clone)]
pub struct DownsampleConfig {
    pub enabled: bool,
    pub interval: Duration,
    pub concurrency: usize,
    pub timeout: Duration,
    pub levels: Vec<LevelConfig>,
}

impl Default for DownsampleConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            interval: Duration::from_secs(900), // 15分钟
            concurrency: 4,
            timeout: Duration::from_secs(3600), // 1小时
            levels: vec![
                LevelConfig {
                    level: DownsampleLevel::L1,
                    enabled: true,
                    functions: vec!["min", "max", "avg", "sum", "count", "last"]
                        .into_iter()
                        .map(String::from)
                        .collect(),
                },
                LevelConfig {
                    level: DownsampleLevel::L2,
                    enabled: true,
                    functions: vec!["min", "max", "avg", "sum", "count", "last"]
                        .into_iter()
                        .map(String::from)
                        .collect(),
                },
                LevelConfig {
                    level: DownsampleLevel::L3,
                    enabled: true,
                    functions: vec!["min", "max", "avg", "sum", "count", "last"]
                        .into_iter()
                        .map(String::from)
                        .collect(),
                },
                LevelConfig {
                    level: DownsampleLevel::L4,
                    enabled: true,
                    functions: vec!["min", "max", "avg", "sum", "count", "last"]
                        .into_iter()
                        .map(String::from)
                        .collect(),
                },
            ],
        }
    }
}

#[derive(Debug, Clone)]
pub struct LevelConfig {
    pub level: DownsampleLevel,
    pub enabled: bool,
    pub functions: Vec<String>,
}

/// 降采样数据点
#[derive(Debug, Clone)]
pub struct DownsamplePoint {
    pub timestamp: i64,
    pub min_value: f64,
    pub max_value: f64,
    pub avg_value: f64,
    pub sum_value: f64,
    pub count: u64,
    pub last_value: f64,
}

impl DownsamplePoint {
    pub fn new(timestamp: i64) -> Self {
        Self {
            timestamp,
            min_value: f64::MAX,
            max_value: f64::MIN,
            avg_value: 0.0,
            sum_value: 0.0,
            count: 0,
            last_value: 0.0,
        }
    }

    pub fn add_sample(&mut self, value: f64) {
        self.min_value = self.min_value.min(value);
        self.max_value = self.max_value.max(value);
        self.sum_value += value;
        self.count += 1;
        self.avg_value = self.sum_value / self.count as f64;
        self.last_value = value;
    }

    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    pub fn to_samples(&self) -> Vec<Sample> {
        if self.is_empty() {
            return vec![];
        }

        vec![
            Sample::new(self.timestamp, self.min_value),
            Sample::new(self.timestamp, self.max_value),
            Sample::new(self.timestamp, self.avg_value),
            Sample::new(self.timestamp, self.sum_value),
            Sample::new(self.timestamp, self.count as f64),
            Sample::new(self.timestamp, self.last_value),
        ]
    }
}

/// 降采样处理器
pub struct DownsampleProcessor;

impl DownsampleProcessor {
    /// 对时间序列进行降采样
    pub fn downseries(
        samples: &[Sample],
        resolution_ms: i64,
    ) -> Vec<DownsamplePoint> {
        if samples.is_empty() {
            return vec![];
        }

        let mut result = Vec::new();
        let mut current_window = samples[0].timestamp - (samples[0].timestamp % resolution_ms);
        let mut current_point = DownsamplePoint::new(current_window);

        for sample in samples {
            let sample_window = sample.timestamp - (sample.timestamp % resolution_ms);

            if sample_window != current_window {
                // 保存当前窗口的数据
                if !current_point.is_empty() {
                    result.push(current_point);
                }

                // 开始新窗口
                current_window = sample_window;
                current_point = DownsamplePoint::new(current_window);
            }

            current_point.add_sample(sample.value);
        }

        // 保存最后一个窗口
        if !current_point.is_empty() {
            result.push(current_point);
        }

        result
    }

    /// 根据函数类型获取降采样值
    pub fn get_value_by_function(point: &DownsamplePoint, function: &str) -> f64 {
        match function {
            "min" => point.min_value,
            "max" => point.max_value,
            "avg" => point.avg_value,
            "sum" => point.sum_value,
            "count" => point.count as f64,
            "last" => point.last_value,
            _ => point.avg_value, // 默认返回平均值
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_downsample_point() {
        let mut point = DownsamplePoint::new(1000);
        
        point.add_sample(10.0);
        point.add_sample(20.0);
        point.add_sample(30.0);
        
        assert_eq!(point.min_value, 10.0);
        assert_eq!(point.max_value, 30.0);
        assert_eq!(point.avg_value, 20.0);
        assert_eq!(point.sum_value, 60.0);
        assert_eq!(point.count, 3);
        assert_eq!(point.last_value, 30.0);
    }

    #[test]
    fn test_downsample_processor() {
        let samples = vec![
            Sample::new(1000, 10.0),
            Sample::new(2000, 20.0),
            Sample::new(3000, 30.0),
            Sample::new(11000, 40.0),
            Sample::new(12000, 50.0),
        ];

        // 使用10秒分辨率降采样
        let points = DownsampleProcessor::downseries(&samples, 10000);
        
        assert_eq!(points.len(), 2);
        assert_eq!(points[0].count, 3); // 1000, 2000, 3000
        assert_eq!(points[1].count, 2); // 11000, 12000
    }
}
