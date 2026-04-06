use crate::columnstore::DownsampleLevel;
use crate::downsample::{DownsampleProcessor, DownsamplePoint, TaskBatch, TaskResult};
use crate::downsample::task::{DownsampleTask, TaskStatus};
use crate::error::Result;
use crate::memstore::MemStore;
use crate::model::{Sample, TimeSeries, TimeSeriesId};
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{info, error, warn, debug};

/// 降采样工作器
pub struct DownsampleWorker {
    worker_id: usize,
    store: Arc<MemStore>,
}

/// 工作器任务
#[derive(Debug, Clone)]
pub struct WorkerTask {
    pub task_id: String,
    pub target_level: DownsampleLevel,
    pub source_level: DownsampleLevel,
    pub start_time: i64,
    pub end_time: i64,
    pub series_ids: Vec<TimeSeriesId>,
}

impl DownsampleWorker {
    pub fn new(
        worker_id: usize,
        store: Arc<MemStore>,
    ) -> Self {
        Self {
            worker_id,
            store,
        }
    }

    /// 处理单个任务
    pub async fn process_task(&self, task: WorkerTask) -> TaskResult {
        let start_time = std::time::Instant::now();
        
        let mut total_processed = 0u64;
        let mut total_generated = 0u64;
        let mut error = None;

        // 获取源数据的分辨率
        let source_resolution = task.source_level.resolution_ms();
        let target_resolution = task.target_level.resolution_ms();

        debug!(
            "Worker {} processing task {}: {:?} -> {:?}, resolution {} -> {}",
            self.worker_id, task.task_id, task.source_level, task.target_level,
            source_resolution, target_resolution
        );

        for series_id in &task.series_ids {
            match self.process_series(
                *series_id,
                task.start_time,
                task.end_time,
                target_resolution,
            ).await {
                Ok((processed, generated)) => {
                    total_processed += processed;
                    total_generated += generated;
                }
                Err(e) => {
                    error!(
                        "Worker {} failed to process series {}: {}",
                        self.worker_id, series_id, e
                    );
                    error = Some(format!("Failed to process series {}: {}", series_id, e));
                    break;
                }
            }
        }

        let duration_ms = start_time.elapsed().as_millis() as i64;
        let success = error.is_none();

        TaskResult {
            task_id: task.task_id,
            success,
            samples_processed: total_processed,
            samples_generated: total_generated,
            error,
            duration_ms,
        }
    }

    /// 处理单个时间序列
    async fn process_series(
        &self,
        series_id: TimeSeriesId,
        start_time: i64,
        end_time: i64,
        target_resolution: i64,
    ) -> Result<(u64, u64)> {
        // 获取时间序列数据
        let samples = match self.store.get_series(series_id) {
            Some(ts) => {
                ts.samples
                    .into_iter()
                    .filter(|s| s.timestamp >= start_time && s.timestamp <= end_time)
                    .collect::<Vec<_>>()
            }
            None => {
                return Ok((0, 0));
            }
        };

        if samples.is_empty() {
            return Ok((0, 0));
        }

        let processed = samples.len() as u64;

        // 执行降采样
        let downsampled = DownsampleProcessor::downseries(&samples, target_resolution);
        let generated = downsampled.len() as u64;

        // 将降采样数据存储到目标级别
        // 注意：这里需要将降采样数据存储到持久化存储中
        // 目前我们只是计算了降采样结果，实际存储逻辑需要根据存储引擎实现
        
        debug!(
            "Worker {} processed series {}: {} samples -> {} points",
            self.worker_id, series_id, processed, generated
        );

        Ok((processed, generated))
    }
}

/// 降采样工作器池
pub struct WorkerPool {
    workers: Vec<tokio::task::JoinHandle<()>>,
    task_tx: mpsc::Sender<WorkerTask>,
    result_rx: mpsc::Receiver<TaskResult>,
}

impl WorkerPool {
    pub fn new(
        num_workers: usize,
        store: Arc<MemStore>,
        task_buffer: usize,
    ) -> Self {
        let (task_tx, mut task_rx) = mpsc::channel::<WorkerTask>(task_buffer);
        let (result_tx, result_rx) = mpsc::channel::<TaskResult>(task_buffer);

        let mut workers = Vec::with_capacity(num_workers);

        // 创建一个任务分发器，将任务分发给多个工作者
        let dispatcher_handle = tokio::spawn(async move {
            let mut worker_id = 0usize;
            while let Some(task) = task_rx.recv().await {
                // 轮询选择工作者
                let current_worker_id = worker_id % num_workers;
                worker_id += 1;
                
                let worker = DownsampleWorker::new(
                    current_worker_id,
                    store.clone(),
                );
                let result_tx = result_tx.clone();
                
                // 为每个任务创建一个异步任务
                tokio::spawn(async move {
                    let result = worker.process_task(task).await;
                    if let Err(e) = result_tx.send(result).await {
                        error!("Failed to send result: {}", e);
                    }
                });
            }
        });

        workers.push(dispatcher_handle);

        Self {
            workers,
            task_tx,
            result_rx,
        }
    }

    /// 提交任务到工作池
    pub async fn submit(&self, task: WorkerTask) -> Result<()> {
        self.task_tx.send(task).await
            .map_err(|e| crate::error::Error::Internal(format!("Failed to submit task: {}", e)))
    }

    /// 接收任务结果
    pub async fn recv_result(&mut self) -> Option<TaskResult> {
        self.result_rx.recv().await
    }

    /// 关闭工作池
    pub async fn shutdown(self) {
        drop(self.task_tx);
        
        for worker in self.workers {
            if let Err(e) = worker.await {
                error!("Worker panicked: {}", e);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::StorageConfig;
    use crate::model::Label;
    use tempfile::tempdir;

    fn create_test_store() -> Arc<MemStore> {
        let temp_dir = tempdir().unwrap();
        let config = StorageConfig {
            data_dir: temp_dir.path().to_string_lossy().to_string(),
            ..Default::default()
        };
        Arc::new(MemStore::new(config).unwrap())
    }

    #[tokio::test]
    async fn test_worker_pool() {
        let store = create_test_store();
        let pool = WorkerPool::new(2, store.clone(), 10);

        // 创建测试数据
        let labels = vec![
            Label::new("__name__", "test_metric"),
            Label::new("job", "test"),
        ];
        let samples: Vec<Sample> = (0..100)
            .map(|i| Sample::new(i * 1000, i as f64))
            .collect();
        
        store.write(labels, samples).unwrap();

        // 提交任务
        let task = WorkerTask {
            task_id: "test_task".to_string(),
            target_level: DownsampleLevel::L1,
            source_level: DownsampleLevel::L0,
            start_time: 0,
            end_time: 100000,
            series_ids: vec![1],
        };

        pool.submit(task).await.unwrap();

        // 等待结果
        let mut pool = pool;
        let result = pool.recv_result().await;
        
        assert!(result.is_some());
        let result = result.unwrap();
        assert!(result.success);
        assert_eq!(result.task_id, "test_task");

        pool.shutdown().await;
    }
}
