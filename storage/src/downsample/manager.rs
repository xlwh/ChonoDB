use crate::columnstore::DownsampleLevel;
use crate::downsample::{
    DownsampleConfig, DownsampleStats, LevelStats, TaskResult,
};
use crate::downsample::task::{DownsampleTask, TaskStatus};
use crate::downsample::worker::{WorkerPool, WorkerTask};
use crate::error::Result;
use crate::memstore::MemStore;
use crate::model::TimeSeriesId;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::{mpsc, RwLock};
use tokio::time::{interval, sleep};
use tracing::{info, error, warn, debug};

/// 降采样管理器
pub struct DownsampleManager {
    config: DownsampleConfig,
    store: Arc<MemStore>,
    data_dir: std::path::PathBuf,
    tasks: Arc<RwLock<HashMap<String, DownsampleTask>>>,
    stats: Arc<RwLock<DownsampleStats>>,
    worker_pool: Arc<RwLock<Option<WorkerPool>>>,
    shutdown_tx: Option<mpsc::Sender<()>>,
    is_running: Arc<RwLock<bool>>,
}

impl DownsampleManager {
    pub fn new(config: DownsampleConfig, store: Arc<MemStore>, data_dir: std::path::PathBuf) -> Self {
        Self {
            config,
            store,
            data_dir,
            tasks: Arc::new(RwLock::new(HashMap::new())),
            stats: Arc::new(RwLock::new(DownsampleStats::default())),
            worker_pool: Arc::new(RwLock::new(None)),
            shutdown_tx: None,
            is_running: Arc::new(RwLock::new(false)),
        }
    }

    /// 启动降采样管理器
    pub async fn start(&mut self) -> Result<()> {
        if !self.config.enabled {
            info!("Downsample manager is disabled");
            return Ok(());
        }

        let mut is_running = self.is_running.write().await;
        if *is_running {
            warn!("Downsample manager is already running");
            return Ok(());
        }

        *is_running = true;
        drop(is_running);

        info!("Starting downsample manager with {} workers", self.config.concurrency);

        // 创建工作池
        let worker_pool = WorkerPool::new(
            self.config.concurrency,
            self.store.clone(),
            self.data_dir.clone(),
            100,
        );

        {
            let mut pool = self.worker_pool.write().await;
            *pool = Some(worker_pool);
        }

        // 创建关闭信号通道
        let (shutdown_tx, mut shutdown_rx) = mpsc::channel::<()>(1);
        self.shutdown_tx = Some(shutdown_tx);

        // 启动后台任务调度循环
        let config = self.config.clone();
        let tasks = self.tasks.clone();
        let stats = self.stats.clone();
        let worker_pool = self.worker_pool.clone();
        let is_running = self.is_running.clone();
        let store = self.store.clone();

        tokio::spawn(async move {
            let mut interval_timer = interval(config.interval);

            loop {
                tokio::select! {
                    _ = interval_timer.tick() => {
                        if let Err(e) = Self::run_scheduled_tasks(
                            &config,
                            &store,
                            &tasks,
                            &stats,
                            &worker_pool,
                        ).await {
                            error!("Scheduled downsample task failed: {}", e);
                        }
                    }
                    _ = shutdown_rx.recv() => {
                        info!("Downsample manager shutting down");
                        break;
                    }
                    else => {
                        // 检查是否还在运行
                        let running = *is_running.read().await;
                        if !running {
                            break;
                        }
                        sleep(Duration::from_secs(1)).await;
                    }
                }
            }

            info!("Downsample manager stopped");
        });

        // 启动结果处理循环
        let result_tasks = self.tasks.clone();
        let result_stats = self.stats.clone();
        let result_pool = self.worker_pool.clone();

        tokio::spawn(async move {
            loop {
                let result = {
                    let mut pool_guard = result_pool.write().await;
                    if let Some(ref mut pool) = *pool_guard {
                        pool.recv_result().await
                    } else {
                        break;
                    }
                };

                if let Some(result) = result {
                    Self::handle_task_result(&result, &result_tasks, &result_stats).await;
                } else {
                    // 工作池已关闭
                    break;
                }
            }
        });

        info!("Downsample manager started successfully");
        Ok(())
    }

    /// 停止降采样管理器
    pub async fn stop(&mut self) -> Result<()> {
        let mut is_running = self.is_running.write().await;
        if !*is_running {
            return Ok(());
        }

        *is_running = false;
        drop(is_running);

        // 发送关闭信号
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(()).await;
        }

        // 关闭工作池
        let pool = {
            let mut pool_guard = self.worker_pool.write().await;
            pool_guard.take()
        };

        if let Some(pool) = pool {
            pool.shutdown().await;
        }

        info!("Downsample manager stopped");
        Ok(())
    }

    /// 创建并提交降采样任务
    pub async fn create_task(
        &self,
        target_level: DownsampleLevel,
        source_level: DownsampleLevel,
        start_time: i64,
        end_time: i64,
        series_ids: Option<Vec<TimeSeriesId>>,
    ) -> Result<String> {
        let task = DownsampleTask::new(
            target_level,
            source_level,
            start_time,
            end_time,
            series_ids,
        );

        let task_id = task.task_id.clone();

        {
            let mut tasks = self.tasks.write().await;
            tasks.insert(task_id.clone(), task);
        }

        // 更新统计
        {
            let mut stats = self.stats.write().await;
            stats.total_tasks += 1;
        }

        info!(
            "Created downsample task {}: {:?} -> {:?}, time range {} to {}",
            task_id, source_level, target_level, start_time, end_time
        );

        Ok(task_id)
    }

    /// 获取任务状态
    pub async fn get_task_status(&self, task_id: &str) -> Option<TaskStatus> {
        let tasks = self.tasks.read().await;
        tasks.get(task_id).map(|t| t.status)
    }

    /// 获取任务详情
    pub async fn get_task(&self, task_id: &str) -> Option<DownsampleTask> {
        let tasks = self.tasks.read().await;
        tasks.get(task_id).cloned()
    }

    /// 获取所有任务
    pub async fn get_all_tasks(&self) -> Vec<DownsampleTask> {
        let tasks = self.tasks.read().await;
        tasks.values().cloned().collect()
    }

    /// 获取统计信息
    pub async fn get_stats(&self) -> DownsampleStats {
        self.stats.read().await.clone()
    }

    /// 手动触发降采样任务
    pub async fn trigger_downsample(
        &self,
        target_level: DownsampleLevel,
        start_time: i64,
        end_time: i64,
    ) -> Result<String> {
        // 确定源级别
        let source_level = match target_level {
            DownsampleLevel::L1 => DownsampleLevel::L0,
            DownsampleLevel::L2 => DownsampleLevel::L1,
            DownsampleLevel::L3 => DownsampleLevel::L2,
            DownsampleLevel::L4 => DownsampleLevel::L3,
            DownsampleLevel::L0 => {
                return Err(crate::error::Error::InvalidData(
                    "Cannot downsample from L0 to L0".to_string()
                ));
            }
        };

        self.create_task(target_level, source_level, start_time, end_time, None).await
    }

    /// 执行定时任务
    async fn run_scheduled_tasks(
        config: &DownsampleConfig,
        store: &Arc<MemStore>,
        tasks: &Arc<RwLock<HashMap<String, DownsampleTask>>>,
        stats: &Arc<RwLock<DownsampleStats>>,
        worker_pool: &Arc<RwLock<Option<WorkerPool>>>,
    ) -> Result<()> {
        debug!("Running scheduled downsample tasks");

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;

        // 为每个启用的级别创建任务
        for level_config in &config.levels {
            if !level_config.enabled {
                continue;
            }

            let target_level = level_config.level;
            let source_level = match target_level {
                DownsampleLevel::L1 => DownsampleLevel::L0,
                DownsampleLevel::L2 => DownsampleLevel::L1,
                DownsampleLevel::L3 => DownsampleLevel::L2,
                DownsampleLevel::L4 => DownsampleLevel::L3,
                DownsampleLevel::L0 => continue,
            };

            // 计算时间范围
            let retention_ms = target_level.retention_days() as i64 * 24 * 3600 * 1000;
            let start_time = now - retention_ms;
            let end_time = now;

            // 创建任务
            let task = DownsampleTask::new(
                target_level,
                source_level,
                start_time,
                end_time,
                None,
            );

            let task_id = task.task_id.clone();

            {
                let mut tasks_guard = tasks.write().await;
                tasks_guard.insert(task_id.clone(), task);
            }

            // 提交任务到工作池
            let worker_task = WorkerTask {
                task_id: task_id.clone(),
                target_level,
                source_level,
                start_time,
                end_time,
                series_ids: Self::get_all_series_ids(store).await,
            };

            {
                let pool_guard = worker_pool.read().await;
                if let Some(ref pool) = *pool_guard {
                    pool.submit(worker_task).await?;
                }
            }

            // 更新统计
            {
                let mut stats_guard = stats.write().await;
                stats_guard.total_tasks += 1;
            }

            info!(
                "Scheduled downsample task {} for level {:?}",
                task_id, target_level
            );
        }

        // 更新最后运行时间
        {
            let mut stats_guard = stats.write().await;
            stats_guard.last_run_timestamp = Some(now);
        }

        Ok(())
    }

    /// 处理任务结果
    async fn handle_task_result(
        result: &TaskResult,
        tasks: &Arc<RwLock<HashMap<String, DownsampleTask>>>,
        stats: &Arc<RwLock<DownsampleStats>>,
    ) {
        let mut tasks_guard = tasks.write().await;
        
        if let Some(task) = tasks_guard.get_mut(&result.task_id) {
            if result.success {
                task.mark_completed(result.samples_processed, result.samples_generated);
                
                // 更新统计
                let mut stats_guard = stats.write().await;
                stats_guard.completed_tasks += 1;
                stats_guard.total_samples_processed += result.samples_processed;
                stats_guard.total_samples_generated += result.samples_generated;
                
                // 更新级别统计
                let level_stats = stats_guard.level_stats
                    .entry(task.target_level)
                    .or_insert_with(LevelStats::default);
                level_stats.samples_processed += result.samples_processed;
                level_stats.samples_generated += result.samples_generated;
                level_stats.task_count += 1;
            } else {
                let error = result.error.clone().unwrap_or_else(|| "Unknown error".to_string());
                task.mark_failed(error);
                
                // 更新统计
                let mut stats_guard = stats.write().await;
                stats_guard.failed_tasks += 1;
            }
        }
    }

    /// 获取所有系列ID
    async fn get_all_series_ids(store: &Arc<MemStore>) -> Vec<TimeSeriesId> {
        // 从存储中获取所有系列ID
        store.get_all_series_ids()
    }

    /// 清理已完成的任务
    pub async fn cleanup_completed_tasks(&self, max_age: Duration) -> Result<usize> {
        let cutoff = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64
            - max_age.as_millis() as i64;

        let mut tasks = self.tasks.write().await;
        let before_count = tasks.len();

        tasks.retain(|_, task| {
            !task.status.is_terminal() || task.completed_at.map_or(true, |t| t > cutoff)
        });

        let removed = before_count - tasks.len();
        info!("Cleaned up {} completed downsample tasks", removed);

        Ok(removed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::StorageConfig;
    use crate::downsample::LevelConfig;
    use tempfile::tempdir;

    fn create_test_manager() -> (DownsampleManager, Arc<MemStore>) {
        let temp_dir = tempdir().unwrap();
        let data_dir = temp_dir.path().to_path_buf();
        let config = StorageConfig {
            data_dir: data_dir.to_string_lossy().to_string(),
            ..Default::default()
        };
        let store = Arc::new(MemStore::new(config).unwrap());
        
        let ds_config = DownsampleConfig {
            enabled: true,
            interval: Duration::from_secs(1), // 1秒间隔用于测试
            concurrency: 2,
            timeout: Duration::from_secs(60),
            levels: vec![
                LevelConfig {
                    level: DownsampleLevel::L1,
                    enabled: true,
                    functions: vec!["avg".to_string()],
                },
            ],
        };

        let manager = DownsampleManager::new(ds_config, store.clone(), data_dir);
        (manager, store)
    }

    #[tokio::test]
    async fn test_manager_start_stop() {
        let (mut manager, _) = create_test_manager();

        // 启动管理器
        manager.start().await.unwrap();
        assert!(*manager.is_running.read().await);

        // 停止管理器
        manager.stop().await.unwrap();
        assert!(!*manager.is_running.read().await);
    }

    #[tokio::test]
    async fn test_create_task() {
        let (manager, _) = create_test_manager();

        let task_id = manager.create_task(
            DownsampleLevel::L1,
            DownsampleLevel::L0,
            0,
            1000,
            None,
        ).await.unwrap();

        assert!(task_id.starts_with("ds_"));

        let task = manager.get_task(&task_id).await;
        assert!(task.is_some());
        assert_eq!(task.unwrap().target_level, DownsampleLevel::L1);
    }

    #[tokio::test]
    async fn test_get_stats() {
        let (manager, _) = create_test_manager();

        // 创建一些任务
        for _ in 0..3 {
            manager.create_task(
                DownsampleLevel::L1,
                DownsampleLevel::L0,
                0,
                1000,
                None,
            ).await.unwrap();
        }

        let stats = manager.get_stats().await;
        assert_eq!(stats.total_tasks, 3);
    }
}
