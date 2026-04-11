use chronodb_storage::migration::{DataMigrator, MigrationConfig, MigrationStats, DataSourceType};
use chronodb_storage::export::{ExportData, ExportFormat, ExportTimeSeries, ExportMetadata, ExportSample};
use chronodb_storage::memstore::MemStore;
use chronodb_storage::config::StorageConfig;
use std::sync::Arc;
use tempfile::tempdir;

/// 测试迁移配置
#[test]
fn test_migration_config() {
    let config = MigrationConfig::default();
    assert_eq!(config.batch_size, 1000);
    assert_eq!(config.concurrency, 4);
    assert_eq!(config.timeout_secs, 300);
    assert!(config.verify_data);
    assert!(!config.skip_errors);

    println!("Migration config test completed successfully!");
}

/// 测试迁移统计
#[test]
fn test_migration_stats() {
    let mut stats = MigrationStats::new();
    assert!(stats.start_time.is_some());
    
    stats.total_series = 100;
    stats.total_samples = 1000;
    stats.processed_series = 90;
    stats.processed_samples = 900;
    
    assert_eq!(stats.success_rate(), 90.0);
    
    stats.finish();
    assert!(stats.end_time.is_some());

    println!("Migration stats test completed successfully!");
}

/// 测试数据源类型
#[test]
fn test_data_source_type() {
    assert_eq!(DataSourceType::Prometheus.to_string(), "prometheus");
    assert_eq!(DataSourceType::InfluxDB.to_string(), "influxdb");
    assert_eq!(DataSourceType::ChronoDB.to_string(), "chronodb");
    assert_eq!(DataSourceType::TimescaleDB.to_string(), "timescaledb");
    assert_eq!(DataSourceType::OpenTSDB.to_string(), "opentsdb");

    println!("Data source type test completed successfully!");
}

/// 测试数据导出
#[tokio::test]
async fn test_export_data() {
    let temp_dir = tempdir().unwrap();
    let data_dir = temp_dir.path().join("data").to_string_lossy().to_string();
    let export_file = temp_dir.path().join("export.json");

    // 创建存储
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

    // 创建迁移器
    let migrator = DataMigrator::new(store.clone(), MigrationConfig::default());

    // 导出数据（即使没有数据也应该成功）
    let stats = migrator.export_to_file(
        &export_file,
        ExportFormat::Json,
        "test_metric",
        0,
        1000000
    ).await;

    assert!(stats.is_ok());
    let stats = stats.unwrap();
    
    println!("Export stats: {:?}", stats);
    println!("Export file created: {:?}", export_file.exists());

    println!("Export data test completed successfully!");
}

/// 测试数据导入
#[tokio::test]
async fn test_import_data() {
    let temp_dir = tempdir().unwrap();
    let data_dir = temp_dir.path().join("data").to_string_lossy().to_string();
    let import_file = temp_dir.path().join("import.json");

    // 创建测试数据
    let export_data = ExportData::new()
        .with_query("cpu_usage".to_string())
        .with_time_range(1609459200, 1609545600);

    let ts1 = ExportTimeSeries {
        metadata: ExportMetadata {
            metric_name: "cpu_usage".to_string(),
            labels: vec![
                ("__name__".to_string(), "cpu_usage".to_string()),
                ("server".to_string(), "server1".to_string()),
            ],
            unit: Some("%".to_string()),
            description: Some("CPU usage percentage".to_string()),
        },
        samples: vec![
            ExportSample { timestamp: 1609459200, value: 50.5 },
            ExportSample { timestamp: 1609459260, value: 60.0 },
            ExportSample { timestamp: 1609459320, value: 55.5 },
        ],
    };

    let export_data = export_data.add_time_series(ts1);

    // 写入测试文件
    let json_content = export_data.to_json().unwrap();
    tokio::fs::write(&import_file, json_content).await.unwrap();

    // 创建存储
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

    // 创建迁移器
    let migrator = DataMigrator::new(store.clone(), MigrationConfig::default());

    // 导入数据
    let stats = migrator.import_from_file(
        &import_file,
        ExportFormat::Json,
        DataSourceType::ChronoDB
    ).await;

    assert!(stats.is_ok());
    let stats = stats.unwrap();
    
    println!("Import stats: {:?}", stats);
    assert_eq!(stats.total_series, 1);
    assert_eq!(stats.total_samples, 3);
    assert_eq!(stats.processed_series, 1);
    assert_eq!(stats.processed_samples, 3);

    println!("Import data test completed successfully!");
}

/// 测试从 Prometheus 迁移
#[tokio::test]
async fn test_migrate_from_prometheus() {
    let temp_dir = tempdir().unwrap();
    let data_dir = temp_dir.path().join("data").to_string_lossy().to_string();

    // 创建存储
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

    // 创建迁移器
    let migrator = DataMigrator::new(store.clone(), MigrationConfig::default());

    // 从 Prometheus 迁移（简化实现，返回空统计）
    let stats = migrator.migrate_from(
        DataSourceType::Prometheus,
        "http://localhost:9090"
    ).await;

    assert!(stats.is_ok());
    let stats = stats.unwrap();
    
    println!("Prometheus migration stats: {:?}", stats);

    println!("Migrate from Prometheus test completed successfully!");
}

/// 测试从 InfluxDB 迁移
#[tokio::test]
async fn test_migrate_from_influxdb() {
    let temp_dir = tempdir().unwrap();
    let data_dir = temp_dir.path().join("data").to_string_lossy().to_string();

    // 创建存储
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

    // 创建迁移器
    let migrator = DataMigrator::new(store.clone(), MigrationConfig::default());

    // 从 InfluxDB 迁移（简化实现，返回空统计）
    let stats = migrator.migrate_from(
        DataSourceType::InfluxDB,
        "http://localhost:8086"
    ).await;

    assert!(stats.is_ok());
    let stats = stats.unwrap();
    
    println!("InfluxDB migration stats: {:?}", stats);

    println!("Migrate from InfluxDB test completed successfully!");
}
