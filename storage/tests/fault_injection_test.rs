use chronodb_storage::config::StorageConfig;
use chronodb_storage::memstore::MemStore;
use chronodb_storage::distributed::{ClusterManager, ClusterConfig, NodeInfo, NodeStatus};
use chronodb_storage::distributed::{ReplicationManager, ReplicationConfig};
use chronodb_storage::distributed::{ShardManager, ShardConfig};
use chronodb_storage::rpc::ClusterRpcManager;
use chronodb_storage::fault_injection::{FaultInjector, FaultInjectionTest, FaultInjectionConfig, FaultType, create_test_suite};
use std::sync::Arc;
use tempfile::tempdir;

#[tokio::test]
async fn test_node_failure() {
    // 创建测试存储
    let temp_dir = tempdir().unwrap();
    let data_dir = temp_dir.path().to_string_lossy().to_string();
    let storage_config = StorageConfig {
        data_dir: data_dir.clone(),
        ..Default::default()
    };
    let _store = MemStore::new(storage_config).unwrap();

    // 初始化集群管理器（不启动心跳任务）
    let cluster_manager = Arc::new(ClusterManager::new(ClusterConfig::default()));

    // 注册测试节点（不调用 start()，仅使用 register_node）
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

    // 初始化复制管理器
    let replication_config = ReplicationConfig::default();
    let replication_manager = Arc::new(ReplicationManager::new(replication_config));
    let rpc_manager = Arc::new(ClusterRpcManager::new());

    // 初始化分片管理器
    let shard_config = ShardConfig {
        shard_count: 128,
        replication_factor: 2,
        enable_virtual_nodes: false,
        virtual_nodes_per_physical: 0,
    };
    let shard_manager = Arc::new(ShardManager::new(shard_config));

    // 创建故障注入器
    let injector = FaultInjector::new(
        cluster_manager.clone(),
        replication_manager.clone(),
        shard_manager.clone(),
        rpc_manager.clone(),
    );

    // 创建故障注入测试
    let test = FaultInjectionTest::new(injector);

    // 配置节点故障测试
    let config = FaultInjectionConfig {
        fault_type: FaultType::NodeFailure("node1".to_string()),
        duration: std::time::Duration::from_millis(10),
        intensity: 1.0,
        auto_recover: true,
    };

    // 运行测试
    let result = test.run_test(config).await.unwrap();
    assert!(result.success);
}

#[tokio::test]
async fn test_leader_election_failure() {
    // 创建测试存储
    let temp_dir = tempdir().unwrap();
    let data_dir = temp_dir.path().to_string_lossy().to_string();
    let storage_config = StorageConfig {
        data_dir: data_dir.clone(),
        ..Default::default()
    };
    let _store = MemStore::new(storage_config).unwrap();

    // 初始化集群管理器（不启动心跳任务）
    let cluster_manager = Arc::new(ClusterManager::new(ClusterConfig::default()));

    // 注册测试节点
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

    // 初始化复制管理器
    let replication_config = ReplicationConfig::default();
    let replication_manager = Arc::new(ReplicationManager::new(replication_config));
    let rpc_manager = Arc::new(ClusterRpcManager::new());

    // 初始化分片管理器
    let shard_config = ShardConfig {
        shard_count: 128,
        replication_factor: 2,
        enable_virtual_nodes: false,
        virtual_nodes_per_physical: 0,
    };
    let shard_manager = Arc::new(ShardManager::new(shard_config));

    // 创建故障注入器
    let injector = FaultInjector::new(
        cluster_manager.clone(),
        replication_manager.clone(),
        shard_manager.clone(),
        rpc_manager.clone(),
    );

    // 创建故障注入测试
    let test = FaultInjectionTest::new(injector);

    // 配置领导者选举失败测试
    let config = FaultInjectionConfig {
        fault_type: FaultType::LeaderElectionFailure,
        duration: std::time::Duration::from_millis(10),
        intensity: 1.0,
        auto_recover: true,
    };

    // 运行测试
    let result = test.run_test(config).await.unwrap();
    assert!(result.success);
}

#[tokio::test]
async fn test_network_delay() {
    // 创建测试存储
    let temp_dir = tempdir().unwrap();
    let data_dir = temp_dir.path().to_string_lossy().to_string();
    let storage_config = StorageConfig {
        data_dir: data_dir.clone(),
        ..Default::default()
    };
    let _store = MemStore::new(storage_config).unwrap();

    // 初始化集群管理器（不启动心跳任务）
    let cluster_manager = Arc::new(ClusterManager::new(ClusterConfig::default()));

    // 注册测试节点
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

    // 初始化复制管理器
    let replication_config = ReplicationConfig::default();
    let replication_manager = Arc::new(ReplicationManager::new(replication_config));
    let rpc_manager = Arc::new(ClusterRpcManager::new());

    // 初始化分片管理器
    let shard_config = ShardConfig {
        shard_count: 128,
        replication_factor: 2,
        enable_virtual_nodes: false,
        virtual_nodes_per_physical: 0,
    };
    let shard_manager = Arc::new(ShardManager::new(shard_config));

    // 创建故障注入器
    let injector = FaultInjector::new(
        cluster_manager.clone(),
        replication_manager.clone(),
        shard_manager.clone(),
        rpc_manager.clone(),
    );

    // 创建故障注入测试
    let test = FaultInjectionTest::new(injector);

    // 配置网络延迟测试
    let config = FaultInjectionConfig {
        fault_type: FaultType::NetworkDelay(std::time::Duration::from_millis(1)),
        duration: std::time::Duration::from_millis(10),
        intensity: 1.0,
        auto_recover: true,
    };

    // 运行测试
    let result = test.run_test(config).await.unwrap();
    assert!(result.success);
}

#[tokio::test]
async fn test_test_suite() {
    // 创建测试存储
    let temp_dir = tempdir().unwrap();
    let data_dir = temp_dir.path().to_string_lossy().to_string();
    let storage_config = StorageConfig {
        data_dir: data_dir.clone(),
        ..Default::default()
    };
    let _store = MemStore::new(storage_config).unwrap();

    // 初始化集群管理器（不启动心跳任务）
    let cluster_manager = Arc::new(ClusterManager::new(ClusterConfig::default()));

    // 注册测试节点
    let node_info1 = NodeInfo {
        node_id: "node1".to_string(),
        address: "localhost:9090".to_string(),
        status: NodeStatus::Online,
        last_heartbeat: chrono::Utc::now().timestamp_millis(),
        shard_count: 100,
        series_count: 1000,
        is_leader: false,
        version: "1.0.0".to_string(),
    };
    cluster_manager.register_node(node_info1).await.unwrap();

    // 初始化复制管理器
    let replication_config = ReplicationConfig::default();
    let replication_manager = Arc::new(ReplicationManager::new(replication_config));
    let rpc_manager = Arc::new(ClusterRpcManager::new());

    // 初始化分片管理器
    let shard_config = ShardConfig {
        shard_count: 128,
        replication_factor: 2,
        enable_virtual_nodes: false,
        virtual_nodes_per_physical: 0,
    };
    let shard_manager = Arc::new(ShardManager::new(shard_config));

    // 创建故障注入器
    let injector = FaultInjector::new(
        cluster_manager.clone(),
        replication_manager.clone(),
        shard_manager.clone(),
        rpc_manager.clone(),
    );

    // 创建故障注入测试
    let test = FaultInjectionTest::new(injector);

    // 获取预定义测试套件
    let test_suite = create_test_suite();

    // 运行测试套件
    let results = test.run_test_suite(&test_suite).await.unwrap();
    assert!(!results.is_empty());
}
