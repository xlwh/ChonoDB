use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use chronodb_storage::config::StorageConfig;
use chronodb_storage::memstore::MemStore;
use chronodb_storage::model::{Label, Sample};
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

fn bench_write(c: &mut Criterion) {
    let mut group = c.benchmark_group("write");

    for size in [100, 1000, 10000].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let store = create_test_store();
            let labels = vec![
                Label::new("__name__", "test_metric"),
                Label::new("job", "test"),
            ];
            let samples: Vec<Sample> = (0..size)
                .map(|i| Sample::new(i as i64 * 1000, i as f64))
                .collect();

            b.iter(|| {
                store.write(labels.clone(), samples.clone()).unwrap();
            });
        });
    }

    group.finish();
}

fn bench_query(c: &mut Criterion) {
    let mut group = c.benchmark_group("query");

    for size in [100, 1000, 10000].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let store = create_test_store();
            
            // 准备数据
            let labels = vec![
                Label::new("__name__", "test_metric"),
                Label::new("job", "test"),
            ];
            let samples: Vec<Sample> = (0..size)
                .map(|i| Sample::new(i as i64 * 1000, i as f64))
                .collect();
            store.write(labels.clone(), samples).unwrap();

            b.iter(|| {
                store.query(&[("job".to_string(), "test".to_string())], 0, size as i64 * 1000).unwrap();
            });
        });
    }

    group.finish();
}

fn bench_compression(c: &mut Criterion) {
    use chronodb_storage::compression::{DeltaEncoder, DeltaDecoder};

    let mut group = c.benchmark_group("compression");

    for size in [1000, 10000, 100000].iter() {
        group.bench_with_input(BenchmarkId::new("delta_encode", size), size, |b, &size| {
            let data: Vec<i64> = (0..size).map(|i| i as i64 * 1000).collect();
            let mut encoder = DeltaEncoder::new();

            b.iter(|| {
                encoder.encode(black_box(&data)).unwrap();
            });
        });

        group.bench_with_input(BenchmarkId::new("delta_decode", size), size, |b, &size| {
            let data: Vec<i64> = (0..size).map(|i| i as i64 * 1000).collect();
            let mut encoder = DeltaEncoder::new();
            let encoded = encoder.encode(&data).unwrap();
            let mut decoder = DeltaDecoder::new();

            b.iter(|| {
                decoder.decode(black_box(&encoded)).unwrap();
            });
        });
    }

    group.finish();
}

fn bench_bitmap_index(c: &mut Criterion) {
    use chronodb_storage::index::BitmapIndex;

    let mut group = c.benchmark_group("bitmap_index");

    for size in [1000, 10000, 100000].iter() {
        group.bench_with_input(BenchmarkId::new("add", size), size, |b, &size| {
            let mut index = BitmapIndex::new();

            b.iter(|| {
                for i in 0..size {
                    index.add_series(i as u64, &[
                        ("job".to_string(), "test".to_string()),
                        ("instance".to_string(), format!("host{}", i % 10)),
                    ]);
                }
            });
        });

        group.bench_with_input(BenchmarkId::new("query", size), size, |b, &size| {
            let mut index = BitmapIndex::new();
            for i in 0..size {
                index.add_series(i as u64, &[
                    ("job".to_string(), "test".to_string()),
                    ("instance".to_string(), format!("host{}", i % 10)),
                ]);
            }

            b.iter(|| {
                index.query_equal("job", "test");
            });
        });
    }

    group.finish();
}

fn bench_downsample(c: &mut Criterion) {
    use chronodb_storage::query::downsample_router::DownsampleQueryExecutor;
    use chronodb_storage::query::downsample_router::DownsampleRouter;

    let mut group = c.benchmark_group("downsample");

    for size in [1000, 10000, 100000].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let store = create_test_store();
            let router = DownsampleRouter::new(true);
            let executor = DownsampleQueryExecutor::new(store.clone(), router);

            // 准备数据
            let labels = vec![
                Label::new("__name__", "test_metric"),
                Label::new("job", "test"),
            ];
            let samples: Vec<Sample> = (0..size)
                .map(|i| Sample::new(i as i64 * 1000, i as f64))
                .collect();
            store.write(labels, samples).unwrap();

            b.iter(|| {
                let runtime = tokio::runtime::Runtime::new().unwrap();
                runtime.block_on(async {
                    executor.query(&[1], 0, size as i64 * 1000, "avg").await.unwrap();
                });
            });
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_write,
    bench_query,
    bench_compression,
    bench_bitmap_index,
    bench_downsample
);
criterion_main!(benches);
