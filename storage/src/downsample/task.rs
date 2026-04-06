use crate::columnstore::DownsampleLevel;
use crate::model::TimeSeriesId;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tracing::{info, error, warn};

/// 任务状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

impl TaskStatus {
    pub fn is_terminal(&self) -> bool {
        matches!(self, TaskStatus::Completed | TaskStatus::Failed | TaskStatus::Cancelled)
    }
}

/// 降采样任务配置
#[derive(Debug, Clone)]
pub struct TaskConfig {
    /// 任务超时时间
    pub timeout: Duration,
    /// 最大重试次数
    pub max_retries: u32,
    /// 批处理大小
    pub batch_size: usize,
    /// 并发数
    pub concurrency: usize,
}

impl Default for TaskConfig {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(3600), // 1小时
            max_retries: 3,
            batch_size: 1000,
            concurrency: 4,
        }
    }
}

/// 降采样任务
#[derive(Debug, Clone)]
pub struct DownsampleTask {
    /// 任务ID
    pub task_id: String,
    /// 目标降采样级别
    pub target_level: DownsampleLevel,
    /// 源级别（从哪个级别降采样）
    pub source_level: DownsampleLevel,
    /// 时间范围 - 开始
    pub start_time: i64,
    /// 时间范围 - 结束
    pub end_time: i64,
    /// 特定的系列ID列表（None表示所有系列）
    pub series_ids: Option<Vec<TimeSeriesId>>,
    /// 任务状态
    pub status: TaskStatus,
    /// 创建时间
    pub created_at: i64,
    /// 开始时间
    pub started_at: Option<i64>,
    /// 完成时间
    pub completed_at: Option<i64>,
    /// 处理的样本数
    pub samples_processed: u64,
    /// 生成的样本数
    pub samples_generated: u64,
    /// 错误信息
    pub error_message: Option<String>,
    /// 重试次数
    pub retry_count: u32,
    /// 任务配置
    pub config: TaskConfig,
}

impl DownsampleTask {
    /// 创建新的降采样任务
    pub fn new(
        target_level: DownsampleLevel,
        source_level: DownsampleLevel,
        start_time: i64,
        end_time: i64,
        series_ids: Option<Vec<TimeSeriesId>>,
    ) -> Self {
        let task_id = format!(
            "ds_{}_{}_{}_{}",
            target_level as u8,
            start_time,
            end_time,
            uuid::Uuid::new_v4().to_string().split('-').next().unwrap_or("")
        );

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;

        Self {
            task_id,
            target_level,
            source_level,
            start_time,
            end_time,
            series_ids,
            status: TaskStatus::Pending,
            created_at: now,
            started_at: None,
            completed_at: None,
            samples_processed: 0,
            samples_generated: 0,
            error_message: None,
            retry_count: 0,
            config: TaskConfig::default(),
        }
    }

    /// 设置任务配置
    pub fn with_config(mut self, config: TaskConfig) -> Self {
        self.config = config;
        self
    }

    /// 标记任务为运行中
    pub fn mark_running(&mut self) {
        self.status = TaskStatus::Running;
        self.started_at = Some(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as i64,
        );
        info!(
            "Downsample task {} started: {:?} -> {:?}, time range: {} to {}",
            self.task_id, self.source_level, self.target_level, self.start_time, self.end_time
        );
    }

    /// 标记任务为完成
    pub fn mark_completed(&mut self, samples_processed: u64, samples_generated: u64) {
        self.status = TaskStatus::Completed;
        self.completed_at = Some(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as i64,
        );
        self.samples_processed = samples_processed;
        self.samples_generated = samples_generated;
        
        let duration = self.started_at.map(|start| {
            self.completed_at.unwrap_or(start) - start
        });
        
        info!(
            "Downsample task {} completed: processed {} samples, generated {} samples, took {:?}ms",
            self.task_id, samples_processed, samples_generated, duration
        );
    }

    /// 标记任务为失败
    pub fn mark_failed(&mut self, error: String) {
        self.status = TaskStatus::Failed;
        self.error_message = Some(error.clone());
        self.retry_count += 1;
        
        error!(
            "Downsample task {} failed: {}, retry count: {}/{}",
            self.task_id, error, self.retry_count, self.config.max_retries
        );
    }

    /// 标记任务为取消
    pub fn mark_cancelled(&mut self) {
        self.status = TaskStatus::Cancelled;
        warn!("Downsample task {} cancelled", self.task_id);
    }

    /// 检查是否可以重试
    pub fn can_retry(&self) -> bool {
        self.status == TaskStatus::Failed && self.retry_count < self.config.max_retries
    }

    /// 重置任务状态（用于重试）
    pub fn reset_for_retry(&mut self) {
        if self.can_retry() {
            self.status = TaskStatus::Pending;
            self.error_message = None;
            info!("Downsample task {} reset for retry {}/{}", 
                self.task_id, self.retry_count + 1, self.config.max_retries);
        }
    }

    /// 获取任务持续时间（毫秒）
    pub fn duration_ms(&self) -> Option<i64> {
        match (self.started_at, self.completed_at) {
            (Some(start), Some(end)) => Some(end - start),
            (Some(start), None) => Some(
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as i64 - start,
            ),
            _ => None,
        }
    }

    /// 检查任务是否超时
    pub fn is_timeout(&self) -> bool {
        if self.status != TaskStatus::Running {
            return false;
        }

        if let Some(started) = self.started_at {
            let elapsed = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as i64 - started;
            
            elapsed > self.config.timeout.as_millis() as i64
        } else {
            false
        }
    }

    /// 获取任务进度（0.0 - 1.0）
    pub fn progress(&self) -> f64 {
        if self.status == TaskStatus::Completed {
            return 1.0;
        }
        
        if self.samples_processed == 0 {
            return 0.0;
        }

        // 这里是一个简化的进度计算，实际应该基于总系列数
        // 由于我们不知道总系列数，这里返回一个估计值
        match self.status {
            TaskStatus::Running => 0.5,
            TaskStatus::Pending => 0.0,
            TaskStatus::Failed | TaskStatus::Cancelled => 0.0,
            TaskStatus::Completed => 1.0,
        }
    }
}

/// 任务批次
#[derive(Debug, Clone)]
pub struct TaskBatch {
    /// 批次ID
    pub batch_id: usize,
    /// 系列ID列表
    pub series_ids: Vec<TimeSeriesId>,
    /// 批次状态
    pub status: TaskStatus,
    /// 处理的样本数
    pub samples_processed: u64,
    /// 生成的样本数
    pub samples_generated: u64,
}

impl TaskBatch {
    pub fn new(batch_id: usize, series_ids: Vec<TimeSeriesId>) -> Self {
        Self {
            batch_id,
            series_ids,
            status: TaskStatus::Pending,
            samples_processed: 0,
            samples_generated: 0,
        }
    }
}

/// 任务结果
#[derive(Debug, Clone)]
pub struct TaskResult {
    pub task_id: String,
    pub success: bool,
    pub samples_processed: u64,
    pub samples_generated: u64,
    pub error: Option<String>,
    pub duration_ms: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_lifecycle() {
        let mut task = DownsampleTask::new(
            DownsampleLevel::L1,
            DownsampleLevel::L0,
            0,
            1000,
            None,
        );

        assert_eq!(task.status, TaskStatus::Pending);
        assert!(task.task_id.starts_with("ds_"));

        task.mark_running();
        assert_eq!(task.status, TaskStatus::Running);
        assert!(task.started_at.is_some());

        task.mark_completed(1000, 100);
        assert_eq!(task.status, TaskStatus::Completed);
        assert_eq!(task.samples_processed, 1000);
        assert_eq!(task.samples_generated, 100);
        assert!(task.completed_at.is_some());
    }

    #[test]
    fn test_task_retry() {
        let mut task = DownsampleTask::new(
            DownsampleLevel::L1,
            DownsampleLevel::L0,
            0,
            1000,
            None,
        );

        task.mark_failed("Test error".to_string());
        assert_eq!(task.status, TaskStatus::Failed);
        assert_eq!(task.retry_count, 1);
        assert!(task.can_retry());

        task.reset_for_retry();
        assert_eq!(task.status, TaskStatus::Pending);

        // 模拟多次失败
        for _ in 0..3 {
            task.mark_failed("Test error".to_string());
        }
        
        assert!(!task.can_retry());
    }

    #[test]
    fn test_task_timeout() {
        let mut task = DownsampleTask::new(
            DownsampleLevel::L1,
            DownsampleLevel::L0,
            0,
            1000,
            None,
        );

        // 设置一个很短的超时时间
        task.config.timeout = Duration::from_millis(1);
        
        task.mark_running();
        
        // 等待一小段时间
        std::thread::sleep(Duration::from_millis(10));
        
        assert!(task.is_timeout());
    }
}
