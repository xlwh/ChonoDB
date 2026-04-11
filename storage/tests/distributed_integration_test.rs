use chronodb_storage::distributed::{DistributedStorage, DistributedConfig, ShardConfig, ReplicationConfig, ClusterConfig, ShardManager};
use chronodb_storage::model::{TimeSeries, Sample, Label, TimeSeriesId};
use std::time::Duration;
use tokio::time::sleep;

/// 测试分布式集群基本功能
#[tokio::test]
async fn test_distributed_cluster_basic() {
    // 创建3个节点的配置
    let mut configs = vec![];
    for i in 0..3 {
        let config = DistributedConfig {
            node_id: format!("node{}", i + 1),
            cluster_name: "test-cluster".to_string(),
            node_address: format!("127.0.0.1:{}", 9100 + i),
            coordinator_address: "127.0.0.1:9100".to_string(),
            is_coordinator: i == 0,
            shard_config: ShardConfig {
                shard_count: 16,
                replication_factor: 3,
                enable_virtual_nodes: true,
                virtual_nodes_per_physical: 128,
            },
            replication_config: ReplicationConfig {
                replication_factor: 3,
                min_write_replicas: 2,
                min_read_replicas: 1,
                async_replication: true,
                replication_timeout_ms: 5000,
                replication_queue_size: 10000,
                batch_size: 100,
            },
            cluster_config: ClusterConfig {
                cluster_name: "test-cluster".to_string(),
                heartbeat_interval_ms: 1000,
                node_timeout_ms: 5000,
                discovery_addresses: vec![
                    "127.0.0.1:9101".to_string(),
                    "127.0.0.1:9102".to_string(),
                ],
                enable_auto_discovery: true,
            },
        };
        configs.push(config);
    }

    // 创建存储实例
    let mut storages = vec![];
    for config in &configs {
        let storage = DistributedStorage::new(config.clone()).unwrap();
        storages.push(storage);
    }

    // 启动所有节点
    for storage in &storages {
        storage.start().await.unwrap();
    }

    // 等待集群形成
    sleep(Duration::from_secs(3)).await;

    // 测试写入数据
    let test_series = create_test_time_series(1, "test_metric", 100);
    let write_result = storages[0].write(test_series).await;
    assert!(write_result.is_ok());

    // 等待数据复制
    sleep(Duration::from_secs(2)).await;

    // 测试读取数据
    let read_result = storages[1].query_with_matchers(
        &[("__name__".to_string(), "test_metric".to_string())],
        0,
        i64::MAX
    ).await;
    assert!(read_result.is_ok());
    let series = read_result.unwrap();
    assert!(!series.is_empty());

    // 停止所有节点
    for storage in &storages {
        storage.stop().await.unwrap();
    }

    println!("Distributed cluster basic test completed successfully!");
}

/// 测试分片功能
#[tokio::test]
async fn test_shard_distribution() {
    // 创建分片配置
    let shard_config = ShardConfig {
        shard_count: 16,
        replication_factor: 3,
        enable_virtual_nodes: true,
        virtual_nodes_per_physical: 128,
    };

    // 创建分片管理器
    let shard_manager = ShardManager::new(shard_config);

    // 测试分片分配
    let shard_id = shard_manager.get_shard_for_series(12345);
    assert!(shard_id < 16);

    // 测试多个系列的分片分配
    let mut shard_distribution = std::collections::HashMap::new();
    for i in 0..100 {
        let series_id = i as u64 + 1000;
        let shard = shard_manager.get_shard_for_series(series_id);
        *shard_distribution.entry(shard).or_insert(0) += 1;
    }

    // 验证分片分布
    assert!(!shard_distribution.is_empty());
    println!("Shard distribution: {:?}", shard_distribution);

    println!("Shard distribution test completed successfully!");
}

/// 测试数据复制功能
#[tokio::test]
async fn test_data_replication() {
    // 创建3个节点的配置
    let mut configs = vec![];
    for i in 0..3 {
        let config = DistributedConfig {
            node_id: format!("node{}", i + 1),
            cluster_name: "test-cluster".to_string(),
            node_address: format!("127.0.0.1:{}", 9300 + i),
            coordinator_address: "127.0.0.1:9300".to_string(),
            is_coordinator: i == 0,
            shard_config: ShardConfig::default(),
            replication_config: ReplicationConfig {
                replication_factor: 3,
                min_write_replicas: 2,
                min_read_replicas: 2,
                async_replication: false,
                replication_timeout_ms: 5000,
                replication_queue_size: 10000,
                batch_size: 100,
            },
            cluster_config: ClusterConfig::default(),
        };
        configs.push(config);
    }

    // 创建存储实例
    let mut storages = vec![];
    for config in &configs {
        let storage = DistributedStorage::new(config.clone()).unwrap();
        storages.push(storage);
    }

    // 启动所有节点
    for storage in &storages {
        storage.start().await.unwrap();
    }

    // 等待集群形成
    sleep(Duration::from_secs(3)).await;

    // 写入测试数据
    let test_series = create_test_time_series(100, "replicated_metric", 50);
    let write_result = storages[0].write(test_series).await;
    assert!(write_result.is_ok());

    // 等待复制完成
    sleep(Duration::from_secs(2)).await;

    // 验证数据在所有节点上可用
    for (i, storage) in storages.iter().enumerate() {
        let read_result = storage.query_with_matchers(
            &[("__name__".to_string(), "replicated_metric".to_string())],
            0,
            i64::MAX
        ).await;
        assert!(read_result.is_ok(), "Node {} should have the data", i + 1);
        let series = read_result.unwrap();
        assert!(!series.is_empty(), "Node {} should have non-empty data", i + 1);
    }

    // 停止所有节点
    for storage in &storages {
        storage.stop().await.unwrap();
    }

    println!("Data replication test completed successfully!");
}

/// 测试节点故障转移
#[tokio::test]
async fn test_node_failover() {
    // 创建3个节点的配置
    let mut configs = vec![];
    for i in 0..3 {
        let config = DistributedConfig {
            node_id: format!("node{}", i + 1),
            cluster_name: "test-cluster".to_string(),
            node_address: format!("127.0.0.1:{}", 9400 + i),
            coordinator_address: "127.0.0.1:9400".to_string(),
            is_coordinator: i == 0,
            shard_config: ShardConfig::default(),
            replication_config: ReplicationConfig {
                replication_factor: 3,
                min_write_replicas: 1,
                min_read_replicas: 1,
                async_replication: true,
                replication_timeout_ms: 5000,
                replication_queue_size: 10000,
                batch_size: 100,
            },
            cluster_config: ClusterConfig {
                cluster_name: "test-cluster".to_string(),
                heartbeat_interval_ms: 1000,
                node_timeout_ms: 3000,
                discovery_addresses: vec![],
                enable_auto_discovery: false,
            },
        };
        configs.push(config);
    }

    // 创建存储实例
    let mut storages = vec![];
    for config in &configs {
        let storage = DistributedStorage::new(config.clone()).unwrap();
        storages.push(storage);
    }

    // 启动所有节点
    for storage in &storages {
        storage.start().await.unwrap();
    }

    // 等待集群形成
    sleep(Duration::from_secs(3)).await;

    // 写入测试数据
    let test_series = create_test_time_series(200, "failover_test", 30);
    let write_result = storages[0].write(test_series).await;
    assert!(write_result.is_ok());

    // 模拟节点2故障
    println!("Simulating node2 failure...");
    storages[1].stop().await.unwrap();

    // 等待故障检测
    sleep(Duration::from_secs(5)).await;

    // 验证其他节点仍然可以服务请求
    let read_result = storages[0].query_with_matchers(
        &[("__name__".to_string(), "failover_test".to_string())],
        0,
        i64::MAX
    ).await;
    assert!(read_result.is_ok(), "Coordinator should still serve requests");

    let read_result = storages[2].query_with_matchers(
        &[("__name__".to_string(), "failover_test".to_string())],
        0,
        i64::MAX
    ).await;
    assert!(read_result.is_ok(), "Node3 should still serve requests");

    // 停止剩余节点
    storages[0].stop().await.unwrap();
    storages[2].stop().await.unwrap();

    println!("Node failover test completed successfully!");
}

/// 测试集群扩展性
#[tokio::test]
async fn test_cluster_scalability() {
    // 创建初始2个节点
    let mut configs = vec![];
    for i in 0..2 {
        let config = DistributedConfig {
            node_id: format!("node{}", i + 1),
            cluster_name: "test-cluster".to_string(),
            node_address: format!("127.0.0.1:{}", 9500 + i),
            coordinator_address: "127.0.0.1:9500".to_string(),
            is_coordinator: i == 0,
            shard_config: ShardConfig {
                shard_count: 8,
                replication_factor: 2,
                enable_virtual_nodes: true,
                virtual_nodes_per_physical: 64,
            },
            replication_config: ReplicationConfig::default(),
            cluster_config: ClusterConfig::default(),
        };
        configs.push(config);
    }

    // 创建并启动初始节点
    let mut storages = vec![];
    for config in &configs {
        let storage = DistributedStorage::new(config.clone()).unwrap();
        storages.push(storage);
    }

    for storage in &storages {
        storage.start().await.unwrap();
    }

    sleep(Duration::from_secs(2)).await;

    // 写入初始数据
    for i in 0..10 {
        let series = create_test_time_series(300 + i as u64, &format!("metric_{}", i), 20);
        storages[0].write(series).await.unwrap();
    }

    // 添加第3个节点
    let new_config = DistributedConfig {
        node_id: "node3".to_string(),
        cluster_name: "test-cluster".to_string(),
        node_address: "127.0.0.1:9502".to_string(),
        coordinator_address: "127.0.0.1:9500".to_string(),
        is_coordinator: false,
        shard_config: ShardConfig {
            shard_count: 8,
            replication_factor: 2,
            enable_virtual_nodes: true,
            virtual_nodes_per_physical: 64,
        },
        replication_config: ReplicationConfig::default(),
        cluster_config: ClusterConfig::default(),
    };

    let new_storage = DistributedStorage::new(new_config).unwrap();
    new_storage.start().await.unwrap();

    // 等待新节点加入
    sleep(Duration::from_secs(3)).await;

    // 写入更多数据
    for i in 10..20 {
        let series = create_test_time_series(300 + i as u64, &format!("metric_{}", i), 20);
        new_storage.write(series).await.unwrap();
    }

    // 停止所有节点
    for storage in &storages {
        storage.stop().await.unwrap();
    }
    new_storage.stop().await.unwrap();

    println!("Cluster scalability test completed successfully!");
}

/// 创建测试时间序列
fn create_test_time_series(id: TimeSeriesId, name: &str, sample_count: usize) -> TimeSeries {
    let now = chrono::Utc::now().timestamp_millis();
    let mut samples = vec![];

    for i in 0..sample_count {
        samples.push(Sample::new(
            now - (sample_count - i) as i64 * 1000,
            i as f64 * 1.5,
        ));
    }

    TimeSeries {
        id,
        labels: vec![
            Label::new("__name__", name),
            Label::new("job", "test"),
        ],
        samples,
    }
}
