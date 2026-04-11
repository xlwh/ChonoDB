use chronodb_storage::memstore::MemStore;
use chronodb_storage::config::StorageConfig;
use chronodb_storage::model::{Label, Sample};
use chronodb_storage::query::{QueryEngine, cache::CacheConfig};
use std::sync::Arc;
use std::time::{Instant, Duration};
use tempfile::tempdir;

/// 创建测试数据
fn create_test_data(store: &MemStore, num_series: usize, samples_per_series: usize) {
    for series in 0..num_series {
        let labels = vec![
            Label::new("__name__", "test_metric"),
            Label::new("series", series.to_string()),
            Label::new("job", "test"),
            Label::new("instance", format!("instance_{}", series % 10)),
        ];
        
        let mut samples = Vec::with_capacity(samples_per_series);
        for sample in 0..samples_per_series {
            let timestamp = sample as i64 * 1000; // 每秒一个样本
            let value = (series * 1000 + sample) as f64;
            samples.push(Sample::new(timestamp, value));
        }
        
        store.write_batch(vec![(labels, samples)]).unwrap();
    }
    
    // 刷新缓冲区
    store.flush().unwrap();
}

/// 测试查询性能
#[tokio::test]
async fn test_query_performance() {
    // 创建临时目录
    let temp_dir = tempdir().unwrap();
    let data_dir = temp_dir.path().to_string_lossy().to_string();

    // 创建存储配置
    let config = StorageConfig {
        data_dir,
        memstore_size: 1024 * 1024 * 1024,
        wal_size: 1024 * 1024 * 1024,
        wal_sync_interval_ms: 100,
        block_size: 64 * 1024,
        compression: Default::default(),
        retention: Default::default(),
    };

    // 创建内存存储
    let store = Arc::new(MemStore::new(config).unwrap());

    // 创建测试数据
    const NUM_SERIES: usize = 1000;
    const SAMPLES_PER_SERIES: usize = 1000;
    
    println!("Creating test data: {} series, {} samples per series", NUM_SERIES, SAMPLES_PER_SERIES);
    create_test_data(&store, NUM_SERIES, SAMPLES_PER_SERIES);
    
    // 创建查询引擎（无缓存）
    let engine_no_cache = QueryEngine::new(store.clone());
    
    // 创建查询引擎（有缓存）
    let cache_config = CacheConfig {
        max_size: 1000,
        ttl: Duration::from_secs(3600),
        enabled: true,
        max_bytes: 1024 * 1024 * 100,
    };
    let engine_with_cache = QueryEngine::with_cache(store.clone(), cache_config);

    println!("\nTesting query performance...");

    // 测试1: 简单查询（无缓存）
    println!("\n1. Testing simple query (no cache)...");
    let start = Instant::now();
    
    for i in 0..10 {
        let query = format!("test_metric{{series=\"{}\"}}", i);
        let result = engine_no_cache.query(&query, 0, 100000, 1000).await.unwrap();
        assert!(!result.is_empty());
    }
    
    let duration = start.elapsed();
    let rate = 10.0 / duration.as_secs_f64();
    println!("Simple query (no cache): {} queries/sec ({} ms total)", rate, duration.as_millis());

    // 测试2: 简单查询（有缓存）
    println!("\n2. Testing simple query (with cache)...");
    
    // 首次查询（缓存未命中）
    let start = Instant::now();
    for i in 0..10 {
        let query = format!("test_metric{{series=\"{}\"}}", i);
        let result = engine_with_cache.query(&query, 0, 100000, 1000).await.unwrap();
        assert!(!result.is_empty());
    }
    let first_duration = start.elapsed();
    
    // 再次查询（缓存命中）
    let start = Instant::now();
    for i in 0..10 {
        let query = format!("test_metric{{series=\"{}\"}}", i);
        let result = engine_with_cache.query(&query, 0, 100000, 1000).await.unwrap();
        assert!(!result.is_empty());
    }
    let second_duration = start.elapsed();
    
    println!("Simple query (cache miss): {} queries/sec ({} ms total)", 
             10.0 / first_duration.as_secs_f64(), first_duration.as_millis());
    println!("Simple query (cache hit): {} queries/sec ({} ms total)", 
             10.0 / second_duration.as_secs_f64(), second_duration.as_millis());
    
    // 打印缓存统计
    if let Some(stats) = engine_with_cache.cache_stats() {
        println!("Cache stats: hits={}, misses={}, hit_rate={:.2}%", 
                 stats.hits, stats.misses, stats.hit_rate() * 100.0);
    }

    // 测试3: 范围查询
    println!("\n3. Testing range query...");
    let start = Instant::now();
    
    for _ in 0..5 {
        let result = engine_no_cache.query("test_metric{job=\"test\"}", 0, 10000, 1000).await.unwrap();
        assert!(!result.is_empty());
    }
    
    let duration = start.elapsed();
    let rate = 5.0 / duration.as_secs_f64();
    println!("Range query: {} queries/sec ({} ms total)", rate, duration.as_millis());

    // 测试4: 聚合查询
    println!("\n4. Testing aggregation query...");
    let start = Instant::now();
    
    for _ in 0..5 {
        let result = engine_no_cache.query("sum(test_metric{job=\"test\"})", 0, 10000, 1000).await.unwrap();
        assert!(!result.is_empty());
    }
    
    let duration = start.elapsed();
    let rate = 5.0 / duration.as_secs_f64();
    println!("Aggregation query: {} queries/sec ({} ms total)", rate, duration.as_millis());

    // 测试5: 复杂查询
    println!("\n5. Testing complex query...");
    let start = Instant::now();
    
    for _ in 0..3 {
        let result = engine_no_cache.query(
            "rate(test_metric{job=\"test\"}[5m])", 
            0, 50000, 1000
        ).await.unwrap();
        // 注意：rate 查询可能返回空结果，因为需要足够的数据点
    }
    
    let duration = start.elapsed();
    let rate = 3.0 / duration.as_secs_f64();
    println!("Complex query: {} queries/sec ({} ms total)", rate, duration.as_millis());

    // 测试6: 大规模查询
    println!("\n6. Testing large scale query...");
    let start = Instant::now();
    
    let result = engine_no_cache.query("test_metric{job=\"test\"}", 0, 1000000, 10000).await.unwrap();
    
    let duration = start.elapsed();
    println!("Large scale query: {} series, {} samples, {} ms", 
             result.series_count(), result.sample_count(), duration.as_millis());

    println!("\nQuery performance test completed successfully!");
}

/// 测试查询缓存效果
#[tokio::test]
async fn test_query_cache_performance() {
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

    let store = Arc::new(MemStore::new(config).unwrap());
    create_test_data(&store, 100, 100);

    let cache_config = CacheConfig {
        max_size: 100,
        ttl: Duration::from_secs(3600),
        enabled: true,
        max_bytes: 1024 * 1024 * 100,
    };
    let engine = QueryEngine::with_cache(store.clone(), cache_config);

    println!("Testing query cache performance...");

    // 执行相同的查询多次
    const ITERATIONS: usize = 100;
    
    let start = Instant::now();
    for i in 0..ITERATIONS {
        let query = format!("test_metric{{series=\"{}\"}}", i % 10);
        let result = engine.query(&query, 0, 100000, 1000).await.unwrap();
        assert!(!result.is_empty());
    }
    let duration = start.elapsed();

    // 打印缓存统计
    if let Some(stats) = engine.cache_stats() {
        println!("Cache performance:");
        println!("- Total queries: {}", ITERATIONS);
        println!("- Cache hits: {}", stats.hits);
        println!("- Cache misses: {}", stats.misses);
        println!("- Hit rate: {:.2}%", stats.hit_rate() * 100.0);
        println!("- Average query time: {:.2} ms", duration.as_millis() as f64 / ITERATIONS as f64);
        
        // 验证缓存命中率
        assert!(stats.hit_rate() > 0.5, "Cache hit rate should be > 50%");
    }

    println!("Query cache test completed successfully!");
}
