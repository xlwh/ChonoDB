use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId, Throughput};
use chronodb_storage::{
    config::StorageConfig,
    memstore::MemStore,
    model::{Label, Sample},
    query::{QueryEngine, QueryExecutor, ParallelConfig},
};
use std::sync::Arc;
use tempfile::tempdir;

fn create_test_store_with_series(num_series: usize, samples_per_series: usize) -> Arc<MemStore> {
    let temp_dir = tempdir().unwrap();
    let config = StorageConfig {
        data_dir: temp_dir.path().to_string_lossy().to_string(),
        ..Default::default()
    };
    let store = Arc::new(MemStore::new(config).unwrap());

    for i in 0..num_series {
        let labels = vec![
            Label::new("__name__", "http_requests_total"),
            Label::new("job", format!("job_{}", i % 10)),
            Label::new("instance", format!("instance_{}", i)),
        ];

        let samples: Vec<Sample> = (0..samples_per_series)
            .map(|j| Sample::new(j as i64 * 1000, (i * 100 + j) as f64))
            .collect();

        store.write(labels, samples).unwrap();
    }

    store
}

fn benchmark_parallel_query(c: &mut Criterion) {
    let mut group = c.benchmark_group("parallel_query");
    
    let test_cases = vec![
        (10, 100),
        (50, 100),
        (100, 100),
        (500, 100),
        (1000, 100),
    ];

    for (num_series, samples_per_series) in test_cases {
        let store = create_test_store_with_series(num_series, samples_per_series);
        
        group.throughput(Throughput::Elements(num_series as u64));
        
        group.bench_with_input(
            BenchmarkId::new("sequential", format!("{}_series", num_series)),
            &num_series,
            |b, _| {
                let config = ParallelConfig {
                    enable_parallel: false,
                    max_concurrency: 1,
                    ..Default::default()
                };
                let executor = QueryExecutor::with_parallel_config(store.clone(), config);
                
                b.to_async(tokio::runtime::Runtime::new().unwrap()).iter(|| {
                    let executor = executor.clone();
                    async move {
                        let matchers = vec![("job".to_string(), "job_0".to_string())];
                        let series_ids = store.find_series(&matchers).unwrap();
                        black_box(series_ids.len())
                    }
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("parallel", format!("{}_series", num_series)),
            &num_series,
            |b, _| {
                let config = ParallelConfig {
                    enable_parallel: true,
                    max_concurrency: num_cpus::get(),
                    min_series_for_parallel: 10,
                    ..Default::default()
                };
                let executor = QueryExecutor::with_parallel_config(store.clone(), config);
                
                b.to_async(tokio::runtime::Runtime::new().unwrap()).iter(|| {
                    let executor = executor.clone();
                    async move {
                        let matchers = vec![("job".to_string(), "job_0".to_string())];
                        let series_ids = store.find_series(&matchers).unwrap();
                        black_box(series_ids.len())
                    }
                });
            },
        );
    }

    group.finish();
}

fn benchmark_parallel_aggregation(c: &mut Criterion) {
    let mut group = c.benchmark_group("parallel_aggregation");
    
    let num_series = 100;
    let samples_per_series = 100;
    let store = create_test_store_with_series(num_series, samples_per_series);
    
    group.throughput(Throughput::Elements(num_series as u64));

    group.bench_function("sequential_sum", |b| {
        let config = ParallelConfig {
            enable_parallel: false,
            max_concurrency: 1,
            ..Default::default()
        };
        let engine = QueryEngine::new(store.clone());
        
        b.to_async(tokio::runtime::Runtime::new().unwrap()).iter(|| {
            let engine = engine.clone();
            async move {
                let result = engine.query(
                    "sum(http_requests_total)",
                    0,
                    samples_per_series as i64 * 1000,
                    1000
                ).await.unwrap();
                black_box(result.series_count())
            }
        });
    });

    group.bench_function("parallel_sum", |b| {
        let config = ParallelConfig {
            enable_parallel: true,
            max_concurrency: num_cpus::get(),
            min_series_for_parallel: 10,
            ..Default::default()
        };
        let engine = QueryEngine::new(store.clone());
        
        b.to_async(tokio::runtime::Runtime::new().unwrap()).iter(|| {
            let engine = engine.clone();
            async move {
                let result = engine.query(
                    "sum(http_requests_total)",
                    0,
                    samples_per_series as i64 * 1000,
                    1000
                ).await.unwrap();
                black_box(result.series_count())
            }
        });
    });

    group.finish();
}

fn benchmark_query_engine(c: &mut Criterion) {
    let mut group = c.benchmark_group("query_engine");
    
    let num_series = 1000;
    let samples_per_series = 100;
    let store = create_test_store_with_series(num_series, samples_per_series);
    
    group.throughput(Throughput::Elements(num_series as u64));

    group.bench_function("simple_query", |b| {
        let engine = QueryEngine::new(store.clone());
        
        b.to_async(tokio::runtime::Runtime::new().unwrap()).iter(|| {
            let engine = engine.clone();
            async move {
                let result = engine.query(
                    "http_requests_total{job=\"job_0\"}",
                    0,
                    samples_per_series as i64 * 1000,
                    1000
                ).await.unwrap();
                black_box(result.series_count())
            }
        });
    });

    group.bench_function("complex_query", |b| {
        let engine = QueryEngine::new(store.clone());
        
        b.to_async(tokio::runtime::Runtime::new().unwrap()).iter(|| {
            let engine = engine.clone();
            async move {
                let result = engine.query(
                    "sum(http_requests_total) by (job)",
                    0,
                    samples_per_series as i64 * 1000,
                    1000
                ).await.unwrap();
                black_box(result.series_count())
            }
        });
    });

    group.bench_function("rate_query", |b| {
        let engine = QueryEngine::new(store.clone());
        
        b.to_async(tokio::runtime::Runtime::new().unwrap()).iter(|| {
            let engine = engine.clone();
            async move {
                let result = engine.query(
                    "rate(http_requests_total[5m])",
                    0,
                    samples_per_series as i64 * 1000,
                    1000
                ).await.unwrap();
                black_box(result.series_count())
            }
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    benchmark_parallel_query,
    benchmark_parallel_aggregation,
    benchmark_query_engine,
);

criterion_main!(benches);
