use chronodb_storage::distributed::{DistributedStorage, DistributedConfig};
use chronodb_storage::rpc::{ClusterRpcManager, RpcClient};
use chronodb_storage::model::{TimeSeries, Labels, Sample};
use std::net::SocketAddr;
use std::time::Duration;
use tokio::time::sleep;

#[tokio::test]
async fn test_heartbeat_mechanism() {
    // 创建两个节点的配置
    let config1 = DistributedConfig {
        node_id: "node1".to_string(),
        node_address: "127.0.0.1:9093".to_string(),
        coordinator_address: "127.0.0.1:9093".to_string(),
        is_coordinator: true,
        ..Default::default()
    };

    let config2 = DistributedConfig {
        node_id: "node2".to_string(),
        node_address: "127.0.0.1:9094".to_string(),
        coordinator_address: "127.0.0.1:9093".to_string(),
        is_coordinator: false,
        ..Default::default()
    };

    // 创建存储实例
    let storage1 = DistributedStorage::new(config1).unwrap();
    let storage2 = DistributedStorage::new(config2).unwrap();

    // 启动存储实例
    storage1.start().await.unwrap();
    storage2.start().await.unwrap();

    // 等待节点启动
    sleep(Duration::from_secs(2)).await;

    // 测试心跳机制
    println!("Testing heartbeat mechanism...");
    let addr1: SocketAddr = "127.0.0.1:9093".parse().unwrap();
    let client1 = RpcClient::new(addr1);

    // 发送心跳
    for i in 0..5 {
        let heartbeat_response = client1.heartbeat(format!("test-node-{}", i)).await;
        assert!(heartbeat_response.is_ok());
        assert!(heartbeat_response.unwrap().success);
        sleep(Duration::from_millis(500)).await;
    }

    // 停止存储实例
    storage1.stop().await.unwrap();
    storage2.stop().await.unwrap();

    // 清理
    println!("Heartbeat test completed successfully!");
}
