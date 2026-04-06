use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId}; 
use chronodb_storage::config::StorageConfig;
use chronodb_storage::memstore::MemStore;
use chronodb_storage::model::{Label, Sample};
use chronodb_storage::distributed::{ClusterManager, ClusterConfig, NodeInfo, NodeStatus};
use chronodb_storage::distributed::{ReplicationManager, ReplicationConfig};
use chronodb_storage::distributed::{ShardManager, ShardConfig};
use chronodb_storage::rpc::ClusterRpcManager;
use std::sync::Arc;
use tempfile::tempdir;

fn create_test_store() -> Arc<MemStore> {
    let temp_dir = tempdir().unwrap();
    let config = StorageConfig {
        data_dir: temp_dir.path().to_string_lossy().to_string(),
        ..Default::default()
    };
    Arc::new(MemStore::new(config).unwrap())
}

fn bench_cluster_manager(c: &mut Criterion) {
    let mut group = c.benchmark_group("cluster_manager");

    group.bench_function("register_node", |b| {
        let config = ClusterConfig::default();
        let mut manager = ClusterManager::new(config);
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

        b.to_async(tokio::runtime::Runtime::new().unwrap())
            .iter(|| async {
                manager.register_node(node_info.clone()).await.unwrap();
            });
    });

    group.bench_function("get_nodes", |b| {
        let config = ClusterConfig::default();
        let mut manager = ClusterManager::new(config);
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

        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            manager.register_node(node_info).await.unwrap();
        });

        b.to_async(tokio::runtime::Runtime::new().unwrap())
            .iter(|| async {
                manager.get_nodes().await.unwrap();
            });
    });

    group.finish();
}

fn bench_replication_manager(c: &mut Criterion) {
    let mut group = c.benchmark_group("replication_manager");

    group.bench_function("replicate", |b| {
        let config = ReplicationConfig::default();
        let mut manager = ReplicationManager::new(config);
        let rpc_manager = Arc::new(ClusterRpcManager::new());

        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            manager.start(rpc_manager).await.unwrap();
        });

        let series = {
            let mut series = chronodb_storage::model::TimeSeries::new(
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
            series
        };

        let target_nodes = vec!["node1".to_string(), "node2".to_string()];

        b.to_async(tokio::runtime::Runtime::new().unwrap())
            .iter(|| async {
                let _ = manager.replicate(1, series.clone(), &target_nodes).await;
            });
    });

    group.finish();
}

fn bench_shard_manager(c: &mut Criterion) {
    let mut group = c.benchmark_group("shard_manager");

    for shard_count in [64, 128, 256].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(shard_count), shard_count, |b, &shard_count| {
            let config = ShardConfig {
                shard_count,
                replication_factor: 2,
                enable_virtual_nodes: false,
                virtual_nodes_per_physical: 0,
            };
            let manager = ShardManager::new(config);

            b.iter(|| {
                for i in 0..1000 {
                    black_box(manager.get_shard_for_series(i));
                }
            });
        });
    }

    group.finish();
}

fn bench_concurrent_writes(c: &mut Criterion) {
    use std::thread;

    let mut group = c.benchmark_group("concurrent_writes");

    for threads in [2, 4, 8].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(threads), threads, |b, &threads| {
            let store = create_test_store();

            b.iter(|| {
                let mut handles = vec![];
                for i in 0..threads {
                    let store_clone = store.clone();
                    let handle = thread::spawn(move || {
                        let labels = vec![
                            Label::new("__name__", &format!("metric_{}", i)),
                            Label::new("job", "test"),
                        ];
                        let samples = vec![
                            Sample::new(1000, i as f64),
                            Sample::new(2000, (i * 2) as f64),
                            Sample::new(3000, (i * 3) as f64),
                        ];
                        store_clone.write(labels, samples).unwrap();
                    });
                    handles.push(handle);
                }
                for handle in handles {
                    handle.join().unwrap();
                }
            });
        });
    }

    group.finish();
}

fn bench_query_optimization(c: &mut Criterion) {
    let mut group = c.benchmark_group("query_optimization");

    group.bench_function("query_with_optimizer", |b| {
        let store = create_test_store();
        let engine = chronodb_storage::query::QueryEngine::new(store.clone());

        // 准备数据
        let labels = vec![
            Label::new("__name__", "http_requests_total"),
            Label::new("job", "prometheus"),
        ];
        let samples: Vec<Sample> = (0..1000)
            .map(|i| Sample::new(i as i64 * 1000, i as f64))
            .collect();
        store.write(labels, samples).unwrap();

        b.to_async(tokio::runtime::Runtime::new().unwrap())
            .iter(|| async {
                engine.query_instant("http_requests_total", 500000).await.unwrap();
            });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_cluster_manager,
    bench_replication_manager,
    bench_shard_manager,
    bench_concurrent_writes,
    bench_query_optimization
);
criterion_main!(benches);
