use chronodb_storage::distributed::{DistributedConfig, ShardConfig, ReplicationConfig, ClusterConfig, ShardManager};
use chronodb_storage::model::{TimeSeries, Sample, Label, TimeSeriesId};
use std::time::Duration;
use tokio::time::{sleep, timeout};

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

    // 验证分片分布的均匀性
    let min_count = shard_distribution.values().min().copied().unwrap_or(0);
    let max_count = shard_distribution.values().max().copied().unwrap_or(0);
    
    println!("Shard distribution: min={}, max={}", min_count, max_count);
    
    // 检查分片分布是否相对均匀（最大值不应超过最小值的10倍，对于小样本）
    // 注意：100个样本分布在16个分片上，每个分片平均6-7个，分布可能不均匀
    assert!(
        max_count <= min_count * 10 || min_count == 0,
        "Shard distribution is too uneven: min={}, max={}",
        min_count,
        max_count
    );

    println!("Shard distribution test completed successfully!");
}

/// 测试分片配置
#[tokio::test]
async fn test_shard_config() {
    // 测试默认配置
    let default_config = ShardConfig::default();
    assert_eq!(default_config.shard_count, 64);
    assert_eq!(default_config.replication_factor, 3);
    assert!(default_config.enable_virtual_nodes);
    assert_eq!(default_config.virtual_nodes_per_physical, 128);

    // 测试自定义配置
    let custom_config = ShardConfig {
        shard_count: 32,
        replication_factor: 2,
        enable_virtual_nodes: false,
        virtual_nodes_per_physical: 0,
    };
    assert_eq!(custom_config.shard_count, 32);
    assert_eq!(custom_config.replication_factor, 2);
    assert!(!custom_config.enable_virtual_nodes);
    assert_eq!(custom_config.virtual_nodes_per_physical, 0);

    println!("Shard config test completed successfully!");
}

/// 测试复制配置
#[tokio::test]
async fn test_replication_config() {
    // 测试默认配置
    let default_config = ReplicationConfig::default();
    assert_eq!(default_config.replication_factor, 3);
    assert_eq!(default_config.min_write_replicas, 2);
    assert_eq!(default_config.min_read_replicas, 1);
    assert!(default_config.async_replication);
    assert_eq!(default_config.replication_timeout_ms, 5000);
    assert_eq!(default_config.replication_queue_size, 10000);
    assert_eq!(default_config.batch_size, 100);

    // 测试同步复制配置
    let sync_config = ReplicationConfig {
        replication_factor: 3,
        min_write_replicas: 3,
        min_read_replicas: 2,
        async_replication: false,
        replication_timeout_ms: 10000,
        replication_queue_size: 5000,
        batch_size: 50,
    };
    assert_eq!(sync_config.replication_factor, 3);
    assert_eq!(sync_config.min_write_replicas, 3);
    assert_eq!(sync_config.min_read_replicas, 2);
    assert!(!sync_config.async_replication);
    assert_eq!(sync_config.replication_timeout_ms, 10000);

    println!("Replication config test completed successfully!");
}

/// 测试集群配置
#[tokio::test]
async fn test_cluster_config() {
    // 测试默认配置
    let default_config = ClusterConfig::default();
    assert_eq!(default_config.cluster_name, "chronodb-cluster");
    assert_eq!(default_config.heartbeat_interval_ms, 5000);
    assert_eq!(default_config.node_timeout_ms, 15000);
    assert!(default_config.discovery_addresses.is_empty());
    assert!(!default_config.enable_auto_discovery);

    // 测试自定义配置
    let custom_config = ClusterConfig {
        cluster_name: "test-cluster".to_string(),
        heartbeat_interval_ms: 1000,
        node_timeout_ms: 5000,
        discovery_addresses: vec![
            "127.0.0.1:9091".to_string(),
            "127.0.0.1:9092".to_string(),
        ],
        enable_auto_discovery: true,
    };
    assert_eq!(custom_config.cluster_name, "test-cluster");
    assert_eq!(custom_config.heartbeat_interval_ms, 1000);
    assert_eq!(custom_config.node_timeout_ms, 5000);
    assert_eq!(custom_config.discovery_addresses.len(), 2);
    assert!(custom_config.enable_auto_discovery);

    println!("Cluster config test completed successfully!");
}

/// 测试分布式配置
#[tokio::test]
async fn test_distributed_config() {
    // 创建完整的分布式配置
    let config = DistributedConfig {
        node_id: "node1".to_string(),
        cluster_name: "test-cluster".to_string(),
        node_address: "127.0.0.1:9090".to_string(),
        coordinator_address: "127.0.0.1:9091".to_string(),
        is_coordinator: true,
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
            discovery_addresses: vec![],
            enable_auto_discovery: false,
        },
    };

    assert_eq!(config.node_id, "node1");
    assert_eq!(config.cluster_name, "test-cluster");
    assert_eq!(config.node_address, "127.0.0.1:9090");
    assert_eq!(config.coordinator_address, "127.0.0.1:9091");
    assert!(config.is_coordinator);
    assert_eq!(config.shard_config.shard_count, 16);
    assert_eq!(config.replication_config.replication_factor, 3);
    assert_eq!(config.cluster_config.heartbeat_interval_ms, 1000);

    println!("Distributed config test completed successfully!");
}

/// 测试虚拟节点分片
#[tokio::test]
async fn test_virtual_node_sharding() {
    // 创建启用虚拟节点的配置
    let shard_config = ShardConfig {
        shard_count: 8,
        replication_factor: 2,
        enable_virtual_nodes: true,
        virtual_nodes_per_physical: 64,
    };

    let shard_manager = ShardManager::new(shard_config);

    // 测试大量系列的分片分配
    let mut shard_distribution = std::collections::HashMap::new();
    for i in 0..1000 {
        let series_id = i as u64;
        let shard = shard_manager.get_shard_for_series(series_id);
        *shard_distribution.entry(shard).or_insert(0) += 1;
    }

    // 验证所有分片都有数据
    assert_eq!(shard_distribution.len(), 8, "Not all shards have data");

    // 验证分片分布的均匀性
    let min_count = shard_distribution.values().min().copied().unwrap_or(0);
    let max_count = shard_distribution.values().max().copied().unwrap_or(0);
    
    println!(
        "Virtual node shard distribution: min={}, max={}, distribution={:?}",
        min_count, max_count, shard_distribution
    );

    // 检查分片分布是否相对均匀（最大值不应超过最小值的2倍）
    assert!(
        max_count <= min_count * 2,
        "Virtual node shard distribution is too uneven: min={}, max={}",
        min_count,
        max_count
    );

    println!("Virtual node sharding test completed successfully!");
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
