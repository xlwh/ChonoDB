use crate::error::Result;
use crate::model::{Sample, TimeSeries, TimeSeriesId};
use crate::query::planner::{PlanType, VectorQueryPlan};
use crate::memstore::MemStore;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tracing::{info, error, debug};

/// 并行查询执行器
pub struct ParallelQueryExecutor {
    max_concurrency: usize,
}

impl ParallelQueryExecutor {
    pub fn new(max_concurrency: usize) -> Self {
        Self { max_concurrency }
    }

    /// 并行执行多个时间序列的查询
    pub async fn execute_series_parallel<F, Fut>(
        &self,
        series_ids: Vec<TimeSeriesId>,
        process_fn: F,
    ) -> Result<Vec<TimeSeries>>
    where
        F: Fn(TimeSeriesId) -> Fut + Send + Sync + Clone + 'static,
        Fut: std::future::Future<Output = Result<Option<TimeSeries>>> + Send,
    {
        if series_ids.is_empty() {
            return Ok(vec![]);
        }

        let (tx, mut rx) = mpsc::channel(self.max_concurrency);
        let batch_size = (series_ids.len() + self.max_concurrency - 1) / self.max_concurrency;

        // 将系列ID分批处理
        let batches: Vec<Vec<TimeSeriesId>> = series_ids
            .chunks(batch_size.max(1))
            .map(|chunk| chunk.to_vec())
            .collect();

        let mut handles: Vec<JoinHandle<()>> = vec![];

        for batch in batches {
            let tx = tx.clone();
            let process_fn = process_fn.clone();

            let handle = tokio::spawn(async move {
                for series_id in batch {
                    match process_fn(series_id).await {
                        Ok(Some(series)) => {
                            if tx.send(Ok(series)).await.is_err() {
                                break;
                            }
                        }
                        Ok(None) => {}
                        Err(e) => {
                            let _ = tx.send(Err(e)).await;
                            break;
                        }
                    }
                }
            });

            handles.push(handle);
        }

        // 关闭发送端
        drop(tx);

        // 收集结果
        let mut results = vec![];
        while let Some(result) = rx.recv().await {
            match result {
                Ok(series) => results.push(series),
                Err(e) => {
                    // 取消所有任务
                    for handle in &handles {
                        handle.abort();
                    }
                    return Err(e);
                }
            }
        }

        // 等待所有任务完成
        for handle in handles {
            let _ = handle.await;
        }

        Ok(results)
    }

    /// 并行处理时间范围分片
    pub async fn execute_time_range_parallel<F, Fut>(
        &self,
        start: i64,
        end: i64,
        num_shards: usize,
        process_fn: F,
    ) -> Result<Vec<Vec<Sample>>>
    where
        F: Fn(i64, i64) -> Fut + Send + Sync + Clone + 'static,
        Fut: std::future::Future<Output = Result<Vec<Sample>>> + Send,
    {
        if start >= end || num_shards == 0 {
            return Ok(vec![]);
        }

        let range = end - start;
        let shard_size = range / num_shards as i64;

        if shard_size == 0 {
            // 范围太小，不分片
            let samples = process_fn(start, end).await?;
            return Ok(vec![samples]);
        }

        let (tx, mut rx) = mpsc::channel(num_shards);
        let mut handles: Vec<JoinHandle<()>> = vec![];

        for i in 0..num_shards {
            let shard_start = start + i as i64 * shard_size;
            let shard_end = if i == num_shards - 1 {
                end
            } else {
                shard_start + shard_size
            };

            let tx = tx.clone();
            let process_fn = process_fn.clone();

            let handle = tokio::spawn(async move {
                match process_fn(shard_start, shard_end).await {
                    Ok(samples) => {
                        let _ = tx.send(Ok((i, samples))).await;
                    }
                    Err(e) => {
                        let _ = tx.send(Err(e)).await;
                    }
                }
            });

            handles.push(handle);
        }

        // 关闭发送端
        drop(tx);

        // 收集结果并按顺序排列
        let mut shard_results: Vec<Option<Vec<Sample>>> = vec![None; num_shards];
        
        while let Some(result) = rx.recv().await {
            match result {
                Ok((shard_idx, samples)) => {
                    shard_results[shard_idx] = Some(samples);
                }
                Err(e) => {
                    // 取消所有任务
                    for handle in &handles {
                        handle.abort();
                    }
                    return Err(e);
                }
            }
        }

        // 等待所有任务完成
        for handle in handles {
            let _ = handle.await;
        }

        // 按顺序合并结果
        let mut all_samples = vec![];
        for samples in shard_results.into_iter().flatten() {
            all_samples.push(samples);
        }

        Ok(all_samples)
    }

    /// 并行聚合操作
    pub async fn parallel_aggregate<F, Fut>(
        &self,
        series_list: Vec<TimeSeries>,
        aggregate_fn: F,
    ) -> Result<TimeSeries>
    where
        F: Fn(Vec<TimeSeries>) -> Fut + Send + Sync + Clone + 'static,
        Fut: std::future::Future<Output = Result<TimeSeries>> + Send,
    {
        if series_list.is_empty() {
            return Err(crate::error::Error::InvalidData(
                "No series to aggregate".to_string()
            ));
        }

        if series_list.len() == 1 {
            return Ok(series_list.into_iter().next().unwrap());
        }

        // 分批聚合
        let batch_size = (series_list.len() + self.max_concurrency - 1) / self.max_concurrency;
        let batches: Vec<Vec<TimeSeries>> = series_list
            .chunks(batch_size.max(1))
            .map(|chunk| chunk.to_vec())
            .collect();

        let (tx, mut rx) = mpsc::channel(batches.len());
        let mut handles: Vec<JoinHandle<()>> = vec![];

        for batch in batches {
            let tx = tx.clone();
            let aggregate_fn = aggregate_fn.clone();

            let handle = tokio::spawn(async move {
                match aggregate_fn(batch).await {
                    Ok(result) => {
                        let _ = tx.send(Ok(result)).await;
                    }
                    Err(e) => {
                        let _ = tx.send(Err(e)).await;
                    }
                }
            });

            handles.push(handle);
        }

        // 关闭发送端
        drop(tx);

        // 收集部分聚合结果
        let mut partial_results = vec![];
        while let Some(result) = rx.recv().await {
            match result {
                Ok(series) => partial_results.push(series),
                Err(e) => {
                    // 取消所有任务
                    for handle in &handles {
                        handle.abort();
                    }
                    return Err(e);
                }
            }
        }

        // 等待所有任务完成
        for handle in handles {
            let _ = handle.await;
        }

        // 最终聚合
        if partial_results.len() == 1 {
            Ok(partial_results.into_iter().next().unwrap())
        } else {
            aggregate_fn(partial_results).await
        }
    }
}

/// 查询并行配置
#[derive(Debug, Clone)]
pub struct ParallelConfig {
    pub enable_parallel: bool,
    pub max_concurrency: usize,
    pub enable_series_parallel: bool,
    pub enable_time_parallel: bool,
    pub min_series_for_parallel: usize,
    pub min_time_range_for_parallel_ms: i64,
}

impl Default for ParallelConfig {
    fn default() -> Self {
        Self {
            enable_parallel: true,
            max_concurrency: num_cpus::get(),
            enable_series_parallel: true,
            enable_time_parallel: true,
            min_series_for_parallel: 10,
            min_time_range_for_parallel_ms: 3600_000, // 1小时
        }
    }
}

/// 并行查询上下文
pub struct ParallelContext {
    pub config: ParallelConfig,
    pub executor: ParallelQueryExecutor,
}

impl ParallelContext {
    pub fn new(config: ParallelConfig) -> Self {
        let executor = ParallelQueryExecutor::new(config.max_concurrency);
        Self { config, executor }
    }

    /// 判断是否使用并行查询
    pub fn should_use_parallel(&self, num_series: usize, time_range_ms: i64) -> bool {
        if !self.config.enable_parallel {
            return false;
        }

        if self.config.enable_series_parallel && num_series >= self.config.min_series_for_parallel {
            return true;
        }

        if self.config.enable_time_parallel && time_range_ms >= self.config.min_time_range_for_parallel_ms {
            return true;
        }

        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Label;

    fn create_test_series(id: TimeSeriesId, num_samples: usize) -> TimeSeries {
        let labels = vec![
            Label::new("__name__", "test_metric"),
            Label::new("id", id.to_string()),
        ];

        let samples: Vec<Sample> = (0..num_samples)
            .map(|i| Sample::new(i as i64 * 1000, i as f64))
            .collect();

        let mut series = TimeSeries::new(id, labels);
        series.add_samples(samples);
        series
    }

    #[tokio::test]
    async fn test_execute_series_parallel() {
        let executor = ParallelQueryExecutor::new(4);

        let series_ids: Vec<TimeSeriesId> = (1..=10).collect();

        let process_fn = |id: TimeSeriesId| async move {
            Ok(Some(create_test_series(id, 10)))
        };

        let results = executor.execute_series_parallel(series_ids, process_fn).await.unwrap();
        assert_eq!(results.len(), 10);
    }

    #[tokio::test]
    async fn test_execute_time_range_parallel() {
        let executor = ParallelQueryExecutor::new(4);

        let process_fn = |start: i64, end: i64| async move {
            let samples: Vec<Sample> = (start..end)
                .step_by(1000)
                .map(|ts| Sample::new(ts, ts as f64))
                .collect();
            Ok(samples)
        };

        let results = executor
            .execute_time_range_parallel(0, 10000, 4, process_fn)
            .await
            .unwrap();

        assert_eq!(results.len(), 4);
    }

    #[tokio::test]
    async fn test_parallel_context() {
        let config = ParallelConfig::default();
        let context = ParallelContext::new(config);

        assert!(context.should_use_parallel(100, 1000));
        assert!(!context.should_use_parallel(5, 1000));
        assert!(context.should_use_parallel(5, 7200_000));
    }
}
