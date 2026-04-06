use chronodb_storage::config::StorageConfig;
use chronodb_storage::memstore::MemStore;
use chronodb_storage::model::{Label, Sample, TimeSeries};
use chronodb_storage::query::{QueryEngine, parse_promql};
use std::sync::Arc;
use tempfile::tempdir;

/// 集成测试辅助函数
fn create_test_store() -> Arc<MemStore> {
    let temp_dir = tempdir().unwrap();
    let config = StorageConfig {
        data_dir: temp_dir.path().to_string_lossy().to_string(),
        ..Default::default()
    };
    Arc::new(MemStore::new(config).unwrap())
}

fn create_test_series(id: u64, name: &str, job: &str, instance: &str) -> TimeSeries {
    let labels = vec![
        Label::new("__name__", name),
        Label::new("job", job),
        Label::new("instance", instance),
    ];

    let samples: Vec<Sample> = (0..100)
        .map(|i| Sample::new(i as i64 * 1000, i as f64 * 10.0))
        .collect();

    let mut ts = TimeSeries::new(id, labels);
    ts.add_samples(samples);
    ts
}

#[test]
fn test_write_and_read() {
    let store = create_test_store();

    // 写入数据
    let labels = vec![
        Label::new("__name__", "test_metric"),
        Label::new("job", "test"),
    ];
    let samples = vec![
        Sample::new(1000, 10.0),
        Sample::new(2000, 20.0),
        Sample::new(3000, 30.0),
    ];

    store.write(labels, samples).unwrap();

    // 查询数据
    let results = store
        .query(&[("job".to_string(), "test".to_string())], 0, 10000)
        .unwrap();

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].samples.len(), 3);
}

#[test]
fn test_query_by_label() {
    let store = create_test_store();

    // 写入多个系列
    let series1 = create_test_series(1, "metric1", "job1", "instance1");
    let series2 = create_test_series(2, "metric2", "job1", "instance2");
    let series3 = create_test_series(3, "metric3", "job2", "instance1");

    store.write(series1.labels.clone(), series1.samples.clone()).unwrap();
    store.write(series2.labels.clone(), series2.samples.clone()).unwrap();
    store.write(series3.labels.clone(), series3.samples.clone()).unwrap();

    // 按job标签查询
    let results = store
        .query(&[("job".to_string(), "job1".to_string())], 0, 100000)
        .unwrap();

    assert_eq!(results.len(), 2);
}

#[test]
fn test_query_time_range() {
    let store = create_test_store();

    let labels = vec![
        Label::new("__name__", "test_metric"),
        Label::new("job", "test"),
    ];
    let samples = vec![
        Sample::new(1000, 10.0),
        Sample::new(2000, 20.0),
        Sample::new(3000, 30.0),
        Sample::new(4000, 40.0),
        Sample::new(5000, 50.0),
    ];

    store.write(labels, samples).unwrap();

    // 查询部分时间范围
    let results = store
        .query(&[("job".to_string(), "test".to_string())], 2000, 4000)
        .unwrap();

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].samples.len(), 3); // 2000, 3000, 4000
}

#[tokio::test]
async fn test_query_engine() {
    let store = create_test_store();
    let engine = QueryEngine::new(store.clone());

    // 写入测试数据
    let labels = vec![
        Label::new("__name__", "http_requests_total"),
        Label::new("job", "prometheus"),
    ];
    let samples = vec![
        Sample::new(1000, 100.0),
        Sample::new(2000, 150.0),
        Sample::new(3000, 200.0),
    ];
    store.write(labels, samples).unwrap();

    // 执行查询
    let result = engine.query_instant("http_requests_total", 3000).await;
    assert!(result.is_ok());

    let query_result = result.unwrap();
    assert_eq!(query_result.series.len(), 1);
}

#[test]
fn test_downsample_query() {
    let store = create_test_store();

    // 写入大量数据
    let labels = vec![
        Label::new("__name__", "test_metric"),
        Label::new("job", "test"),
    ];

    let samples: Vec<Sample> = (0..1000)
        .map(|i| Sample::new(i as i64 * 1000, i as f64))
        .collect();

    store.write(labels, samples).unwrap();

    // 查询大范围（应该使用降采样）
    let results = store
        .query_with_downsample(
            &[("job".to_string(), "test".to_string())],
            0,
            1000000,
            chronodb_storage::columnstore::DownsampleLevel::L1,
        )
        .unwrap();

    assert_eq!(results.len(), 1);
    // 降采样后样本数应该减少
    assert!(results[0].samples.len() < 1000);
}

#[test]
fn test_prometheus_compatibility() {
    let store = create_test_store();

    // 写入Prometheus格式的数据
    let labels = vec![
        Label::new("__name__", "prometheus_http_requests_total"),
        Label::new("handler", "/api/v1/query"),
        Label::new("job", "prometheus"),
        Label::new("instance", "localhost:9090"),
    ];

    let samples = vec![
        Sample::new(1609459200000, 1000.0),
        Sample::new(1609459201000, 1001.0),
        Sample::new(1609459202000, 1002.0),
    ];

    store.write(labels, samples).unwrap();

    // 查询
    let results = store
        .query(
            &[
                ("__name__".to_string(), "prometheus_http_requests_total".to_string()),
                ("job".to_string(), "prometheus".to_string()),
            ],
            1609459200000,
            1609459202000,
        )
        .unwrap();

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].labels.len(), 4);
}

#[test]
fn test_concurrent_writes() {
    use std::thread;

    let store = create_test_store();
    let mut handles = vec![];

    for i in 0..10 {
        let store_clone = store.clone();
        let handle = thread::spawn(move || {
            let labels = vec![
                Label::new("__name__", &format!("metric_{}", i)),
                Label::new("job", "test"),
            ];
            let samples = vec![Sample::new(1000, i as f64)];
            store_clone.write(labels, samples).unwrap();
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    // 验证所有数据都被写入
    let stats = store.stats();
    assert_eq!(stats.total_series, 10);
}
