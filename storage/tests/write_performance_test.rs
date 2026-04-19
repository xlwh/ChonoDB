use chronodb_storage::memstore::MemStore;
use chronodb_storage::config::StorageConfig;
use chronodb_storage::model::{Labels, Label, Sample};
use std::time::Instant;
use tempfile::tempdir;

/// 测试写入性能
#[tokio::test]
async fn test_write_performance() {
    // 创建临时目录
    let temp_dir = tempdir().unwrap();
    let data_dir = temp_dir.path().to_string_lossy().to_string();

    // 创建存储配置
    let config = StorageConfig {
        data_dir,
        memstore_size: 1024 * 1024 * 1024, // 1GB
        wal_size: 1024 * 1024 * 1024,
        wal_sync_interval_ms: 100,
        block_size: 64 * 1024,
        compression: Default::default(),
        retention: Default::default(),
    };

    // 创建内存存储
    let store = MemStore::new(config).unwrap();

    // 测试参数
    const NUM_SERIES: usize = 1000;
    const SAMPLES_PER_SERIES: usize = 100;
    const TOTAL_SAMPLES: usize = NUM_SERIES * SAMPLES_PER_SERIES;

    println!("Testing write performance: {} series, {} samples per series, total {} samples",
             NUM_SERIES, SAMPLES_PER_SERIES, TOTAL_SAMPLES);

    // 测试1: 单个写入
    println!("\n1. Testing single writes...");
    let start = Instant::now();
    
    for series in 0..NUM_SERIES {
        let labels = vec![
            Label::new("__name__", "test_metric"),
            Label::new("series", series.to_string()),
            Label::new("job", "test"),
        ];
        
        for sample in 0..SAMPLES_PER_SERIES {
            let timestamp = Instant::now().elapsed().as_millis() as i64;
            let value = (series * 1000 + sample) as f64;
            let sample = Sample::new(timestamp, value);
            
            store.write_single(labels.clone(), sample).unwrap();
        }
    }
    
    let duration = start.elapsed();
    let rate = TOTAL_SAMPLES as f64 / duration.as_secs_f64();
    println!("Single write: {} samples/sec ({} ms total)", rate, duration.as_millis());

    // 测试2: 批量写入
    println!("\n2. Testing batch writes...");
    let start = Instant::now();
    
    let mut batch = Vec::with_capacity(NUM_SERIES);
    
    for series in 0..NUM_SERIES {
        let labels = vec![
            Label::new("__name__", "test_metric_batch"),
            Label::new("series", series.to_string()),
            Label::new("job", "test"),
        ];
        
        let mut samples = Vec::with_capacity(SAMPLES_PER_SERIES);
        for sample in 0..SAMPLES_PER_SERIES {
            let timestamp = Instant::now().elapsed().as_millis() as i64;
            let value = (series * 1000 + sample) as f64;
            samples.push(Sample::new(timestamp, value));
        }
        
        batch.push((labels, samples));
    }
    
    store.write_batch(batch).unwrap();
    
    let duration = start.elapsed();
    let rate = TOTAL_SAMPLES as f64 / duration.as_secs_f64();
    println!("Batch write: {} samples/sec ({} ms total)", rate, duration.as_millis());

    // 测试3: 混合写入（模拟真实场景）
    println!("\n3. Testing mixed writes (simulating real scenario)...");
    let start = Instant::now();
    
    let mut total_written = 0;
    for series in 0..NUM_SERIES {
        let labels = vec![
            Label::new("__name__", "test_metric_mixed"),
            Label::new("series", series.to_string()),
            Label::new("job", "test"),
        ];
        
        // 模拟不同批次大小
        for batch_size in &[1, 5, 10, 20] {
            let mut samples = Vec::with_capacity(*batch_size);
            for _ in 0..*batch_size {
                let timestamp = Instant::now().elapsed().as_millis() as i64;
                let value = (series * 1000 + total_written) as f64;
                samples.push(Sample::new(timestamp, value));
                total_written += 1;
            }
            
            store.write(labels.clone(), samples).unwrap();
        }
    }
    
    let duration = start.elapsed();
    let rate = total_written as f64 / duration.as_secs_f64();
    println!("Mixed write: {} samples/sec ({} ms total)", rate, duration.as_millis());

    // 打印统计信息
    let stats = store.stats();
    println!("\nStats:");
    println!("- Total series: {}", stats.total_series);
    println!("- Total samples: {}", stats.total_samples);
    println!("- Writes: {}", stats.writes);

    // 验证数据写入成功
    assert!(stats.total_series > 0);
    assert!(stats.total_samples > 0);
    assert!(stats.writes > 0);

    println!("\nWrite performance test completed successfully!");
}

/// 测试写入缓冲区效果
#[tokio::test]
async fn test_write_buffer_effect() {
    let temp_dir = tempdir().unwrap();
    let data_dir = temp_dir.path().to_string_lossy().to_string();

    let config = StorageConfig {
        data_dir,
        memstore_size: 1024 * 1024 * 1024,
        wal_size: 1024 * 1024 * 1024,
        wal_sync_interval_ms: 100,
        block_size: 64 * 1024,
        compression: Default::default(),
        retention: Default::default(),
    };

    let store = MemStore::new(config).unwrap();

    const TEST_SAMPLES: usize = 10000;
    
    println!("Testing write buffer effect with {} samples...", TEST_SAMPLES);

    let start = Instant::now();
    
    for i in 0..TEST_SAMPLES {
        let labels = vec![
            Label::new("__name__", "buffer_test"),
            Label::new("i", (i % 100).to_string()), // 100 different series
        ];
        
        let sample = Sample::new(
            Instant::now().elapsed().as_millis() as i64,
            i as f64,
        );
        
        store.write_single(labels, sample).unwrap();
    }
    
    // 手动刷新缓冲区
    store.flush().unwrap();
    
    let duration = start.elapsed();
    let rate = TEST_SAMPLES as f64 / duration.as_secs_f64();
    
    let stats = store.stats();
    
    println!("- Write rate: {} samples/sec ({} ms total)", rate, duration.as_millis());
    println!("- Total series: {}", stats.total_series);
    println!("- Total samples: {}", stats.total_samples);

    assert_eq!(stats.total_samples, TEST_SAMPLES as u64);

    println!("Write buffer test completed successfully!");
}
