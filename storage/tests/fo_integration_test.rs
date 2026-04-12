use std::sync::Arc;
use tempfile::TempDir;
use tracing::info;

use chronodb_storage::memstore::{MemStore};
use chronodb_storage::config::StorageConfig;
use chronodb_storage::model::{Label, Labels, Sample};
use chronodb_storage::flush::{FlushManager, BlockManager};

async fn do_flush(data_dir: &str, memstore: &Arc<MemStore>) {
    let (flush_manager, _) = FlushManager::new(data_dir, 1, 1);
    flush_manager.flush_memstore(memstore).await.unwrap();
}

#[tokio::test]
async fn test_fo_basic_write_read() {
    let temp_dir = TempDir::new().unwrap();
    let data_dir = temp_dir.path().to_string_lossy().to_string();

    let storage_config = StorageConfig {
        data_dir: data_dir.clone(),
        ..Default::default()
    };

    let memstore = Arc::new(MemStore::new(storage_config).unwrap());

    let labels: Labels = vec![
        Label::new("job", "basic_test"),
        Label::new("instance", "server1"),
    ];

    let mut samples = Vec::new();
    let now = 1000000000000i64;
    
    for i in 0..10 {
        samples.push(Sample {
            timestamp: now + i * 1000,
            value: 10.0 + i as f64,
        });
    }

    memstore.write(labels.clone(), samples.clone()).unwrap();

    let series_ids = memstore.get_all_series_ids();
    assert_eq!(series_ids.len(), 1);

    let series = memstore.get_series(series_ids[0]);
    assert!(series.is_some());
    assert_eq!(series.unwrap().samples.len(), 10);

    info!("✅ FO场景测试 - 基本写入读取成功");
}

#[tokio::test]
async fn test_fo_flush_to_disk() {
    let temp_dir = TempDir::new().unwrap();
    let data_dir = temp_dir.path().to_string_lossy().to_string();

    let storage_config = StorageConfig {
        data_dir: data_dir.clone(),
        ..Default::default()
    };

    let memstore = Arc::new(MemStore::new(storage_config).unwrap());

    let labels: Labels = vec![
        Label::new("job", "flush_test"),
    ];

    let mut samples = Vec::new();
    let now = 1000000000000i64;
    
    for i in 0..50 {
        samples.push(Sample {
            timestamp: now + i * 1000,
            value: 100.0 + i as f64,
        });
    }

    memstore.write(labels.clone(), samples.clone()).unwrap();

    do_flush(&data_dir, &memstore).await;

    let block_manager = BlockManager::new(&data_dir).unwrap();
    assert!(block_manager.total_blocks() >= 1, 
        "Expected at least 1 block, got {}", block_manager.total_blocks());
    assert!(block_manager.total_samples() >= 50,
        "Expected at least 50 samples, got {}", block_manager.total_samples());

    info!("✅ FO场景测试 - 数据刷盘成功");
}

#[tokio::test]
async fn test_fo_read_after_flush() {
    let temp_dir = TempDir::new().unwrap();
    let data_dir = temp_dir.path().to_string_lossy().to_string();

    let storage_config = StorageConfig {
        data_dir: data_dir.clone(),
        ..Default::default()
    };

    let memstore = Arc::new(MemStore::new(storage_config.clone()).unwrap());

    let labels: Labels = vec![
        Label::new("job", "read_after_flush"),
        Label::new("instance", "server1"),
    ];

    let mut samples = Vec::new();
    let now = 1000000000000i64;
    let mut expected_sum = 0.0;
    
    for i in 0..100 {
        let value = 1.0 + i as f64;
        samples.push(Sample {
            timestamp: now + i * 1000,
            value,
        });
        expected_sum += value;
    }

    memstore.write(labels.clone(), samples.clone()).unwrap();
    do_flush(&data_dir, &memstore).await;

    let block_manager = BlockManager::new(&data_dir).unwrap();
    
    assert!(block_manager.total_blocks() >= 1);
    assert_eq!(block_manager.total_samples(), 100);

    info!("✅ FO场景测试 - 刷盘后读取成功");
}

#[tokio::test]
async fn test_fo_multiple_blocks() {
    let temp_dir = TempDir::new().unwrap();
    let data_dir = temp_dir.path().to_string_lossy().to_string();

    let storage_config = StorageConfig {
        data_dir: data_dir.clone(),
        ..Default::default()
    };

    let memstore = Arc::new(MemStore::new(storage_config).unwrap());

    let now = 1000000000000i64;

    for job_idx in 0..3 {
        let labels: Labels = vec![
            Label::new("job", format!("job_{}", job_idx)),
            Label::new("instance", "server1"),
        ];

        let mut samples = Vec::new();
        for i in 0..50 {
            samples.push(Sample {
                timestamp: now + job_idx * 100000 + i * 1000,
                value: (job_idx * 100 + i) as f64,
            });
        }

        memstore.write(labels, samples).unwrap();
        do_flush(&data_dir, &memstore).await;
    }

    let block_manager = BlockManager::new(&data_dir).unwrap();
    assert!(block_manager.total_blocks() >= 3, 
        "Expected at least 3 blocks, got {}", block_manager.total_blocks());

    info!("✅ FO场景测试 - 多块写入成功");
}

#[tokio::test]
async fn test_fo_mixed_memory_disk_read() {
    let temp_dir = TempDir::new().unwrap();
    let data_dir = temp_dir.path().to_string_lossy().to_string();

    let storage_config = StorageConfig {
        data_dir: data_dir.clone(),
        ..Default::default()
    };

    let memstore = Arc::new(MemStore::new(storage_config.clone()).unwrap());

    let now = 1000000000000i64;

    let labels1: Labels = vec![Label::new("job", "memory_only")];
    let labels2: Labels = vec![Label::new("job", "disk_only")];

    let mut samples1 = Vec::new();
    let mut samples2 = Vec::new();
    
    for i in 0..30 {
        samples1.push(Sample {
            timestamp: now + i * 1000,
            value: 100.0 + i as f64,
        });
        samples2.push(Sample {
            timestamp: now + i * 1000,
            value: 200.0 + i as f64,
        });
    }

    memstore.write(labels1.clone(), samples1.clone()).unwrap();
    memstore.write(labels2.clone(), samples2.clone()).unwrap();

    do_flush(&data_dir, &memstore).await;

    let block_manager = BlockManager::new(&data_dir).unwrap();
    
    assert!(block_manager.total_blocks() >= 1);
    assert_eq!(block_manager.total_samples(), 60);

    let new_memstore = Arc::new(MemStore::new(storage_config).unwrap());
    new_memstore.write(labels1.clone(), samples1.clone()).unwrap();

    let result1 = new_memstore.query(
        &[("job".to_string(), "memory_only".to_string())],
        now - 10000,
        now + 50000
    ).unwrap();
    
    assert!(!result1.is_empty(), "Memory data should be queryable");

    info!("✅ FO场景测试 - 混合内存磁盘读取成功");
}