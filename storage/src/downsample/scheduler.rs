use crate::columnstore::DownsampleLevel;
use crate::downsample::task::{DownsampleTask, TaskStatus};
use crate::downsample::worker::{WorkerPool, WorkerTask};
use crate::error::Result;
use crate::memstore::MemStore;
use crate::model::TimeSeriesId;
use std::collections::{BinaryHeap, HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::{mpsc, RwLock};
use tokio::time::{interval, sleep};
use tracing::{info, error, warn, debug};

/// 任务优先级
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TaskPriority {
    Low = 0,
    Normal = 1,
    High = 2,
    Critical = 3,
}

/// 待调度任务
struct ScheduledTask {
    task: DownsampleTask,
    priority: TaskPriority,
    enqueue_time: i64,
}

impl PartialEq for ScheduledTask {
    fn eq(&self, other: &Self) -> bool {
        self.task.task_id == other.task.task_id
    }
}

impl Eq for ScheduledTask {}

impl PartialOrd for ScheduledTask {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ScheduledTask {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // 高优先级先执行
        match self.priority.cmp(&other.priority) {
            std::cmp::Ordering::Equal => {
                // 相同优先级，先入队的先执行
                self.enqueue_time.cmp(&other.enqueue_time).reverse()
            }
            ord => ord,
        }
    }
}

/// 降采样任务调度器
pub struct DownsampleScheduler {
    /// 任务队列（优先队列）
    task_queue: Arc<RwLock<BinaryHeap<ScheduledTask>>>,
    /// 正在运行的任务
    running_tasks: Arc<RwLock<HashSet<String>>>,
    /// 所有任务
    all_tasks: Arc<RwLock<HashMap<String, DownsampleTask>>>,
    /// 工作池
    worker_pool: Arc<RwLock<Option<WorkerPool>>>,
    /// 数据目录
    data_dir: PathBuf,
    /// 内存存储
    store: Arc<MemStore>,
    /// 并发数
    concurrency: usize,
    /// 停止信号发送器
    shutdown_tx: Option<mpsc::Sender<()>>,
    /// 是否正在运行
    is_running: Arc<RwLock<bool>>,
}

impl DownsampleScheduler {
    /// 创建新的调度器
    pub fn new(
        store: Arc<MemStore>,
        data_dir: PathBuf,
        concurrency: usize,
    ) -> Self {
        Self {
            task_queue: Arc::new(RwLock::new(BinaryHeap::new())),
            running_tasks: Arc::new(RwLock::new(HashSet::new())),
            all_tasks: Arc::new(RwLock::new(HashMap::new())),
            worker_pool: Arc::new(RwLock::new(None)),
            data_dir,
            store,
            concurrency,
            shutdown_tx: None,
            is_running: Arc::new(RwLock::new(false)),
        }
    }

    /// 启动调度器
    pub async fn start(&mut self) -> Result<()> {
        let mut is_running = self.is_running.write().await;
        if *is_running {
            warn!("Downsample scheduler is already running");
            return Ok(());
        }

        *is_running = true;
        drop(is_running);

        info!("Starting downsample scheduler with concurrency {}", self.concurrency);

        // 创建工作池
        let worker_pool = WorkerPool::new(
            self.concurrency,
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

        // 启动主调度循环
        let task_queue = self.task_queue.clone();
        let running_tasks = self.running_tasks.clone();
        let all_tasks = self.all_tasks.clone();
        let worker_pool = self.worker_pool.clone();
        let is_running = self.is_running.clone();
        let concurrency = self.concurrency;

        tokio::spawn(async move {
            let mut ticker = interval(Duration::from_millis(100));

            loop {
                tokio::select! {
                    _ = ticker.tick() => {
                        if let Err(e) = Self::schedule_tasks(
                            &task_queue,
                            &running_tasks,
                            &all_tasks,
                            &worker_pool,
                            concurrency,
                        ).await {
                            error!("Task scheduling failed: {}", e);
                        }
                    }
                    _ = shutdown_rx.recv() => {
                        info!("Downsample scheduler shutting down");
                        break;
                    }
                    else => {
                        let running = *is_running.read().await;
                        if !running {
                            break;
                        }
                        sleep(Duration::from_millis(100)).await;
                    }
                }
            }

            info!("Downsample scheduler stopped");
        });

        // 启动超时检查循环
        let all_tasks = self.all_tasks.clone();
        let running_tasks = self.running_tasks.clone();
        let is_running = self.is_running.clone();

        tokio::spawn(async move {
            let mut ticker = interval(Duration::from_secs(10));

            loop {
                tokio::select! {
                    _ = ticker.tick() => {
                        Self::check_timeouts(&all_tasks, &running_tasks).await;
                    }
                    else => {
                        let running = *is_running.read().await;
                        if !running {
                            break;
                        }
                        sleep(Duration::from_millis(100)).await;
                    }
                }
            }
        });

        // 启动结果处理循环
        let all_tasks = self.all_tasks.clone();
        let running_tasks = self.running_tasks.clone();
        let worker_pool = self.worker_pool.clone();
        let is_running = self.is_running.clone();

        tokio::spawn(async move {
            loop {
                let result = {
                    let mut pool_guard = worker_pool.write().await;
                    if let Some(ref mut pool) = *pool_guard {
                        pool.recv_result().await
                    } else {
                        break;
                    }
                };

                if let Some(result) = result {
                    Self::handle_task_result(&result, &all_tasks, &running_tasks).await;
                } else {
                    break;
                }

                let running = *is_running.read().await;
                if !running {
                    break;
                }
            }
        });

        info!("Downsample scheduler started successfully");
        Ok(())
    }

    /// 停止调度器
    pub async fn stop(&mut self) -> Result<()> {
        let mut is_running = self.is_running.write().await;
        if !*is_running {
            return Ok(());
        }

        *is_running = false;
        drop(is_running);

        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(()).await;
        }

        let pool = {
            let mut pool_guard = self.worker_pool.write().await;
            pool_guard.take()
        };

        if let Some(pool) = pool {
            pool.shutdown().await;
        }

        info!("Downsample scheduler stopped");
        Ok(())
    }

    /// 提交任务到调度器
    pub async fn submit_task(&self, task: DownsampleTask, priority: TaskPriority) -> Result<String> {
        let task_id = task.task_id.clone();

        {
            let mut all_tasks = self.all_tasks.write().await;
            all_tasks.insert(task_id.clone(), task.clone());
        }

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;

        let scheduled_task = ScheduledTask {
            task,
            priority,
            enqueue_time: now,
        };

        {
            let mut queue = self.task_queue.write().await;
            queue.push(scheduled_task);
        }

        info!("Submitted downsample task {} with priority {:?}", task_id, priority);
        Ok(task_id)
    }

    /// 创建并提交降采样任务
    pub async fn create_and_submit_task(
        &self,
        target_level: DownsampleLevel,
        source_level: DownsampleLevel,
        start_time: i64,
        end_time: i64,
        series_ids: Option<Vec<TimeSeriesId>>,
        priority: TaskPriority,
    ) -> Result<String> {
        let task = DownsampleTask::new(
            target_level,
            source_level,
            start_time,
            end_time,
            series_ids,
        );

        self.submit_task(task, priority).await
    }

    /// 取消任务
    pub async fn cancel_task(&self, task_id: &str) -> Result<bool> {
        {
            let mut all_tasks = self.all_tasks.write().await;
            if let Some(task) = all_tasks.get_mut(task_id) {
                if task.status.is_terminal() {
                    return Ok(false);
                }
                task.mark_cancelled();
            } else {
                return Ok(false);
            }
        }

        {
            let mut running_tasks = self.running_tasks.write().await;
            running_tasks.remove(task_id);
        }

        info!("Cancelled downsample task {}", task_id);
        Ok(true)
    }

    /// 获取任务状态
    pub async fn get_task(&self, task_id: &str) -> Option<DownsampleTask> {
        let all_tasks = self.all_tasks.read().await;
        all_tasks.get(task_id).cloned()
    }

    /// 获取所有任务
    pub async fn get_all_tasks(&self) -> Vec<DownsampleTask> {
        let all_tasks = self.all_tasks.read().await;
        all_tasks.values().cloned().collect()
    }

    /// 调度任务的核心逻辑
    async fn schedule_tasks(
        task_queue: &Arc<RwLock<BinaryHeap<ScheduledTask>>>,
        running_tasks: &Arc<RwLock<HashSet<String>>>,
        all_tasks: &Arc<RwLock<HashMap<String, DownsampleTask>>>,
        worker_pool: &Arc<RwLock<Option<WorkerPool>>>,
        concurrency: usize,
    ) -> Result<()> {
        let current_running = {
            let running = running_tasks.read().await;
            running.len()
        };

        if current_running >= concurrency {
            return Ok(());
        }

        let available_slots = concurrency - current_running;

        for _ in 0..available_slots {
            let scheduled_task = {
                let mut queue = task_queue.write().await;
                queue.pop()
            };

            if let Some(scheduled_task) = scheduled_task {
                let task_id = scheduled_task.task.task_id.clone();

                {
                    let mut all_tasks_guard = all_tasks.write().await;
                    if let Some(task) = all_tasks_guard.get_mut(&task_id) {
                        if task.status.is_terminal() || task.status == TaskStatus::Running {
                            continue;
                        }
                        task.mark_running();
                    } else {
                        continue;
                    }
                }

                {
                    let mut running = running_tasks.write().await;
                    running.insert(task_id.clone());
                }

                let series_ids = scheduled_task.task.series_ids.clone().unwrap_or_else(|| {
                    vec![]
                });

                let worker_task = WorkerTask {
                    task_id: task_id.clone(),
                    target_level: scheduled_task.task.target_level,
                    source_level: scheduled_task.task.source_level,
                    start_time: scheduled_task.task.start_time,
                    end_time: scheduled_task.task.end_time,
                    series_ids,
                };

                {
                    let pool_guard = worker_pool.read().await;
                    if let Some(ref pool) = *pool_guard {
                        pool.submit(worker_task).await?;
                    }
                }

                debug!("Scheduled downsample task {}", task_id);
            } else {
                break;
            }
        }

        Ok(())
    }

    /// 检查超时任务
    async fn check_timeouts(
        all_tasks: &Arc<RwLock<HashMap<String, DownsampleTask>>>,
        running_tasks: &Arc<RwLock<HashSet<String>>>,
    ) {
        let mut timeout_tasks = Vec::new();

        {
            let all_tasks_guard = all_tasks.read().await;
            for (task_id, task) in all_tasks_guard.iter() {
                if task.status == TaskStatus::Running && task.is_timeout() {
                    timeout_tasks.push(task_id.clone());
                }
            }
        }

        for task_id in timeout_tasks {
            {
                let mut all_tasks_guard = all_tasks.write().await;
                if let Some(task) = all_tasks_guard.get_mut(&task_id) {
                    if task.can_retry() {
                        warn!("Downsample task {} timed out, resetting for retry", task_id);
                        task.reset_for_retry();
                    } else {
                        error!("Downsample task {} timed out and exceeded max retries", task_id);
                        task.mark_failed("Task timeout".to_string());
                    }
                }
            }

            {
                let mut running = running_tasks.write().await;
                running.remove(&task_id);
            }
        }
    }

    /// 处理任务结果
    async fn handle_task_result(
        result: &crate::downsample::TaskResult,
        all_tasks: &Arc<RwLock<HashMap<String, DownsampleTask>>>,
        running_tasks: &Arc<RwLock<HashSet<String>>>,
    ) {
        {
            let mut all_tasks_guard = all_tasks.write().await;
            if let Some(task) = all_tasks_guard.get_mut(&result.task_id) {
                if result.success {
                    task.mark_completed(result.samples_processed, result.samples_generated);
                } else {
                    if task.can_retry() {
                        task.mark_failed(result.error.clone().unwrap_or_else(|| "Unknown error".to_string()));
                        task.reset_for_retry();
                    } else {
                        task.mark_failed(result.error.clone().unwrap_or_else(|| "Unknown error".to_string()));
                    }
                }
            }
        }

        {
            let mut running = running_tasks.write().await;
            running.remove(&result.task_id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::StorageConfig;
    use tempfile::tempdir;

    fn create_test_scheduler() -> (DownsampleScheduler, Arc<MemStore>) {
        let temp_dir = tempdir().unwrap();
        let data_dir = temp_dir.path().to_path_buf();
        let config = StorageConfig {
            data_dir: data_dir.to_string_lossy().to_string(),
            ..Default::default()
        };
        let store = Arc::new(MemStore::new(config).unwrap());
        
        let scheduler = DownsampleScheduler::new(store.clone(), data_dir, 2);
        (scheduler, store)
    }

    #[tokio::test]
    async fn test_scheduler_start_stop() {
        let (mut scheduler, _) = create_test_scheduler();

        scheduler.start().await.unwrap();
        assert!(*scheduler.is_running.read().await);

        scheduler.stop().await.unwrap();
        assert!(!*scheduler.is_running.read().await);
    }

    #[tokio::test]
    async fn test_submit_task() {
        let (scheduler, _) = create_test_scheduler();

        let task_id = scheduler.create_and_submit_task(
            DownsampleLevel::L1,
            DownsampleLevel::L0,
            0,
            1000,
            None,
            TaskPriority::Normal,
        ).await.unwrap();

        assert!(task_id.starts_with("ds_"));

        let task = scheduler.get_task(&task_id).await;
        assert!(task.is_some());
        assert_eq!(task.unwrap().target_level, DownsampleLevel::L1);
    }

    #[tokio::test]
    async fn test_cancel_task() {
        let (scheduler, _) = create_test_scheduler();

        let task_id = scheduler.create_and_submit_task(
            DownsampleLevel::L1,
            DownsampleLevel::L0,
            0,
            1000,
            None,
            TaskPriority::Normal,
        ).await.unwrap();

        let cancelled = scheduler.cancel_task(&task_id).await.unwrap();
        assert!(cancelled);

        let task = scheduler.get_task(&task_id).await;
        assert!(task.is_some());
        assert_eq!(task.unwrap().status, TaskStatus::Cancelled);
    }
}
