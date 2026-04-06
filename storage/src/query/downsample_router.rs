use crate::columnstore::DownsampleLevel;
use crate::error::Result;
use crate::model::{Sample, TimeSeries};
use crate::memstore::MemStore;
use std::sync::Arc;
use tracing::{info, debug};

/// 降采样路由决策
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DownsampleRoute {
    /// 使用原始数据
    Raw,
    /// 使用降采样数据
    Downsampled(DownsampleLevel),
}

/// 降采样路由器
pub struct DownsampleRouter {
    /// 是否启用自动降采样
    enabled: bool,
    /// 降采样策略
    policy: DownsamplePolicy,
}

/// 降采样策略
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DownsamplePolicy {
    /// 自动选择
    Auto,
    /// 保守策略（优先使用高精度）
    Conservative,
    /// 激进策略（优先使用低精度）
    Aggressive,
}

impl Default for DownsamplePolicy {
    fn default() -> Self {
        DownsamplePolicy::Auto
    }
}

impl DownsampleRouter {
    /// 创建新的降采样路由器
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled,
            policy: DownsamplePolicy::default(),
        }
    }

    /// 设置降采样策略
    pub fn with_policy(mut self, policy: DownsamplePolicy) -> Self {
        self.policy = policy;
        self
    }

    /// 根据查询计划选择降采样级别
    pub fn select_level(&self, query_range_ms: i64, func_type: &str) -> DownsampleRoute {
        if !self.enabled {
            return DownsampleRoute::Raw;
        }

        // 将毫秒转换为小时
        let query_range_hours = query_range_ms / (3600 * 1000);

        // 基础规则：根据时间范围选择
        let base_level = self.select_by_time_range(query_range_hours);

        // 根据函数类型调整
        let adjusted_level = self.adjust_by_function(base_level, func_type);

        debug!(
            "Downsample route selected: query_range={}h, func_type={}, level={:?}",
            query_range_hours, func_type, adjusted_level
        );

        adjusted_level
    }

    /// 根据时间范围选择基础级别
    fn select_by_time_range(&self, query_range_hours: i64) -> DownsampleRoute {
        match self.policy {
            DownsamplePolicy::Conservative => {
                // 保守策略：优先使用高精度
                match query_range_hours {
                    0..=1 => DownsampleRoute::Raw,           // < 1h: 原始数据
                    2..=24 => DownsampleRoute::Downsampled(DownsampleLevel::L1),  // 1-24h: 1min
                    25..=168 => DownsampleRoute::Downsampled(DownsampleLevel::L2), // 1-7d: 5min
                    169..=720 => DownsampleRoute::Downsampled(DownsampleLevel::L3), // 7-30d: 1h
                    _ => DownsampleRoute::Downsampled(DownsampleLevel::L4),        // >30d: 1d
                }
            }
            DownsamplePolicy::Aggressive => {
                // 激进策略：优先使用低精度
                match query_range_hours {
                    0..=1 => DownsampleRoute::Raw,
                    2..=12 => DownsampleRoute::Downsampled(DownsampleLevel::L1),
                    13..=72 => DownsampleRoute::Downsampled(DownsampleLevel::L2),
                    73..=336 => DownsampleRoute::Downsampled(DownsampleLevel::L3),
                    _ => DownsampleRoute::Downsampled(DownsampleLevel::L4),
                }
            }
            DownsamplePolicy::Auto => {
                // 自动策略：平衡选择
                match query_range_hours {
                    0..=1 => DownsampleRoute::Raw,           // < 1h: 原始数据
                    2..=24 => DownsampleRoute::Downsampled(DownsampleLevel::L1),  // 1-24h: 1min
                    25..=168 => DownsampleRoute::Downsampled(DownsampleLevel::L2), // 1-7d: 5min
                    169..=720 => DownsampleRoute::Downsampled(DownsampleLevel::L3), // 7-30d: 1h
                    _ => DownsampleRoute::Downsampled(DownsampleLevel::L4),        // >30d: 1d
                }
            }
        }
    }

    /// 根据函数类型调整级别
    fn adjust_by_function(&self, base: DownsampleRoute, func_type: &str) -> DownsampleRoute {
        match func_type {
            // rate类函数需要较高精度，降低一级
            "rate" | "irate" | "delta" | "increase" => {
                match base {
                    DownsampleRoute::Raw => DownsampleRoute::Raw,
                    DownsampleRoute::Downsampled(DownsampleLevel::L1) => DownsampleRoute::Raw,
                    DownsampleRoute::Downsampled(DownsampleLevel::L2) => DownsampleRoute::Downsampled(DownsampleLevel::L1),
                    DownsampleRoute::Downsampled(DownsampleLevel::L3) => DownsampleRoute::Downsampled(DownsampleLevel::L2),
                    DownsampleRoute::Downsampled(DownsampleLevel::L4) => DownsampleRoute::Downsampled(DownsampleLevel::L3),
                    _ => base,
                }
            }
            // 分位数函数需要原始精度
            "quantile" | "histogram_quantile" => DownsampleRoute::Raw,
            // 聚合函数可以使用较低精度
            "sum" | "avg" | "min" | "max" | "count" | "group" => base,
            // 默认保持原级别
            _ => base,
        }
    }

    /// 从函数名称推断函数类型
    pub fn infer_function_type(func_name: &str) -> &str {
        match func_name.to_lowercase().as_str() {
            s if s.contains("rate") => "rate",
            s if s.contains("delta") => "delta",
            s if s.contains("increase") => "increase",
            s if s.contains("quantile") => "quantile",
            s if s.contains("histogram") => "histogram_quantile",
            s if s.contains("sum") => "sum",
            s if s.contains("avg") => "avg",
            s if s.contains("min") => "min",
            s if s.contains("max") => "max",
            s if s.contains("count") => "count",
            s if s.contains("group") => "group",
            _ => "unknown",
        }
    }
}

/// 降采样查询执行器
pub struct DownsampleQueryExecutor {
    router: DownsampleRouter,
    store: Arc<MemStore>,
}

impl DownsampleQueryExecutor {
    /// 创建新的降采样查询执行器
    pub fn new(store: Arc<MemStore>, router: DownsampleRouter) -> Self {
        Self { router, store }
    }

    /// 执行查询，自动选择合适的降采样级别
    pub async fn query(
        &self,
        series_ids: &[u64],
        start: i64,
        end: i64,
        func_type: &str,
    ) -> Result<Vec<TimeSeries>> {
        let query_range = end - start;
        let route = self.router.select_level(query_range, func_type);

        info!(
            "Executing query with downsample route: {:?}, range={}ms, series_count={}",
            route, query_range, series_ids.len()
        );

        match route {
            DownsampleRoute::Raw => {
                // 查询原始数据
                self.query_raw(series_ids, start, end).await
            }
            DownsampleRoute::Downsampled(level) => {
                // 查询降采样数据
                self.query_downsampled(series_ids, start, end, level).await
            }
        }
    }

    /// 查询原始数据
    async fn query_raw(&self, series_ids: &[u64], start: i64, end: i64) -> Result<Vec<TimeSeries>> {
        let mut results = Vec::new();

        for &series_id in series_ids {
            if let Some(series) = self.store.get_series(series_id) {
                let filtered_samples: Vec<Sample> = series.samples
                    .into_iter()
                    .filter(|s| s.timestamp >= start && s.timestamp <= end)
                    .collect();

                if !filtered_samples.is_empty() {
                    let mut ts = TimeSeries::new(series_id, series.labels);
                    ts.add_samples(filtered_samples);
                    results.push(ts);
                }
            }
        }

        Ok(results)
    }

    /// 查询降采样数据
    async fn query_downsampled(
        &self,
        series_ids: &[u64],
        start: i64,
        end: i64,
        level: DownsampleLevel,
    ) -> Result<Vec<TimeSeries>> {
        // 这里应该查询降采样存储
        // 简化实现：先查询原始数据，然后实时降采样
        debug!(
            "Querying downsampled data at level {:?}, resolution={}ms",
            level,
            level.resolution_ms()
        );

        let raw_results = self.query_raw(series_ids, start, end).await?;
        let resolution = level.resolution_ms();

        // 实时降采样
        let mut downsampled_results = Vec::new();
        for series in raw_results {
            let downsampled = self.downsample_series(&series, resolution)?;
            if !downsampled.samples.is_empty() {
                downsampled_results.push(downsampled);
            }
        }

        Ok(downsampled_results)
    }

    /// 对单个时间序列进行降采样
    fn downsample_series(&self, series: &TimeSeries, resolution: i64) -> Result<TimeSeries> {
        if series.samples.is_empty() {
            return Ok(series.clone());
        }

        let mut downsampled_samples = Vec::new();
        let mut current_window = series.samples[0].timestamp - (series.samples[0].timestamp % resolution);
        let mut window_values = Vec::new();

        for sample in &series.samples {
            let sample_window = sample.timestamp - (sample.timestamp % resolution);

            if sample_window != current_window {
                // 处理当前窗口
                if !window_values.is_empty() {
                    let avg_value = window_values.iter().sum::<f64>() / window_values.len() as f64;
                    downsampled_samples.push(Sample::new(current_window, avg_value));
                }

                // 开始新窗口
                current_window = sample_window;
                window_values.clear();
            }

            window_values.push(sample.value);
        }

        // 处理最后一个窗口
        if !window_values.is_empty() {
            let avg_value = window_values.iter().sum::<f64>() / window_values.len() as f64;
            downsampled_samples.push(Sample::new(current_window, avg_value));
        }

        let mut result = TimeSeries::new(series.id, series.labels.clone());
        result.add_samples(downsampled_samples);

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_downsample_router_select_by_time() {
        let router = DownsampleRouter::new(true);

        // < 1h: 原始数据
        assert_eq!(
            router.select_level(30 * 60 * 1000, "sum"),
            DownsampleRoute::Raw
        );

        // 1-24h: L1 (1min)
        assert_eq!(
            router.select_level(2 * 3600 * 1000, "sum"),
            DownsampleRoute::Downsampled(DownsampleLevel::L1)
        );

        // 1-7d: L2 (5min)
        assert_eq!(
            router.select_level(3 * 24 * 3600 * 1000, "sum"),
            DownsampleRoute::Downsampled(DownsampleLevel::L2)
        );

        // 7-30d: L3 (1h)
        assert_eq!(
            router.select_level(15 * 24 * 3600 * 1000, "sum"),
            DownsampleRoute::Downsampled(DownsampleLevel::L3)
        );

        // >30d: L4 (1d)
        assert_eq!(
            router.select_level(60 * 24 * 3600 * 1000, "sum"),
            DownsampleRoute::Downsampled(DownsampleLevel::L4)
        );
    }

    #[test]
    fn test_downsample_router_adjust_by_function() {
        let router = DownsampleRouter::new(true);

        // rate函数需要更高精度
        let route = router.select_level(3 * 24 * 3600 * 1000, "rate");
        // 原本应该是L2，但rate函数会降低一级到L1
        assert_eq!(route, DownsampleRoute::Downsampled(DownsampleLevel::L1));

        // quantile函数需要原始精度
        let route = router.select_level(3 * 24 * 3600 * 1000, "quantile");
        assert_eq!(route, DownsampleRoute::Raw);
    }

    #[test]
    fn test_infer_function_type() {
        assert_eq!(DownsampleRouter::infer_function_type("rate"), "rate");
        assert_eq!(DownsampleRouter::infer_function_type("irate"), "rate");
        assert_eq!(DownsampleRouter::infer_function_type("sum"), "sum");
        assert_eq!(DownsampleRouter::infer_function_type("avg"), "avg");
        assert_eq!(DownsampleRouter::infer_function_type("quantile"), "quantile");
    }
}
