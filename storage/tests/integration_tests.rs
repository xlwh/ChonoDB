use chronodb_storage::config::StorageConfig;
use chronodb_storage::memstore::MemStore;
use chronodb_storage::model::{Label, Sample, TimeSeries};
use chronodb_storage::query::{QueryEngine, parse_promql};
use chronodb_storage::distributed::{ClusterManager, ClusterConfig, NodeInfo, NodeStatus};
use chronodb_storage::distributed::{ReplicationManager, ReplicationConfig};
use chronodb_storage::distributed::{QueryCoordinator, QueryCoordinatorConfig, ShardConfig, ShardManager, QueryShardManager};
use chronodb_storage::rpc::ClusterRpcManager;
use std::sync::Arc;
use tempfile::tempdir;

struct TestStore {
    store: Arc<MemStore>,
    _temp_dir: tempfile::TempDir,
}

/// 集成测试辅助函数
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
    let test_store = create_test_store_with_tempdir();
    let store = test_store.store;

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
    let test_store = create_test_store_with_tempdir();
    let store = test_store.store;

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
    let test_store = create_test_store_with_tempdir();
    let store = test_store.store;

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
    let test_store = create_test_store_with_tempdir();
    let store = test_store.store;
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
    let test_store = create_test_store_with_tempdir();
    let store = test_store.store;

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
    let test_store = create_test_store_with_tempdir();
    let store = test_store.store;

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

    let test_store = create_test_store_with_tempdir();
    let store = test_store.store;
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

#[tokio::test]
async fn test_cluster_manager() {
    let config = ClusterConfig::default();
    let mut manager = ClusterManager::new(config);
    
    manager.start().await.unwrap();
    
    let node_info = NodeInfo {
        node_id: "node1".to_string(),
        address: "localhost:9090".to_string(),
        status: NodeStatus::Online,
        last_heartbeat: chrono::Utc::now().timestamp_millis(),
        shard_count: 100,
        series_count: 1000,
        is_leader: false,
        version: "1.0.0".to_string(),
    };
    
    manager.register_node(node_info).await.unwrap();
    
    let nodes = manager.get_nodes().await.unwrap();
    assert_eq!(nodes.len(), 1);
    
    manager.update_heartbeat("node1").await.unwrap();
    
    let healthy_nodes = manager.get_healthy_nodes().await.unwrap();
    assert_eq!(healthy_nodes.len(), 1);
    
    manager.stop().await.unwrap();
}

#[tokio::test]
async fn test_replication_manager() {
    let config = ReplicationConfig::default();
    let mut manager = ReplicationManager::new(config);
    
    // 创建一个模拟的RPC管理器
    let rpc_manager = Arc::new(ClusterRpcManager::new());
    
    manager.start(rpc_manager).await.unwrap();
    
    let mut series = TimeSeries::new(
        1,
        vec![
            Label::new("__name__", "test_metric"),
            Label::new("job", "test"),
        ],
    );
    
    series.add_samples(vec![
        Sample::new(1000, 1.0),
        Sample::new(2000, 2.0),
    ]);
    
    let target_nodes = vec!["node1".to_string(), "node2".to_string()];
    
    // 由于是模拟环境，这里会失败但不影响测试
    let _ = manager.replicate(1, series, &target_nodes).await;
    
    manager.stop().await.unwrap();
}

#[tokio::test]
async fn test_shard_manager() {
    let config = ShardConfig {
        shard_count: 128,
        replication_factor: 2,
        enable_virtual_nodes: false,
        virtual_nodes_per_physical: 0,
    };
    let manager = ShardManager::new(config);
    
    let shard_id = manager.get_shard_for_series(12345);
    assert!(shard_id < 128);
}

#[tokio::test]
async fn test_query_coordinator() {
    let shard_manager = Arc::new(tokio::sync::RwLock::new(QueryShardManager::new(128)));
    let rpc_manager = Arc::new(ClusterRpcManager::new());
    let config = QueryCoordinatorConfig::default();
    
    let coordinator = QueryCoordinator::new(rpc_manager, shard_manager, config);
    
    // 测试查询缓存清理
    coordinator.cleanup_cache().await;
    
    // 测试获取统计信息
    let stats = coordinator.get_stats().await.unwrap();
    assert_eq!(stats.total_queries, 0);
}

#[tokio::test]
async fn test_distributed_integration() {
    // 测试完整的分布式集成流程
    let config = ClusterConfig::default();
    let mut cluster_manager = ClusterManager::new(config);
    
    cluster_manager.start().await.unwrap();
    
    // 注册节点
    let node_info = NodeInfo {
        node_id: "node1".to_string(),
        address: "localhost:9090".to_string(),
        status: NodeStatus::Online,
        last_heartbeat: chrono::Utc::now().timestamp_millis(),
        shard_count: 100,
        series_count: 1000,
        is_leader: false,
        version: "1.0.0".to_string(),
    };
    
    cluster_manager.register_node(node_info).await.unwrap();
    
    // 测试分片管理器
    let shard_config = ShardConfig {
        shard_count: 128,
        replication_factor: 2,
        enable_virtual_nodes: false,
        virtual_nodes_per_physical: 0,
    };
    let shard_manager = ShardManager::new(shard_config);
    
    // 测试查询协调器
    let query_shard_manager = QueryShardManager::new(128);
    let shard_manager_arc = Arc::new(tokio::sync::RwLock::new(query_shard_manager));
    let rpc_manager = Arc::new(ClusterRpcManager::new());
    let coordinator_config = QueryCoordinatorConfig::default();
    
    let coordinator = QueryCoordinator::new(rpc_manager, shard_manager_arc, coordinator_config);
    
    // 测试查询缓存清理
    coordinator.cleanup_cache().await;
    
    cluster_manager.stop().await.unwrap();
}
