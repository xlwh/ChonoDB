use crate::error::Result;
use crate::model::{Sample, TimeSeries, TimeSeriesId};
use tokio::sync::{mpsc, Semaphore, Mutex};
use tokio::task::JoinHandle;
use tokio::time::Instant;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

/// 并行查询执行器
pub struct ParallelQueryExecutor {
    max_concurrency: usize,
    active_tasks: Arc<AtomicUsize>,
    semaphore: Arc<Semaphore>,
}

/// 并行执行统计
#[derive(Debug, Clone, Default)]
pub struct ParallelStats {
    pub total_tasks: usize,
    pub completed_tasks: usize,
    pub failed_tasks: usize,
    pub execution_time_ms: u64,
    pub max_concurrency_used: usize,
}

impl ParallelQueryExecutor {
    pub fn new(max_concurrency: usize) -> Self {
        let max_concurrency = std::cmp::max(1, max_concurrency);
        Self {
            max_concurrency,
            active_tasks: Arc::new(AtomicUsize::new(0)),
            semaphore: Arc::new(Semaphore::new(max_concurrency)),
        }
    }

    /// 并行执行多个时间序列的查询
    pub async fn execute_series_parallel<F, Fut>(
        &self,
        series_ids: Vec<TimeSeriesId>,
        process_fn: F,
    ) -> Result<(Vec<TimeSeries>, ParallelStats)>
    where
        F: Fn(TimeSeriesId) -> Fut + Send + Sync + Clone + 'static,
        Fut: std::future::Future<Output = Result<Option<TimeSeries>>> + Send,
    {
        if series_ids.is_empty() {
            return Ok((vec![], ParallelStats::default()));
        }

        let start_time = Instant::now();
        let (tx, mut rx) = mpsc::channel(self.max_concurrency * 2);
        let stats = Arc::new(Mutex::new(ParallelStats::default()));
        let max_concurrency_used = Arc::new(AtomicUsize::new(0));

        let total_tasks = series_ids.len();
        let mut stats_lock = stats.lock().await;
        stats_lock.total_tasks = total_tasks;
        drop(stats_lock);

        let mut handles: Vec<JoinHandle<()>> = vec![];

        for series_id in series_ids {
            let tx = tx.clone();
            let process_fn = process_fn.clone();
            let semaphore = self.semaphore.clone();
            let active_tasks = self.active_tasks.clone();
            let stats = stats.clone();
            let max_concurrency_used = max_concurrency_used.clone();

            let handle = tokio::spawn(async move {
                // 获取信号量许可
                let _permit = semaphore.acquire().await.unwrap();
                
                // 更新活跃任务数和最大并发度
                let current = active_tasks.fetch_add(1, Ordering::SeqCst) + 1;
                max_concurrency_used.fetch_max(current, Ordering::SeqCst);

                let result = process_fn(series_id).await;
                
                // 减少活跃任务数
                active_tasks.fetch_sub(1, Ordering::SeqCst);

                match result {
                    Ok(Some(series)) => {
                        if tx.send(Ok(series)).await.is_err() {
                            return;
                        }
                        let mut stats_lock = stats.lock().await;
                        stats_lock.completed_tasks += 1;
                    }
                    Ok(None) => {
                        let mut stats_lock = stats.lock().await;
                        stats_lock.completed_tasks += 1;
                    }
                    Err(e) => {
                        let _ = tx.send(Err(e)).await;
                        let mut stats_lock = stats.lock().await;
                        stats_lock.failed_tasks += 1;
                    }
                }
            });

            handles.push(handle);
        }

        // 关闭发送端
        drop(tx);

        // 收集结果
        let mut results = vec![];
        let mut error: Option<Result<Vec<TimeSeries>>> = None;

        while let Some(result) = rx.recv().await {
            match result {
                Ok(series) => results.push(series),
                Err(e) => {
                    // 只保存第一个错误
                    if error.is_none() {
                        error = Some(Err(e));
                    }
                    // 继续接收其他结果，直到所有任务完成
                }
            }
        }

        // 等待所有任务完成
        for handle in handles {
            let _ = handle.await;
        }

        // 检查是否有错误
        if let Some(Err(e)) = error {
            return Err(e);
        }

        let execution_time_ms = start_time.elapsed().as_millis() as u64;
        let mut stats_lock = stats.lock().await;
        stats_lock.execution_time_ms = execution_time_ms;
        stats_lock.max_concurrency_used = max_concurrency_used.load(Ordering::SeqCst);
        let stats_clone = stats_lock.clone();

        Ok((results, stats_clone))
    }

    /// 并行处理时间范围分片
    pub async fn execute_time_range_parallel<F, Fut>(
        &self,
        start: i64,
        end: i64,
        num_shards: usize,
        process_fn: F,
    ) -> Result<(Vec<Vec<Sample>>, ParallelStats)>
    where
        F: Fn(i64, i64) -> Fut + Send + Sync + Clone + 'static,
        Fut: std::future::Future<Output = Result<Vec<Sample>>> + Send,
    {
        if start >= end || num_shards == 0 {
            return Ok((vec![], ParallelStats::default()));
        }

        let start_time = Instant::now();
        let range = end - start;
        let shard_size = range / num_shards as i64;

        if shard_size == 0 {
            // 范围太小，不分片
            let samples = process_fn(start, end).await?;
            let stats = ParallelStats {
                total_tasks: 1,
                completed_tasks: 1,
                failed_tasks: 0,
                execution_time_ms: start_time.elapsed().as_millis() as u64,
                max_concurrency_used: 1,
            };
            return Ok((vec![samples], stats));
        }

        let (tx, mut rx) = mpsc::channel(num_shards);
        let stats = Arc::new(Mutex::new(ParallelStats::default()));
        let max_concurrency_used = Arc::new(AtomicUsize::new(0));

        let total_tasks = num_shards;
        let mut stats_lock = stats.lock().await;
        stats_lock.total_tasks = total_tasks;
        drop(stats_lock);

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
            let semaphore = self.semaphore.clone();
            let active_tasks = self.active_tasks.clone();
            let stats = stats.clone();
            let max_concurrency_used = max_concurrency_used.clone();

            let handle = tokio::spawn(async move {
                // 获取信号量许可
                let _permit = semaphore.acquire().await.unwrap();
                
                // 更新活跃任务数和最大并发度
                let current = active_tasks.fetch_add(1, Ordering::SeqCst) + 1;
                max_concurrency_used.fetch_max(current, Ordering::SeqCst);

                let result = process_fn(shard_start, shard_end).await;
                
                // 减少活跃任务数
                active_tasks.fetch_sub(1, Ordering::SeqCst);

                match result {
                    Ok(samples) => {
                        let _ = tx.send(Ok((i, samples))).await;
                        let mut stats_lock = stats.lock().await;
                        stats_lock.completed_tasks += 1;
                    }
                    Err(e) => {
                        let _ = tx.send(Err(e)).await;
                        let mut stats_lock = stats.lock().await;
                        stats_lock.failed_tasks += 1;
                    }
                }
            });

            handles.push(handle);
        }

        // 关闭发送端
        drop(tx);

        // 收集结果并按顺序排列
        let mut shard_results: Vec<Option<Vec<Sample>>> = vec![None; num_shards];
        let mut error: Option<Result<Vec<Vec<Sample>>>> = None;
        
        while let Some(result) = rx.recv().await {
            match result {
                Ok((shard_idx, samples)) => {
                    shard_results[shard_idx] = Some(samples);
                }
                Err(e) => {
                    // 只保存第一个错误
                    if error.is_none() {
                        error = Some(Err(e));
                    }
                    // 继续接收其他结果，直到所有任务完成
                }
            }
        }

        // 等待所有任务完成
        for handle in handles {
            let _ = handle.await;
        }

        // 检查是否有错误
        if let Some(Err(e)) = error {
            return Err(e);
        }

        // 按顺序合并结果
        let mut all_samples = vec![];
        for samples in shard_results.into_iter().flatten() {
            all_samples.push(samples);
        }

        let execution_time_ms = start_time.elapsed().as_millis() as u64;
        let mut stats_lock = stats.lock().await;
        stats_lock.execution_time_ms = execution_time_ms;
        stats_lock.max_concurrency_used = max_concurrency_used.load(Ordering::SeqCst);
        let stats_clone = stats_lock.clone();

        Ok((all_samples, stats_clone))
    }

    /// 并行聚合操作
    pub async fn parallel_aggregate<F, Fut>(
        &self,
        series_list: Vec<TimeSeries>,
        aggregate_fn: F,
    ) -> Result<(TimeSeries, ParallelStats)>
    where
        F: Fn(Vec<TimeSeries>) -> Fut + Send + Sync + Clone + 'static,
        Fut: std::future::Future<Output = Result<TimeSeries>> + Send,
    {
        let start_time = Instant::now();
        
        if series_list.is_empty() {
            return Err(crate::error::Error::InvalidData(
                "No series to aggregate".to_string()
            ));
        }

        if series_list.len() == 1 {
            let stats = ParallelStats {
                total_tasks: 1,
                completed_tasks: 1,
                failed_tasks: 0,
                execution_time_ms: start_time.elapsed().as_millis() as u64,
                max_concurrency_used: 1,
            };
            return Ok((series_list.into_iter().next().unwrap(), stats));
        }

        // 分批聚合
        let batch_size = (series_list.len() + self.max_concurrency - 1) / self.max_concurrency;
        let batches: Vec<Vec<TimeSeries>> = series_list
            .chunks(batch_size.max(1))
            .map(|chunk| chunk.to_vec())
            .collect();

        let (tx, mut rx) = mpsc::channel(batches.len());
        let stats = Arc::new(Mutex::new(ParallelStats::default()));
        let max_concurrency_used = Arc::new(AtomicUsize::new(0));

        let total_tasks = batches.len();
        let mut stats_lock = stats.lock().await;
        stats_lock.total_tasks = total_tasks;
        drop(stats_lock);

        let mut handles: Vec<JoinHandle<()>> = vec![];

        for batch in batches {
            let tx = tx.clone();
            let aggregate_fn = aggregate_fn.clone();
            let semaphore = self.semaphore.clone();
            let active_tasks = self.active_tasks.clone();
            let stats = stats.clone();
            let max_concurrency_used = max_concurrency_used.clone();

            let handle = tokio::spawn(async move {
                // 获取信号量许可
                let _permit = semaphore.acquire().await.unwrap();
                
                // 更新活跃任务数和最大并发度
                let current = active_tasks.fetch_add(1, Ordering::SeqCst) + 1;
                max_concurrency_used.fetch_max(current, Ordering::SeqCst);

                let result = aggregate_fn(batch).await;
                
                // 减少活跃任务数
                active_tasks.fetch_sub(1, Ordering::SeqCst);

                match result {
                    Ok(result) => {
                        let _ = tx.send(Ok(result)).await;
                        let mut stats_lock = stats.lock().await;
                        stats_lock.completed_tasks += 1;
                    }
                    Err(e) => {
                        let _ = tx.send(Err(e)).await;
                        let mut stats_lock = stats.lock().await;
                        stats_lock.failed_tasks += 1;
                    }
                }
            });

            handles.push(handle);
        }

        // 关闭发送端
        drop(tx);

        // 收集部分聚合结果
        let mut partial_results = vec![];
        let mut error: Option<Result<TimeSeries>> = None;

        while let Some(result) = rx.recv().await {
            match result {
                Ok(series) => partial_results.push(series),
                Err(e) => {
                    // 只保存第一个错误
                    if error.is_none() {
                        error = Some(Err(e));
                    }
                    // 继续接收其他结果，直到所有任务完成
                }
            }
        }

        // 等待所有任务完成
        for handle in handles {
            let _ = handle.await;
        }

        // 检查是否有错误
        if let Some(Err(e)) = error {
            return Err(e);
        }

        // 最终聚合
        let final_result = if partial_results.len() == 1 {
            partial_results.into_iter().next().unwrap()
        } else {
            aggregate_fn(partial_results).await?
        };

        let execution_time_ms = start_time.elapsed().as_millis() as u64;
        let mut stats_lock = stats.lock().await;
        stats_lock.execution_time_ms = execution_time_ms;
        stats_lock.max_concurrency_used = max_concurrency_used.load(Ordering::SeqCst);
        let stats_clone = stats_lock.clone();

        Ok((final_result, stats_clone))
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
    pub stats_history: Vec<ParallelStats>,
}

impl ParallelContext {
    pub fn new(config: ParallelConfig) -> Self {
        let executor = ParallelQueryExecutor::new(config.max_concurrency);
        Self {
            config,
            executor,
            stats_history: Vec::new(),
        }
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

    /// 记录执行统计信息
    pub fn record_stats(&mut self, stats: ParallelStats) {
        self.stats_history.push(stats);
        // 只保留最近 10 条统计信息
        if self.stats_history.len() > 10 {
            self.stats_history.remove(0);
        }
    }

    /// 根据历史统计信息调整并行度
    pub fn adjust_concurrency(&mut self) {
        if self.stats_history.len() < 3 {
            return;
        }

        // 计算平均执行时间和平均并发度
        let avg_execution_time: u64 = self.stats_history
            .iter()
            .map(|s| s.execution_time_ms)
            .sum::<u64>() / self.stats_history.len() as u64;

        let avg_concurrency: usize = self.stats_history
            .iter()
            .map(|s| s.max_concurrency_used)
            .sum::<usize>() / self.stats_history.len();

        // 根据执行时间调整并行度
        if avg_execution_time > 1000 && avg_concurrency < self.config.max_concurrency {
            // 执行时间较长，尝试增加并发度
            let new_concurrency = std::cmp::min(avg_concurrency + 1, self.config.max_concurrency);
            self.executor = ParallelQueryExecutor::new(new_concurrency);
        } else if avg_execution_time < 100 && avg_concurrency > 1 {
            // 执行时间较短，尝试减少并发度
            let new_concurrency = std::cmp::max(avg_concurrency - 1, 1);
            self.executor = ParallelQueryExecutor::new(new_concurrency);
        }
    }

    /// 获取推荐的时间分片数
    pub fn get_recommended_shards(&self, time_range_ms: i64) -> usize {
        if !self.config.enable_time_parallel {
            return 1;
        }

        // 根据时间范围和最大并发度计算推荐的分片数
        let base_shards = std::cmp::min(
            (time_range_ms / self.config.min_time_range_for_parallel_ms) as usize,
            self.config.max_concurrency * 2
        );
        
        std::cmp::max(base_shards, 1)
    }

    /// 获取推荐的批次大小
    pub fn get_recommended_batch_size(&self, total_tasks: usize) -> usize {
        if !self.config.enable_parallel {
            return total_tasks;
        }

        let max_concurrency = self.config.max_concurrency;
        let batch_size = (total_tasks + max_concurrency - 1) / max_concurrency;
        
        std::cmp::max(batch_size, 1)
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
        assert_eq!(results.0.len(), 10);
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

        assert_eq!(results.0.len(), 4);
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
