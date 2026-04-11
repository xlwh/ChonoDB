use serde::{Deserialize, Serialize};
use thiserror::Error;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::Instant;

#[derive(Debug, Error)]
pub enum BatchExportError {
    #[error("Export error: {0}")]
    ExportError(#[from] crate::export::ExportError),
    
    #[error("Task error: {0}")]
    TaskError(String),
    
    #[error("Timeout")]
    Timeout,
    
    #[error("Cancelled")]
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchExportTask {
    pub task_id: String,
    pub query: String,
    pub start_time: i64,
    pub end_time: i64,
    pub format: crate::export::ExportFormat,
    pub output_path: String,
    pub created_at: i64,
    pub status: ExportStatus,
    pub progress: f64,
    pub error: Option<String>,
    pub completed_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ExportStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchExportRequest {
    pub query: String,
    pub start_time: i64,
    pub end_time: i64,
    pub format: crate::export::ExportFormat,
    pub output_path: String,
    pub timeout: Option<u64>, // 超时时间（秒）
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchExportResponse {
    pub task_id: String,
    pub status: ExportStatus,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchExportStatusResponse {
    pub task_id: String,
    pub status: ExportStatus,
    pub progress: f64,
    pub error: Option<String>,
    pub completed_at: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct BatchExportManager {
    tasks: Arc<tokio::sync::RwLock<std::collections::HashMap<String, BatchExportTask>>>,
    sender: mpsc::Sender<BatchExportTask>,
    worker_count: usize,
}

impl BatchExportManager {
    pub fn new(worker_count: usize) -> Self {
        let (sender, receiver) = mpsc::channel(100);
        
        let tasks = Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new()));
        
        // 启动工作线程
        let tasks_clone = Arc::clone(&tasks);
        tokio::spawn(async move {
            BatchExportManager::worker_loop(receiver, tasks_clone).await;
        });
        
        Self {
            tasks,
            sender,
            worker_count,
        }
    }

    pub async fn create_task(&self, request: BatchExportRequest) -> Result<BatchExportResponse, BatchExportError> {
        let task_id = uuid::Uuid::new_v4().to_string();
        
        let task = BatchExportTask {
            task_id: task_id.clone(),
            query: request.query,
            start_time: request.start_time,
            end_time: request.end_time,
            format: request.format,
            output_path: request.output_path,
            created_at: chrono::Utc::now().timestamp(),
            status: ExportStatus::Pending,
            progress: 0.0,
            error: None,
            completed_at: None,
        };
        
        // 添加任务到任务列表
        let mut tasks = self.tasks.write().await;
        tasks.insert(task_id.clone(), task.clone());
        drop(tasks);
        
        // 发送任务到工作线程
        self.sender.send(task).await
            .map_err(|e| BatchExportError::TaskError(e.to_string()))?;
        
        Ok(BatchExportResponse {
            task_id,
            status: ExportStatus::Pending,
            message: "Task created successfully".to_string(),
        })
    }

    pub async fn get_status(&self, task_id: &str) -> Option<BatchExportStatusResponse> {
        let tasks = self.tasks.read().await;
        tasks.get(task_id).map(|task| BatchExportStatusResponse {
            task_id: task.task_id.clone(),
            status: task.status.clone(),
            progress: task.progress,
            error: task.error.clone(),
            completed_at: task.completed_at,
        })
    }

    pub async fn list_tasks(&self) -> Vec<BatchExportTask> {
        let tasks = self.tasks.read().await;
        tasks.values().cloned().collect()
    }

    pub async fn cancel_task(&self, task_id: &str) -> Result<BatchExportResponse, BatchExportError> {
        let mut tasks = self.tasks.write().await;
        
        if let Some(task) = tasks.get_mut(task_id) {
            if task.status == ExportStatus::Pending || task.status == ExportStatus::Running {
                task.status = ExportStatus::Cancelled;
                Ok(BatchExportResponse {
                    task_id: task_id.to_string(),
                    status: ExportStatus::Cancelled,
                    message: "Task cancelled".to_string(),
                })
            } else {
                Err(BatchExportError::TaskError("Task cannot be cancelled".to_string()))
            }
        } else {
            Err(BatchExportError::TaskError("Task not found".to_string()))
        }
    }

    async fn worker_loop(
        mut receiver: mpsc::Receiver<BatchExportTask>,
        tasks: Arc<tokio::sync::RwLock<std::collections::HashMap<String, BatchExportTask>>>,
    ) {
        while let Some(mut task) = receiver.recv().await {
            // 更新任务状态为运行中
            {
                let mut tasks_write = tasks.write().await;
                if let Some(t) = tasks_write.get_mut(&task.task_id) {
                    t.status = ExportStatus::Running;
                    t.progress = 0.1;
                }
            }
            
            // 执行导出任务
            let result = Self::execute_task(&mut task).await;
            
            // 更新任务状态
            {
                let mut tasks_write = tasks.write().await;
                if let Some(t) = tasks_write.get_mut(&task.task_id) {
                    match result {
                        Ok(_) => {
                            t.status = ExportStatus::Completed;
                            t.progress = 1.0;
                            t.completed_at = Some(chrono::Utc::now().timestamp());
                        }
                        Err(e) => {
                            t.status = ExportStatus::Failed;
                            t.error = Some(e.to_string());
                        }
                    }
                }
            }
        }
    }

    async fn execute_task(task: &mut BatchExportTask) -> Result<(), BatchExportError> {
        // 模拟导出过程
        // 在实际实现中，这里应该：
        // 1. 执行查询获取数据
        // 2. 生成导出数据
        // 3. 写入到指定路径
        
        // 模拟进度更新
        for i in 1..=9 {
            tokio::time::sleep(Duration::from_millis(200)).await;
            task.progress = 0.1 + (i as f64 * 0.09);
        }
        
        // 模拟成功
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_task() {
        let manager = BatchExportManager::new(2);
        
        let request = BatchExportRequest {
            query: "cpu_usage".to_string(),
            start_time: 1609459200,
            end_time: 1609545600,
            format: crate::export::ExportFormat::Json,
            output_path: "/tmp/export.json".to_string(),
            timeout: Some(60),
        };
        
        let response = manager.create_task(request).await;
        assert!(response.is_ok());
        
        let task_id = response.unwrap().task_id;
        assert!(!task_id.is_empty());
        
        // 检查任务状态
        tokio::time::sleep(Duration::from_millis(100)).await;
        let status = manager.get_status(&task_id).await;
        assert!(status.is_some());
        assert_eq!(status.unwrap().status, ExportStatus::Running);
    }

    #[tokio::test]
    async fn test_cancel_task() {
        let manager = BatchExportManager::new(2);
        
        let request = BatchExportRequest {
            query: "cpu_usage".to_string(),
            start_time: 1609459200,
            end_time: 1609545600,
            format: crate::export::ExportFormat::Csv,
            output_path: "/tmp/export.csv".to_string(),
            timeout: Some(60),
        };
        
        let response = manager.create_task(request).await;
        assert!(response.is_ok());
        
        let task_id = response.unwrap().task_id;
        
        // 取消任务
        let cancel_response = manager.cancel_task(&task_id).await;
        assert!(cancel_response.is_ok());
        
        // 检查任务状态
        tokio::time::sleep(Duration::from_millis(100)).await;
        let status = manager.get_status(&task_id).await;
        assert!(status.is_some());
        assert_eq!(status.unwrap().status, ExportStatus::Cancelled);
    }

    #[tokio::test]
    async fn test_list_tasks() {
        let manager = BatchExportManager::new(2);
        
        // 创建两个任务
        for i in 0..2 {
            let request = BatchExportRequest {
                query: format!("cpu_usage{}", i),
                start_time: 1609459200,
                end_time: 1609545600,
                format: crate::export::ExportFormat::Json,
                output_path: format!("/tmp/export{}.json", i),
                timeout: Some(60),
            };
            
            manager.create_task(request).await.unwrap();
        }
        
        // 检查任务列表
        tokio::time::sleep(Duration::from_millis(100)).await;
        let tasks = manager.list_tasks().await;
        assert_eq!(tasks.len(), 2);
    }
}