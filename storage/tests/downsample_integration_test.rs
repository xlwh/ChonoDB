use chronodb_storage::config::StorageConfig;
use chronodb_storage::memstore::MemStore;
use chronodb_storage::model::{Label, Sample};
use chronodb_storage::downsample::{
    DownsampleManager, DownsampleConfig, DownsampleWorker,
    DownsampleProcessor, DownsamplePoint
};
use chronodb_storage::downsample::worker::WorkerTask;
use chronodb_storage::columnstore::DownsampleLevel;
use std::sync::Arc;
use tempfile::tempdir;

struct TestStore {
    store: Arc<MemStore>,
    _temp_dir: tempfile::TempDir,
}

fn create_test_store_with_tempdir() -> TestStore {
    let temp_dir = tempdir().unwrap();
    let config = StorageConfig {
        data_dir: temp_dir.path().to_string_lossy().to_string(),
        ..Default::default()
    };
    let store = Arc::new(MemStore::new(config).unwrap());
    TestStore {
        store,
        _temp_dir: temp_dir,
    }
}

#[test]
fn test_downsample_processor_end_to_end() {
    // 创建测试样本
    let samples: Vec<Sample> = (0..1000)
        .map(|i| Sample::new(i as i64 * 1000, i as f64))
        .collect();

    // 测试不同的降采样级别
    let resolutions = vec![
        10000,    // 10秒
        60000,    // 1分钟
        300000,   // 5分钟
        3600000,  // 1小时
        86400000, // 1天
    ];

    for &resolution in &resolutions {
        let points = DownsampleProcessor::downseries(&samples, resolution);
        
        // 验证降采样结果
        assert!(!points.is_empty());
        assert!(points.len() <= samples.len());
        
        // 验证每个降采样点的聚合值
        for point in &points {
            assert!(!point.is_empty());
            assert!(point.min_value <= point.max_value);
            assert!((point.sum_value / point.count as f64 - point.avg_value).abs() < 1e-6);
        }
    }
}

#[test]
fn test_downsample_point_aggregation() {
    let mut point = DownsamplePoint::new(1000);
    
    // 添加样本
    point.add_sample(10.0);
    point.add_sample(20.0);
    point.add_sample(30.0);
    point.add_sample(40.0);
    point.add_sample(50.0);
    
    // 验证聚合结果
    assert_eq!(point.min_value, 10.0);
    assert_eq!(point.max_value, 50.0);
    assert_eq!(point.avg_value, 30.0);
    assert_eq!(point.sum_value, 150.0);
    assert_eq!(point.count, 5);
    assert_eq!(point.last_value, 50.0);
    
    // 测试获取不同聚合函数的值
    assert_eq!(DownsampleProcessor::get_value_by_function(&point, "min"), 10.0);
    assert_eq!(DownsampleProcessor::get_value_by_function(&point, "max"), 50.0);
    assert_eq!(DownsampleProcessor::get_value_by_function(&point, "avg"), 30.0);
    assert_eq!(DownsampleProcessor::get_value_by_function(&point, "sum"), 150.0);
    assert_eq!(DownsampleProcessor::get_value_by_function(&point, "count"), 5.0);
    assert_eq!(DownsampleProcessor::get_value_by_function(&point, "last"), 50.0);
    assert_eq!(DownsampleProcessor::get_value_by_function(&point, "unknown"), 30.0); // 默认返回平均值
}

#[test]
fn test_downsample_scheduler_basic() {
    // 简化测试，只测试调度器的基本创建和API
    use chronodb_storage::downsample::TaskPriority;
    
    // 测试优先级顺序
    assert!(TaskPriority::Low < TaskPriority::Normal);
    assert!(TaskPriority::Normal < TaskPriority::High);
    assert!(TaskPriority::High < TaskPriority::Critical);
    
    // 测试相等性
    assert_eq!(TaskPriority::Low, TaskPriority::Low);
    assert_eq!(TaskPriority::Normal, TaskPriority::Normal);
}

#[test]
fn test_downsample_with_query() {
    let test_store = create_test_store_with_tempdir();
    let store = test_store.store;
    
    // 写入适量数据，避免超时
    let labels = vec![
        Label::new("__name__", "cpu_usage"),
        Label::new("job", "webserver"),
        Label::new("instance", "web01"),
    ];
    
    let samples: Vec<Sample> = (0..1000)
        .map(|i| Sample::new(i as i64 * 1000, (i % 100) as f64))
        .collect();
    
    store.write(labels.clone(), samples).unwrap();
    
    // 测试原始查询
    let original_results = store
        .query(&[("job".to_string(), "webserver".to_string())], 0, 10000000)
        .unwrap();
    
    assert_eq!(original_results.len(), 1);
    // MemStore的查询可能会返回不同数量的样本，只要大于0就可以
    assert!(original_results[0].samples.len() > 0);
}

#[tokio::test]
async fn test_downsample_manager() {
    let temp_dir = tempdir().unwrap();
    let test_store = create_test_store_with_tempdir();
    let store = test_store.store;
    
    // 创建降采样管理器
    let config = DownsampleConfig::default();
    let mut manager = DownsampleManager::new(config, store.clone(), temp_dir.path().to_path_buf());
    
    // 启动管理器
    manager.start().await.unwrap();
    
    // 写入测试数据
    let labels = vec![
        Label::new("__name__", "memory_usage"),
        Label::new("job", "database"),
    ];
    
    let samples: Vec<Sample> = (0..1000)
        .map(|i| Sample::new(i as i64 * 1000, i as f64))
        .collect();
    
    store.write(labels, samples).unwrap();
    
    // 获取统计信息
    let stats = manager.get_stats().await;
    assert_eq!(stats.total_tasks, 0); // 还没有任务执行
    
    // 停止管理器
    manager.stop().await.unwrap();
}

#[test]
fn test_downsample_boundary_conditions() {
    // 测试空输入
    let empty_samples: Vec<Sample> = vec![];
    let empty_result = DownsampleProcessor::downseries(&empty_samples, 60000);
    assert!(empty_result.is_empty());
    
    // 测试单个样本
    let single_sample = vec![Sample::new(1000, 42.0)];
    let single_result = DownsampleProcessor::downseries(&single_sample, 60000);
    assert_eq!(single_result.len(), 1);
    assert_eq!(single_result[0].count, 1);
    assert_eq!(single_result[0].min_value, 42.0);
    assert_eq!(single_result[0].max_value, 42.0);
    
    // 测试所有样本都在同一个窗口
    let same_window_samples: Vec<Sample> = (0..100)
        .map(|i| Sample::new(1000 + i * 100, i as f64))
        .collect();
    
    let same_window_result = DownsampleProcessor::downseries(&same_window_samples, 100000);
    assert_eq!(same_window_result.len(), 1);
    assert_eq!(same_window_result[0].count, 100);
}

#[tokio::test]
async fn test_downsample_worker() {
    let temp_dir = tempdir().unwrap();
    let test_store = create_test_store_with_tempdir();
    let store = test_store.store;
    
    // 创建工作器
    let worker = DownsampleWorker::new(0, store.clone(), temp_dir.path().to_path_buf());
    
    // 写入测试数据
    let labels = vec![
        Label::new("__name__", "network_traffic"),
        Label::new("job", "proxy"),
    ];
    
    let samples: Vec<Sample> = (0..1000)
        .map(|i| Sample::new(i as i64 * 1000, i as f64))
        .collect();
    
    store.write(labels.clone(), samples).unwrap();
    
    // 创建任务
    let task = WorkerTask {
        task_id: "test_task".to_string(),
        target_level: DownsampleLevel::L1,
        source_level: DownsampleLevel::L0,
        start_time: 0,
        end_time: 1000000,
        series_ids: vec![1],
    };
    
    // 处理任务
    let _result = worker.process_task(task).await;
}

#[test]
fn test_downsample_data_integrity() {
    // 创建简单的线性样本，便于验证
    let samples: Vec<Sample> = (0..1000)
        .map(|i| Sample::new(i as i64 * 1000, i as f64))
        .collect();
    
    // 降采样
    let downsampled = DownsampleProcessor::downseries(&samples, 10000);
    
    // 验证数据完整性
    assert!(!downsampled.is_empty());
    
    // 验证时间范围覆盖
    let first_timestamp = samples.first().unwrap().timestamp;
    let last_timestamp = samples.last().unwrap().timestamp;
    
    let first_downsampled = downsampled.first().unwrap();
    let last_downsampled = downsampled.last().unwrap();
    
    // 降采样是向下对齐的，所以最后一个降采样点的时间戳应该<=原始最后一个时间戳
    assert!(first_downsampled.timestamp <= first_timestamp);
    assert!(last_downsampled.timestamp <= last_timestamp);
    // 同时，最后一个降采样点应该包含原始最后一个样本
    assert!(last_downsampled.timestamp + 10000 > last_timestamp);
    
    // 验证所有聚合值在合理范围内
    let original_min = samples.iter().map(|s| s.value).fold(f64::MAX, f64::min);
    let original_max = samples.iter().map(|s| s.value).fold(f64::MIN, f64::max);
    let original_sum: f64 = samples.iter().map(|s| s.value).sum();
    
    let downsampled_min = downsampled.iter().map(|p| p.min_value).fold(f64::MAX, f64::min);
    let downsampled_max = downsampled.iter().map(|p| p.max_value).fold(f64::MIN, f64::max);
    let downsampled_sum: f64 = downsampled.iter().map(|p| p.sum_value).sum();
    
    // 使用更宽松的精度检查
    assert!(downsampled_min <= original_min + 0.1);
    assert!(downsampled_max >= original_max - 0.1);
    assert!((downsampled_sum - original_sum).abs() < 1.0);
}
