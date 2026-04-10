use chronodb_storage::config::StorageConfig;
use chronodb_storage::memstore::MemStore;
use chronodb_storage::model::{Label, Sample};
use std::sync::Arc;
use std::time::Instant;
use tempfile::tempdir;

fn create_test_store() -> Arc<MemStore> {
    let temp_dir = tempdir().unwrap();
    let config = StorageConfig {
        data_dir: temp_dir.path().to_string_lossy().to_string(),
        ..Default::default()
    };
    Arc::new(MemStore::new(config).unwrap())
}

fn test_write_performance() {
    println!("=== 写入性能测试 ===");
    
    let sizes = vec![100, 1000, 10000, 100000];
    
    for size in sizes {
        let store = create_test_store();
        let labels = vec![
            Label::new("__name__", "test_metric"),
            Label::new("job", "test"),
        ];
        let samples: Vec<Sample> = (0..size)
            .map(|i| Sample::new(i as i64 * 1000, i as f64))
            .collect();
        
        let start = Instant::now();
        store.write(labels.clone(), samples).unwrap();
        let duration = start.elapsed();
        
        let rate = size as f64 / duration.as_secs_f64();
        println!("写入 {} 个样本: {:.2?}, 速率: {:.2} 样本/秒", size, duration, rate);
    }
}

fn test_query_performance() {
    println!("\n=== 查询性能测试 ===");
    
    let sizes = vec![100, 1000, 10000, 100000];
    
    for size in sizes {
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
        
        // 测试查询
        let start = Instant::now();
        let result = store.query(&[("job".to_string(), "test".to_string())], 0, size as i64 * 1000).unwrap();
        let duration = start.elapsed();
        
        let rate = size as f64 / duration.as_secs_f64();
        println!("查询 {} 个样本: {:.2?}, 速率: {:.2} 样本/秒, 返回 {} 个时间序列", size, duration, rate, result.len());
    }
}

fn test_compression_performance() {
    println!("\n=== 压缩性能测试 ===");
    
    use chronodb_storage::compression::{DeltaEncoder, DeltaDecoder};
    
    let sizes = vec![1000, 10000, 100000, 1000000];
    
    for size in sizes {
        let data: Vec<i64> = (0..size).map(|i| i as i64 * 1000).collect();
        
        // 测试编码
        let start = Instant::now();
        let mut encoder = DeltaEncoder::new();
        let encoded = encoder.encode_batch(&data).unwrap();
        let encode_duration = start.elapsed();
        
        // 测试解码
        let start = Instant::now();
        let mut decoder = DeltaDecoder::new();
        let decoded = decoder.decode(&encoded).unwrap();
        let decode_duration = start.elapsed();
        
        let compression_ratio = (data.len() * std::mem::size_of::<i64>()) as f64 / encoded.len() as f64;
        
        println!("压缩 {} 个时间戳:", size);
        println!("  编码时间: {:.2?}", encode_duration);
        println!("  解码时间: {:.2?}", decode_duration);
        println!("  压缩比: {:.2}x", compression_ratio);
        println!("  原始大小: {} 字节, 压缩后: {} 字节", data.len() * std::mem::size_of::<i64>(), encoded.len());
        
        // 验证解码结果
        assert_eq!(data.len(), decoded.len());
        for (i, (original, decoded)) in data.iter().zip(decoded.iter()).enumerate() {
            assert_eq!(*original, *decoded, "解码失败 at index {}", i);
        }
    }
}

fn test_downsample_performance() {
    println!("\n=== 降采样性能测试 ===");
    
    use chronodb_storage::downsample::DownsampleProcessor;
    
    let sizes = vec![1000, 10000, 100000];
    let resolutions = vec![10000, 60000, 300000]; // 10秒, 1分钟, 5分钟
    
    for size in sizes {
        let samples: Vec<Sample> = (0..size)
            .map(|i| Sample::new(i as i64 * 1000, i as f64))
            .collect();
        
        for resolution in &resolutions {
            let start = Instant::now();
            let downsampled = DownsampleProcessor::downseries(&samples, *resolution);
            let duration = start.elapsed();
            
            let reduction_ratio = size as f64 / downsampled.len() as f64;
            
            println!("降采样 {} 个样本到 {}ms 分辨率:", size, resolution);
            println!("  时间: {:.2?}", duration);
            println!("  输入: {} 样本, 输出: {} 样本", size, downsampled.len());
            println!("  压缩比: {:.2}x", reduction_ratio);
        }
    }
}

fn main() {
    println!("ChronoDB 性能测试");
    println!("==================");
    
    test_write_performance();
    test_query_performance();
    test_compression_performance();
    test_downsample_performance();
    
    println!("\n测试完成！");
}
