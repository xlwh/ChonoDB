use chronodb_storage::config::StorageConfig;
use chronodb_storage::memstore::MemStore;
use chronodb_storage::model::{Label, Sample};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tracing::{info, debug};

/// 压力测试配置
#[derive(Debug, Clone)]
pub struct StressTestConfig {
    /// 数据目录
    pub data_dir: String,
    
    /// 测试持续时间（秒）
    pub duration: u64,
    
    /// 并发写入线程数
    pub write_threads: usize,
    
    /// 并发查询线程数
    pub query_threads: usize,
    
    /// 每个线程的写入频率（操作/秒）
    pub write_rate: u64,
    
    /// 每个线程的查询频率（操作/秒）
    pub query_rate: u64,
    
    /// 每个时间序列的标签数量
    pub labels_per_series: usize,
    
    /// 每个写入操作的样本数量
    pub samples_per_write: usize,
    
    /// 测试期间生成的时间序列数量
    pub series_count: usize,
}

/// 压力测试结果
#[derive(Debug)]
pub struct StressTestResult {
    pub duration: Duration,
    pub total_writes: u64,
    pub total_queries: u64,
    pub successful_writes: u64,
    pub successful_queries: u64,
    pub failed_writes: u64,
    pub failed_queries: u64,
    pub avg_write_latency: f64,
    pub avg_query_latency: f64,
    pub max_write_latency: f64,
    pub max_query_latency: f64,
    pub total_series: usize,
    pub total_samples: u64,
}

/// 压力测试工具
pub struct StressTest {
    config: StressTestConfig,
    store: Arc<MemStore>,
}

impl StressTest {
    pub fn new(config: StressTestConfig) -> Self {
        let storage_config = StorageConfig {
            data_dir: config.data_dir.clone(),
            ..Default::default()
        };
        let store = Arc::new(MemStore::new(storage_config).unwrap());
        
        Self {
            config,
            store,
        }
    }

    /// 运行压力测试
    pub fn run(&self) -> StressTestResult {
        info!("Starting stress test with config: {:?}", self.config);
        
        let start_time = Instant::now();
        let duration = Duration::from_secs(self.config.duration);
        
        // 初始化统计信息
        let total_writes = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
        let total_queries = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
        let successful_writes = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
        let successful_queries = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
        let failed_writes = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
        let failed_queries = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
        
        let total_write_latency = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
        let total_query_latency = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
        let max_write_latency = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
        let max_query_latency = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
        
        // 启动写入线程
        let mut write_handles = vec![];
        for thread_id in 0..self.config.write_threads {
            let store = self.store.clone();
            let total_writes = total_writes.clone();
            let successful_writes = successful_writes.clone();
            let failed_writes = failed_writes.clone();
            let total_write_latency = total_write_latency.clone();
            let max_write_latency = max_write_latency.clone();
            let config = self.config.clone();
            
            let handle = thread::spawn(move || {
                let interval = Duration::from_nanos(1_000_000_000 / config.write_rate);
                let start = Instant::now();
                
                while Instant::now().duration_since(start) < duration {
                    let op_start = Instant::now();
                    
                    // 生成随机时间序列
                    let series_id = (thread_id * config.series_count / config.write_threads) + (total_writes.load(std::sync::atomic::Ordering::Relaxed) as usize % (config.series_count / config.write_threads));
                    let labels = Self::generate_labels(series_id, config.labels_per_series);
                    let samples = Self::generate_samples(config.samples_per_write);
                    
                    // 执行写入
                    match store.write(labels, samples) {
                        Ok(_) => {
                            successful_writes.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        }
                        Err(e) => {
                            failed_writes.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                            debug!("Write failed: {:?}", e);
                        }
                    }
                    
                    let op_duration = op_start.elapsed();
                    total_write_latency.fetch_add(op_duration.as_nanos() as u64, std::sync::atomic::Ordering::Relaxed);
                    
                    // 更新最大写入延迟
                    let current_max = max_write_latency.load(std::sync::atomic::Ordering::Relaxed);
                    let op_nanos = op_duration.as_nanos() as u64;
                    if op_nanos > current_max {
                        max_write_latency.store(op_nanos, std::sync::atomic::Ordering::Relaxed);
                    }
                    
                    total_writes.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    
                    // 控制写入速率
                    thread::sleep(interval);
                }
            });
            
            write_handles.push(handle);
        }
        
        // 启动查询线程
        let mut query_handles = vec![];
        for _ in 0..self.config.query_threads {
            let store = self.store.clone();
            let total_queries = total_queries.clone();
            let successful_queries = successful_queries.clone();
            let failed_queries = failed_queries.clone();
            let total_query_latency = total_query_latency.clone();
            let max_query_latency = max_query_latency.clone();
            let config = self.config.clone();
            
            let handle = thread::spawn(move || {
                let interval = Duration::from_nanos(1_000_000_000 / config.query_rate);
                let start = Instant::now();
                
                while Instant::now().duration_since(start) < duration {
                    let op_start = Instant::now();
                    
                    // 执行查询
                    let job_label = format!("job_{}", (total_queries.load(std::sync::atomic::Ordering::Relaxed) % 10) as u32);
                    match store.query(
                        &[("job".to_string(), job_label)],
                        0,
                        SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as i64,
                    ) {
                        Ok(_) => {
                            successful_queries.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        }
                        Err(e) => {
                            failed_queries.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                            debug!("Query failed: {:?}", e);
                        }
                    }
                    
                    let op_duration = op_start.elapsed();
                    total_query_latency.fetch_add(op_duration.as_nanos() as u64, std::sync::atomic::Ordering::Relaxed);
                    
                    // 更新最大查询延迟
                    let current_max = max_query_latency.load(std::sync::atomic::Ordering::Relaxed);
                    let op_nanos = op_duration.as_nanos() as u64;
                    if op_nanos > current_max {
                        max_query_latency.store(op_nanos, std::sync::atomic::Ordering::Relaxed);
                    }
                    
                    total_queries.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    
                    // 控制查询速率
                    thread::sleep(interval);
                }
            });
            
            query_handles.push(handle);
        }
        
        // 等待所有线程完成
        for handle in write_handles {
            handle.join().unwrap();
        }
        
        for handle in query_handles {
            handle.join().unwrap();
        }
        
        // 计算结果
        let actual_duration = start_time.elapsed();
        let total_writes = total_writes.load(std::sync::atomic::Ordering::Relaxed);
        let total_queries = total_queries.load(std::sync::atomic::Ordering::Relaxed);
        let successful_writes = successful_writes.load(std::sync::atomic::Ordering::Relaxed);
        let successful_queries = successful_queries.load(std::sync::atomic::Ordering::Relaxed);
        let failed_writes = failed_writes.load(std::sync::atomic::Ordering::Relaxed);
        let failed_queries = failed_queries.load(std::sync::atomic::Ordering::Relaxed);
        
        let avg_write_latency = if successful_writes > 0 {
            (total_write_latency.load(std::sync::atomic::Ordering::Relaxed) as f64 / successful_writes as f64) / 1_000_000.0
        } else {
            0.0
        };
        
        let avg_query_latency = if successful_queries > 0 {
            (total_query_latency.load(std::sync::atomic::Ordering::Relaxed) as f64 / successful_queries as f64) / 1_000_000.0
        } else {
            0.0
        };
        
        let max_write_latency = max_write_latency.load(std::sync::atomic::Ordering::Relaxed) as f64 / 1_000_000.0;
        let max_query_latency = max_query_latency.load(std::sync::atomic::Ordering::Relaxed) as f64 / 1_000_000.0;
        
        let stats = self.store.stats();
        
        let result = StressTestResult {
            duration: actual_duration,
            total_writes,
            total_queries,
            successful_writes,
            successful_queries,
            failed_writes,
            failed_queries,
            avg_write_latency,
            avg_query_latency,
            max_write_latency,
            max_query_latency,
            total_series: stats.total_series as usize,
            total_samples: stats.total_samples,
        };
        
        info!("Stress test completed:");
        info!("  Duration: {:?}", result.duration);
        info!("  Total writes: {}", result.total_writes);
        info!("  Total queries: {}", result.total_queries);
        info!("  Successful writes: {}", result.successful_writes);
        info!("  Successful queries: {}", result.successful_queries);
        info!("  Failed writes: {}", result.failed_writes);
        info!("  Failed queries: {}", result.failed_queries);
        info!("  Average write latency: {:.2} ms", result.avg_write_latency);
        info!("  Average query latency: {:.2} ms", result.avg_query_latency);
        info!("  Max write latency: {:.2} ms", result.max_write_latency);
        info!("  Max query latency: {:.2} ms", result.max_query_latency);
        info!("  Total series: {}", result.total_series);
        info!("  Total samples: {}", result.total_samples);
        
        result
    }

    /// 生成标签
    fn generate_labels(series_id: usize, labels_count: usize) -> Vec<Label> {
        let mut labels = vec![
            Label::new("__name__", format!("metric_{}", series_id % 100)),
            Label::new("job", format!("job_{}", series_id % 10)),
        ];
        
        for i in 2..labels_count {
            labels.push(Label::new(
                format!("label_{}", i),
                format!("value_{}", series_id % 1000),
            ));
        }
        
        labels
    }

    /// 生成样本
    fn generate_samples(count: usize) -> Vec<Sample> {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as i64;
        
        (0..count)
            .map(|i| {
                Sample::new(
                    now - ((count - i - 1) as i64) * 1000,
                    (i as f64) % 100.0,
                )
            })
            .collect()
    }
}
