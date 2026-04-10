use crate::error::{Error, Result};
use crate::model::TimeSeries;
use crate::rpc::{ClusterRpcManager, ReplicateRequest};
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::{RwLock, Semaphore};
use tokio::time::interval;
use tracing::{info, debug, warn};

/// 副本配置
#[derive(Debug, Clone)]
pub struct ReplicationConfig {
    /// 副本因子
    pub replication_factor: u32,
    /// 最小写入副本数
    pub min_write_replicas: u32,
    /// 最小读取副本数
    pub min_read_replicas: u32,
    /// 异步复制
    pub async_replication: bool,
    /// 复制超时（毫秒）
    pub replication_timeout_ms: u64,
    /// 复制队列大小
    pub replication_queue_size: usize,
    /// 复制批处理大小
    pub batch_size: usize,
}

impl Default for ReplicationConfig {
    fn default() -> Self {
        Self {
            replication_factor: 3,
            min_write_replicas: 2,
            min_read_replicas: 1,
            async_replication: true,
            replication_timeout_ms: 5000,
            replication_queue_size: 10000,
            batch_size: 100,
        }
    }
}

impl ReplicationConfig {
    /// 从YAML配置创建副本配置
    pub fn from_yaml_config(yaml_config: &crate::config::ReplicationConfigYaml) -> Self {
        let replication_timeout_ms = parse_duration(&yaml_config.timeout).unwrap_or(5000);

        Self {
            replication_factor: yaml_config.factor as u32,
            min_write_replicas: (yaml_config.factor as u32).saturating_sub(1),
            min_read_replicas: yaml_config.min_read_replicas,
            async_replication: yaml_config.strategy == "asynchronous",
            replication_timeout_ms,
            replication_queue_size: yaml_config.queue_size,
            batch_size: yaml_config.batch_size,
        }
    }
}

/// 解析时间字符串为毫秒
fn parse_duration(duration: &str) -> Option<u64> {
    let duration = duration.trim();

    if duration.is_empty() {
        return None;
    }

    if let Ok(value) = duration.parse::<u64>() {
        return Some(value);
    }

    if let Some((value, unit)) = duration.rsplit_once(|c: char| !c.is_ascii_digit()) {
        if let Ok(value) = value.parse::<u64>() {
            match unit.trim() {
                "ms" => Some(value),
                "s" => Some(value * 1000),
                "m" => Some(value * 60 * 1000),
                "h" => Some(value * 60 * 60 * 1000),
                "d" => Some(value * 24 * 60 * 60 * 1000),
                _ => None,
            }
        } else {
            None
        }
    } else {
        None
    }
}

/// 副本放置策略
#[derive(Debug, Clone)]
pub struct ReplicaPlacement {
    pub shard_id: u64,
    pub primary_node: String,
    pub replica_nodes: Vec<String>,
}

/// 复制管理器
pub struct ReplicationManager {
    config: ReplicationConfig,
    replication_log: Arc<RwLock<VecDeque<ReplicationEntry>>>,
    replication_queue: Arc<RwLock<VecDeque<ReplicationTask>>>,
    rpc_manager: Arc<RwLock<Option<Arc<ClusterRpcManager>>>>,
    replication_workers: Arc<RwLock<Vec<tokio::task::JoinHandle<()>>>>,
    semaphore: Arc<Semaphore>,
    metrics: Arc<RwLock<ReplicationMetrics>>,
}

#[derive(Debug, Clone)]
pub struct ReplicationEntry {
    pub sequence: u64,
    pub shard_id: u64,
    pub series: TimeSeries,
    pub timestamp: i64,
    pub status: ReplicationStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplicationStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
}

#[derive(Debug, Clone)]
pub struct ReplicationTask {
    pub shard_id: u64,
    pub series: TimeSeries,
    pub target_nodes: Vec<String>,
    pub retry_count: u32,
    pub max_retries: u32,
}

#[derive(Debug, Clone, Default)]
pub struct ReplicationMetrics {
    pub total_replications: u64,
    pub successful_replications: u64,
    pub failed_replications: u64,
    pub replication_latency_ms: f64,
    pub queue_size: usize,
}

impl ReplicationManager {
    pub fn new(config: ReplicationConfig) -> Self {
        Self {
            config,
            replication_log: Arc::new(RwLock::new(VecDeque::new())),
            replication_queue: Arc::new(RwLock::new(VecDeque::new())),
            rpc_manager: Arc::new(RwLock::new(None)),
            replication_workers: Arc::new(RwLock::new(Vec::new())),
            semaphore: Arc::new(Semaphore::new(100)), // 限制并发复制数
            metrics: Arc::new(RwLock::new(ReplicationMetrics::default())),
        }
    }

    pub async fn start(&self, rpc_manager: Arc<ClusterRpcManager>) -> Result<()> {
        info!("Starting replication manager");
        let mut rpc_manager_write = self.rpc_manager.write().await;
        *rpc_manager_write = Some(rpc_manager);
        
        // 启动复制工作线程
        let mut replication_workers_write = self.replication_workers.write().await;
        for i in 0..5 { // 5个工作线程
            let worker = self.start_replication_worker(i).await;
            replication_workers_write.push(worker);
        }
        
        // 启动复制日志清理任务
        self.start_log_cleanup_task().await;
        
        Ok(())
    }

    /// 启动复制工作线程
    async fn start_replication_worker(&self, worker_id: usize) -> tokio::task::JoinHandle<()> {
        let replication_queue = self.replication_queue.clone();
        let replication_log = self.replication_log.clone();
        let rpc_manager = self.rpc_manager.clone();
        let config = self.config.clone();
        let metrics = self.metrics.clone();
        let semaphore = self.semaphore.clone();
        
        tokio::spawn(async move {
            info!("Started replication worker {}", worker_id);
            
            loop {
                // 获取复制任务
                let task = {
                    let mut queue = replication_queue.write().await;
                    queue.pop_front()
                };
                
                if let Some(task) = task {
                    let _permit = semaphore.acquire().await;
                    
                    let start_time = SystemTime::now();
                    
                    // 执行复制
                    let result = async {
                        let mut success_count = 0;
                        
                        // 获取RPC管理器
                        let rpc_manager_read = rpc_manager.read().await;
                        let rpc_manager_ref = rpc_manager_read.as_ref().unwrap();
                        
                        for node in &task.target_nodes {
                            // 获取RPC客户端
                            let client = match rpc_manager_ref.get_client(node).await {
                                Some(client) => client,
                                None => {
                                    warn!("No RPC client for node {} in replication worker {}", node, worker_id);
                                    continue;
                                }
                            };
                            
                            // 构建复制请求
                            let _request = ReplicateRequest {
                                shard_id: task.shard_id,
                                series: task.series.clone(),
                            };
                            
                            // 发送复制请求
                            let response = tokio::time::timeout(
                                Duration::from_millis(config.replication_timeout_ms),
                                client.replicate(task.shard_id, task.series.clone())
                            ).await
                            .map_err(|_| Error::Internal("Replication timeout".to_string()))?
                            .map_err(|e| Error::Internal(format!("Replication failed: {}", e)))?;
                            
                            if response.success {
                                success_count += 1;
                            }
                        }
                        
                        // 检查是否满足最小写入副本数
                        if success_count < config.min_write_replicas {
                            return Err(Error::Internal(
                                format!("Only {} replicas written, minimum required: {}", 
                                    success_count, config.min_write_replicas)
                            ));
                        }
                        
                        Ok(())
                    }.await;
                    
                    match result {
                        Ok(_) => {
                            let mut metrics_write = metrics.write().await;
                            metrics_write.successful_replications += 1;
                            let latency = start_time.elapsed().unwrap().as_millis() as f64;
                            metrics_write.replication_latency_ms = metrics_write.replication_latency_ms * 0.9 + latency * 0.1 ;

                            let mut log = replication_log.write().await;
                            for entry in log.iter_mut() {
                                if entry.shard_id == task.shard_id && entry.status == ReplicationStatus::Pending {
                                    entry.status = ReplicationStatus::Completed;
                                }
                            }
                        },
                        Err(e) => {
                            let mut metrics_write = metrics.write().await;
                            metrics_write.failed_replications += 1;
                            warn!("Replication failed: {:?}", e);

                            let mut log = replication_log.write().await;
                            for entry in log.iter_mut() {
                                if entry.shard_id == task.shard_id && entry.status == ReplicationStatus::Pending {
                                    entry.status = ReplicationStatus::Failed;
                                }
                            }
                            
                            // 重试
                            if task.retry_count < task.max_retries {
                                let mut queue = replication_queue.write().await;
                                queue.push_back(ReplicationTask {
                                    retry_count: task.retry_count + 1,
                                    ..task
                                });
                            }
                        }
                    }
                } else {
                    // 队列为空，等待
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
            }
        })
    }

    /// 启动日志清理任务
    async fn start_log_cleanup_task(&self) {
        let replication_log = self.replication_log.clone();
        
        tokio::spawn(async move {
            let mut interval = interval(Duration::from_mins(5));
            
            loop {
                interval.tick().await;
                
                let mut log = replication_log.write().await;
                // 只保留最近10000条记录
                while log.len() > 10000 {
                    log.pop_front();
                }
            }
        });
    }



    /// 复制数据到副本节点
    pub async fn replicate(&self, shard_id: u64, series: TimeSeries, target_nodes: &[String]) -> Result<()> {
        if target_nodes.is_empty() {
            return Ok(());
        }

        debug!("Replicating series {} to {} nodes", series.id, target_nodes.len());
        
        // 更新指标
        let mut metrics = self.metrics.write().await;
        metrics.total_replications += 1;
        metrics.queue_size = self.replication_queue.read().await.len();
        
        if self.config.async_replication {
            // 异步复制：加入队列
            let task = ReplicationTask {
                shard_id,
                series: series.clone(),
                target_nodes: target_nodes.to_vec(),
                retry_count: 0,
                max_retries: 3,
            };
            
            let mut queue = self.replication_queue.write().await;
            if queue.len() < self.config.replication_queue_size {
                queue.push_back(task);
            } else {
                return Err(Error::Internal("Replication queue full".to_string()));
            }
        } else {
            // 同步复制：立即执行
            if let Some(rpc_manager) = &*self.rpc_manager.read().await {
                let mut success_count = 0;
                
                for node in target_nodes {
                    // 获取RPC客户端
                    let client = match rpc_manager.get_client(node).await {
                        Some(client) => client,
                        None => {
                            warn!("No RPC client for node {} in sync replication", node);
                            continue;
                        }
                    };
                    
                    // 发送复制请求
                    let response = tokio::time::timeout(
                        Duration::from_millis(self.config.replication_timeout_ms),
                        client.replicate(shard_id, series.clone())
                    ).await
                    .map_err(|_| Error::Internal("Replication timeout".to_string()))?
                    .map_err(|e| Error::Internal(format!("Replication failed: {}", e)))?;
                    
                    if response.success {
                        success_count += 1;
                    }
                }
                
                // 检查是否满足最小写入副本数
                if success_count < self.config.min_write_replicas {
                    return Err(Error::Internal(
                        format!("Only {} replicas written, minimum required: {}", 
                            success_count, self.config.min_write_replicas)
                    ));
                }
            }
        }

        // 记录复制日志
        self.log_replication(shard_id, series).await?;

        Ok(())
    }

    /// 记录复制日志
    pub async fn log_replication(&self, shard_id: u64, series: TimeSeries) -> Result<()> {
        let mut log = self.replication_log.write().await;
        let sequence = log.len() as u64;
        
        log.push_back(ReplicationEntry {
            sequence,
            shard_id,
            series,
            timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as i64,
            status: ReplicationStatus::Pending,
        });
        
        Ok(())
    }

    /// 获取复制状态
    pub async fn get_replication_status(&self, shard_id: u64) -> Result<Vec<ReplicationEntry>> {
        let log = self.replication_log.read().await;
        Ok(log
            .iter()
            .filter(|entry| entry.shard_id == shard_id)
            .cloned()
            .collect())
    }

    /// 获取复制指标
    pub async fn get_metrics(&self) -> Result<ReplicationMetrics> {
        let metrics = self.metrics.read().await;
        Ok(metrics.clone())
    }

    pub async fn handle_node_failure(&self, failed_node_id: &str, _healthy_nodes: &[String]) -> Result<()> {
        info!("Replication manager handling failure of node: {}", failed_node_id);

        let mut log = self.replication_log.write().await;
        for entry in log.iter_mut() {
            if entry.status == ReplicationStatus::Pending {
                entry.status = ReplicationStatus::Failed;
            }
        }
        drop(log);

        {
            let rpc_manager = self.rpc_manager.read().await;
            if let Some(ref rpc_mgr) = *rpc_manager {
                rpc_mgr.unregister_node(failed_node_id).await;
                info!("Unregistered RPC client for failed node: {}", failed_node_id);
            }
        }

        let mut metrics = self.metrics.write().await;
        metrics.failed_replications += 1;

        info!("Replication manager completed failure handling for node: {}", failed_node_id);
        Ok(())
    }

    pub async fn stop(&self) -> Result<()> {
        let mut workers = self.replication_workers.write().await;
        for worker in workers.drain(..) {
            worker.abort();
        }
        Ok(())
    }
}
