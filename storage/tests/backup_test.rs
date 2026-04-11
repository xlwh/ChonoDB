use chronodb_storage::backup::{BackupManager, BackupConfig};
use std::time::Duration;
use tempfile::tempdir;

/// 测试备份配置
#[tokio::test]
async fn test_backup_config() {
    // 测试默认配置
    let default_config = BackupConfig::default();
    assert_eq!(default_config.backup_dir, "./backups");
    assert_eq!(default_config.backup_type, "full");
    assert_eq!(default_config.backup_interval_secs, 86400);
    assert_eq!(default_config.retention_days, 7);
    assert_eq!(default_config.storage_backend, "local");
    assert!(default_config.enable_verification);
    assert_eq!(default_config.parallelism, 4);

    // 测试自定义配置
    let custom_config = BackupConfig {
        backup_dir: "/custom/backups".to_string(),
        backup_type: "incremental".to_string(),
        backup_interval_secs: 3600,
        retention_days: 30,
        storage_backend: "local".to_string(),
        s3_config: None,
        gcs_config: None,
        minio_config: None,
        enable_verification: false,
        parallelism: 8,
    };

    assert_eq!(custom_config.backup_dir, "/custom/backups");
    assert_eq!(custom_config.backup_type, "incremental");
    assert_eq!(custom_config.backup_interval_secs, 3600);
    assert_eq!(custom_config.retention_days, 30);
    assert!(!custom_config.enable_verification);
    assert_eq!(custom_config.parallelism, 8);

    println!("Backup config test completed successfully!");
}

/// 测试备份管理器创建
#[tokio::test]
async fn test_backup_manager_creation() {
    let temp_dir = tempdir().unwrap();
    let backup_dir = temp_dir.path().join("backups").to_string_lossy().to_string();

    let config = BackupConfig {
        backup_dir,
        backup_type: "full".to_string(),
        backup_interval_secs: 3600,
        retention_days: 7,
        storage_backend: "local".to_string(),
        s3_config: None,
        gcs_config: None,
        minio_config: None,
        enable_verification: true,
        parallelism: 4,
    };

    let manager = BackupManager::new(config).await;
    assert!(manager.is_ok());

    println!("Backup manager creation test completed successfully!");
}

/// 测试全量备份
#[tokio::test]
async fn test_full_backup() {
    let temp_dir = tempdir().unwrap();
    let data_dir = temp_dir.path().join("data").to_string_lossy().to_string();
    let backup_dir = temp_dir.path().join("backups").to_string_lossy().to_string();

    // 创建测试数据目录和文件
    std::fs::create_dir_all(&data_dir).unwrap();
    std::fs::write(format!("{}/test1.txt", data_dir), "test data 1").unwrap();
    std::fs::write(format!("{}/test2.txt", data_dir), "test data 2").unwrap();

    // 创建子目录和文件
    std::fs::create_dir_all(format!("{}/subdir", data_dir)).unwrap();
    std::fs::write(format!("{}/subdir/test3.txt", data_dir), "test data 3").unwrap();

    let config = BackupConfig {
        backup_dir,
        backup_type: "full".to_string(),
        backup_interval_secs: 3600,
        retention_days: 7,
        storage_backend: "local".to_string(),
        s3_config: None,
        gcs_config: None,
        minio_config: None,
        enable_verification: true,
        parallelism: 4,
    };

    let mut manager = BackupManager::new(config).await.unwrap();

    // 执行备份
    let backup_id = manager.perform_backup(&data_dir).await;
    assert!(backup_id.is_ok());

    let backup_id = backup_id.unwrap();
    println!("Backup created: {}", backup_id);

    // 列出备份
    let backups = manager.list_backups().await.unwrap();
    assert_eq!(backups.len(), 1);
    assert_eq!(backups[0].backup_id, backup_id);
    assert_eq!(backups[0].backup_type, "full");
    assert_eq!(backups[0].files_count, 3); // 3个文件

    println!("Full backup test completed successfully!");
}

/// 测试备份恢复
#[tokio::test]
async fn test_backup_restore() {
    let temp_dir = tempdir().unwrap();
    let data_dir = temp_dir.path().join("data").to_string_lossy().to_string();
    let backup_dir = temp_dir.path().join("backups").to_string_lossy().to_string();
    let restore_dir = temp_dir.path().join("restore").to_string_lossy().to_string();

    // 创建测试数据
    std::fs::create_dir_all(&data_dir).unwrap();
    std::fs::write(format!("{}/test1.txt", data_dir), "test data 1").unwrap();
    std::fs::write(format!("{}/test2.txt", data_dir), "test data 2").unwrap();

    let config = BackupConfig {
        backup_dir,
        backup_type: "full".to_string(),
        backup_interval_secs: 3600,
        retention_days: 7,
        storage_backend: "local".to_string(),
        s3_config: None,
        gcs_config: None,
        minio_config: None,
        enable_verification: true,
        parallelism: 4,
    };

    let mut manager = BackupManager::new(config).await.unwrap();

    // 执行备份
    let backup_id = manager.perform_backup(&data_dir).await.unwrap();
    println!("Backup created: {}", backup_id);

    // 恢复备份到新的目录
    manager.restore_backup(&backup_id, &restore_dir).await.unwrap();

    // 验证恢复的文件
    let restored_file1 = std::fs::read_to_string(format!("{}/test1.txt", restore_dir)).unwrap();
    let restored_file2 = std::fs::read_to_string(format!("{}/test2.txt", restore_dir)).unwrap();

    assert_eq!(restored_file1, "test data 1");
    assert_eq!(restored_file2, "test data 2");

    println!("Backup restore test completed successfully!");
}

/// 测试备份列表
#[tokio::test]
async fn test_list_backups() {
    let temp_dir = tempdir().unwrap();
    let data_dir = temp_dir.path().join("data").to_string_lossy().to_string();
    let backup_dir = temp_dir.path().join("backups").to_string_lossy().to_string();

    // 创建测试数据
    std::fs::create_dir_all(&data_dir).unwrap();
    std::fs::write(format!("{}/test.txt", data_dir), "test data").unwrap();

    let config = BackupConfig {
        backup_dir,
        backup_type: "full".to_string(),
        backup_interval_secs: 3600,
        retention_days: 7,
        storage_backend: "local".to_string(),
        s3_config: None,
        gcs_config: None,
        minio_config: None,
        enable_verification: true,
        parallelism: 4,
    };

    let mut manager = BackupManager::new(config).await.unwrap();

    // 创建多个备份
    let backup1 = manager.perform_backup(&data_dir).await.unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;
    let backup2 = manager.perform_backup(&data_dir).await.unwrap();

    // 列出备份
    let backups = manager.list_backups().await.unwrap();
    assert_eq!(backups.len(), 2);

    // 验证备份按时间排序（最新的在前）
    assert!(backups[0].timestamp >= backups[1].timestamp);

    println!("List backups test completed successfully!");
    println!("Backup 1: {}", backup1);
    println!("Backup 2: {}", backup2);
}

/// 测试备份清理
#[tokio::test]
async fn test_backup_cleanup() {
    let temp_dir = tempdir().unwrap();
    let data_dir = temp_dir.path().join("data").to_string_lossy().to_string();
    let backup_dir = temp_dir.path().join("backups").to_string_lossy().to_string();

    // 创建测试数据
    std::fs::create_dir_all(&data_dir).unwrap();
    std::fs::write(format!("{}/test.txt", data_dir), "test data").unwrap();

    let config = BackupConfig {
        backup_dir,
        backup_type: "full".to_string(),
        backup_interval_secs: 3600,
        retention_days: 0, // 立即过期，用于测试清理
        storage_backend: "local".to_string(),
        s3_config: None,
        gcs_config: None,
        minio_config: None,
        enable_verification: true,
        parallelism: 4,
    };

    let mut manager = BackupManager::new(config).await.unwrap();

    // 创建备份
    let backup_id = manager.perform_backup(&data_dir).await.unwrap();
    println!("Backup created: {}", backup_id);

    // 验证备份被清理（因为 retention_days = 0）
    let backups_after = manager.list_backups().await.unwrap();
    assert_eq!(backups_after.len(), 0, "Backup should be cleaned up immediately when retention_days = 0");

    println!("Backup cleanup test completed successfully!");
}
