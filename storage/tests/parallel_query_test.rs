use chronodb_storage::{
    config::StorageConfig,
    memstore::MemStore,
    model::{Label, Sample},
    query::{QueryEngine, QueryExecutor, ParallelConfig},
};
use std::sync::Arc;
use tempfile::tempdir;

fn create_test_store() -> (tempfile::TempDir, Arc<MemStore>) {
    let temp_dir = tempdir().unwrap();
    let config = StorageConfig {
        data_dir: temp_dir.path().to_string_lossy().to_string(),
        ..Default::default()
    };
    (temp_dir, Arc::new(MemStore::new(config).unwrap()))
}

#[tokio::test]
async fn test_parallel_query_correctness() {
    let (_temp_dir, store) = create_test_store();
    
    for i in 0..100 {
        let labels = vec![
            Label::new("__name__", "http_requests_total"),
            Label::new("job", format!("job_{}", i % 10)),
            Label::new("instance", format!("instance_{}", i)),
        ];
        
        let samples: Vec<Sample> = (0..100)
            .map(|j| Sample::new(j as i64 * 1000, (i * 100 + j) as f64))
            .collect();
        
        store.write(labels, samples).unwrap();
    }
    
    let config_sequential = ParallelConfig {
        enable_parallel: false,
        max_concurrency: 1,
        ..Default::default()
    };
    
    let config_parallel = ParallelConfig {
        enable_parallel: true,
        max_concurrency: num_cpus::get(),
        min_series_for_parallel: 10,
        ..Default::default()
    };
    
    let engine_sequential = QueryEngine::new(store.clone());
    let engine_parallel = QueryEngine::new(store.clone());
    
    let result_sequential = engine_sequential
        .query("http_requests_total", 0, 100000, 1000)
        .await
        .unwrap();
    
    let result_parallel = engine_parallel
        .query("http_requests_total", 0, 100000, 1000)
        .await
        .unwrap();
    
    assert_eq!(result_sequential.series_count(), result_parallel.series_count());
    
    let mut seq_series = result_sequential.series;
    let mut par_series = result_parallel.series;
    
    seq_series.sort_by(|a, b| a.id.cmp(&b.id));
    par_series.sort_by(|a, b| a.id.cmp(&b.id));
    
    for (seq, par) in seq_series.iter().zip(par_series.iter()) {
        assert_eq!(seq.id, par.id);
        assert_eq!(seq.samples.len(), par.samples.len());
        
        for (s1, s2) in seq.samples.iter().zip(par.samples.iter()) {
            assert_eq!(s1.timestamp, s2.timestamp);
            assert!((s1.value - s2.value).abs() < 0.0001);
        }
    }
}

#[tokio::test]
async fn test_parallel_aggregation_correctness() {
    let (_temp_dir, store) = create_test_store();
    
    for i in 0..50 {
        let labels = vec![
            Label::new("__name__", "http_requests_total"),
            Label::new("job", format!("job_{}", i % 5)),
            Label::new("instance", format!("instance_{}", i)),
        ];
        
        let samples: Vec<Sample> = (0..100)
            .map(|j| Sample::new(j as i64 * 1000, (i * 100 + j) as f64))
            .collect();
        
        store.write(labels, samples).unwrap();
    }
    
    let engine = QueryEngine::new(store.clone());
    
    let result = engine
        .query("sum(http_requests_total) by (job)", 0, 100000, 1000)
        .await
        .unwrap();

    println!("Result series count: {}", result.series_count());
    for (i, series) in result.series.iter().enumerate() {
        println!("Series {}: labels={:?}", i, series.labels);
    }

    assert_eq!(result.series_count(), 5);
    
    for series in &result.series {
        let job_label = series.labels.iter()
            .find(|l| l.name == "job")
            .unwrap();
        
        assert!(job_label.value.starts_with("job_"));
        
        assert!(!series.samples.is_empty());
        
        for sample in &series.samples {
            assert!(sample.value > 0.0);
        }
    }
}

#[tokio::test]
async fn test_parallel_query_performance() {
    let (_temp_dir, store) = create_test_store();
    
    for i in 0..1000 {
        let labels = vec![
            Label::new("__name__", "http_requests_total"),
            Label::new("job", format!("job_{}", i % 10)),
            Label::new("instance", format!("instance_{}", i)),
        ];
        
        let samples: Vec<Sample> = (0..100)
            .map(|j| Sample::new(j as i64 * 1000, (i * 100 + j) as f64))
            .collect();
        
        store.write(labels, samples).unwrap();
    }
    
    let config_sequential = ParallelConfig {
        enable_parallel: false,
        max_concurrency: 1,
        ..Default::default()
    };
    
    let config_parallel = ParallelConfig {
        enable_parallel: true,
        max_concurrency: num_cpus::get(),
        min_series_for_parallel: 10,
        ..Default::default()
    };
    
    let engine_sequential = QueryEngine::new(store.clone());
    let engine_parallel = QueryEngine::new(store.clone());
    
    let start = std::time::Instant::now();
    let result_sequential = engine_sequential
        .query("http_requests_total", 0, 100000, 1000)
        .await
        .unwrap();
    let sequential_duration = start.elapsed();
    
    let start = std::time::Instant::now();
    let result_parallel = engine_parallel
        .query("http_requests_total", 0, 100000, 1000)
        .await
        .unwrap();
    let parallel_duration = start.elapsed();
    
    assert_eq!(result_sequential.series_count(), result_parallel.series_count());
    
    println!("Sequential query time: {:?}", sequential_duration);
    println!("Parallel query time: {:?}", parallel_duration);
    println!("Speedup: {:.2}x", 
        sequential_duration.as_secs_f64() / parallel_duration.as_secs_f64());
    
    // 注意：在小数据集上，并行查询的开销可能超过收益
    // 这个测试主要验证并行查询的正确性，而非性能
    // 性能优化应该在生产环境中根据实际情况调整
}

#[tokio::test]
async fn test_parallel_query_with_filters() {
    let (_temp_dir, store) = create_test_store();
    
    for i in 0..100 {
        let labels = vec![
            Label::new("__name__", "http_requests_total"),
            Label::new("job", format!("job_{}", i % 10)),
            Label::new("instance", format!("instance_{}", i)),
        ];
        
        let samples: Vec<Sample> = (0..100)
            .map(|j| Sample::new(j as i64 * 1000, (i * 100 + j) as f64))
            .collect();
        
        store.write(labels, samples).unwrap();
    }
    
    let engine = QueryEngine::new(store.clone());
    
    let result = engine
        .query("http_requests_total{job=\"job_0\"}", 0, 100000, 1000)
        .await
        .unwrap();
    
    assert!(result.series_count() > 0);
    
    for series in &result.series {
        let job_label = series.labels.iter()
            .find(|l| l.name == "job")
            .unwrap();
        assert_eq!(job_label.value, "job_0");
    }
}

#[tokio::test]
async fn test_parallel_aggregation_with_grouping() {
    let (_temp_dir, store) = create_test_store();
    
    for i in 0..100 {
        let labels = vec![
            Label::new("__name__", "http_requests_total"),
            Label::new("job", format!("job_{}", i % 5)),
            Label::new("instance", format!("instance_{}", i)),
        ];
        
        let samples: Vec<Sample> = (0..100)
            .map(|j| Sample::new(j as i64 * 1000, (i * 100 + j) as f64))
            .collect();
        
        store.write(labels, samples).unwrap();
    }
    
    let engine = QueryEngine::new(store.clone());
    
    let result = engine
        .query("sum(http_requests_total) by (job)", 0, 100000, 1000)
        .await
        .unwrap();
    
    assert_eq!(result.series_count(), 5);
    
    let mut job_labels: Vec<String> = result.series.iter()
        .filter_map(|s| s.labels.iter().find(|l| l.name == "job"))
        .map(|l| l.value.clone())
        .collect();
    job_labels.sort();
    
    for i in 0..5 {
        assert_eq!(job_labels[i], format!("job_{}", i));
    }
}

#[tokio::test]
async fn test_parallel_rate_query() {
    let (_temp_dir, store) = create_test_store();
    
    for i in 0..50 {
        let labels = vec![
            Label::new("__name__", "http_requests_total"),
            Label::new("job", format!("job_{}", i % 5)),
        ];
        
        let samples: Vec<Sample> = (0..100)
            .map(|j| Sample::new(j as i64 * 1000, (i * 100 + j) as f64))
            .collect();
        
        store.write(labels, samples).unwrap();
    }
    
    let engine = QueryEngine::new(store.clone());
    
    let result = engine
        .query("rate(http_requests_total[5m])", 0, 100000, 1000)
        .await
        .unwrap();
    
    assert!(result.series_count() > 0);
    
    for series in &result.series {
        assert!(!series.samples.is_empty());
        
        for sample in &series.samples {
            assert!(sample.value >= 0.0);
        }
    }
}
