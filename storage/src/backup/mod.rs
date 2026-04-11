use crate::error::Result;
use crate::storage::backend::{StorageBackend, StorageOptions};
use crate::storage::local::LocalStorage;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tracing::{info, warn, debug};

/// 备份配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupConfig {
    /// 备份目录
    pub backup_dir: String,
    /// 备份类型: full | incremental
    pub backup_type: String,
    /// 备份频率（秒）
    pub backup_interval_secs: u64,
    /// 备份保留天数
    pub retention_days: u64,
    /// 备份存储后端: local | s3 | gcs | minio
    pub storage_backend: String,
    /// S3 配置（当 storage_backend 为 s3 时使用）
    pub s3_config: Option<S3Config>,
    /// GCS 配置（当 storage_backend 为 gcs 时使用）
    pub gcs_config: Option<GCSConfig>,
    /// MinIO 配置（当 storage_backend 为 minio 时使用）
    pub minio_config: Option<MinIOConfig>,
    /// 启用备份验证
    pub enable_verification: bool,
    /// 备份并行度
    pub parallelism: usize,
}

impl Default for BackupConfig {
    fn default() -> Self {
        Self {
            backup_dir: "./backups".to_string(),
            backup_type: "full".to_string(),
            backup_interval_secs: 86400, // 24小时
            retention_days: 7,
            storage_backend: "local".to_string(),
            s3_config: None,
            gcs_config: None,
            minio_config: None,
            enable_verification: true,
            parallelism: 4,
        }
    }
}

/// S3 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct S3Config {
    pub bucket: String,
    pub region: String,
    pub access_key: String,
    pub secret_key: String,
    pub endpoint: Option<String>,
}

/// GCS 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GCSConfig {
    pub bucket: String,
    pub service_account_key: String,
}

/// MinIO 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MinIOConfig {
    pub endpoint: String,
    pub bucket: String,
    pub access_key: String,
    pub secret_key: String,
}

/// 备份元数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupMetadata {
    pub backup_id: String,
    pub backup_type: String,
    pub timestamp: DateTime<Utc>,
    pub source_dir: String,
    pub storage_backend: String,
    pub files_count: u64,
    pub total_size: u64,
    pub previous_backup_id: Option<String>,
    pub verification_status: Option<VerificationStatus>,
}

/// 验证状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationStatus {
    pub verified_at: DateTime<Utc>,
    pub success: bool,
    pub errors: Vec<String>,
}

/// 备份管理器
#[derive(Clone)]
pub struct BackupManager {
    config: BackupConfig,
    backend: Arc<dyn StorageBackend>,
    last_backup_id: Option<String>,
}

impl BackupManager {
    pub async fn new(config: BackupConfig) -> Result<Self> {
        // 初始化存储后端
        let backend: Arc<dyn StorageBackend> = match config.storage_backend.as_str() {
            "local" => {
                // 确保本地备份目录存在
                let backup_path = Path::new(&config.backup_dir);
                std::fs::create_dir_all(backup_path)?;
                // 初始化本地存储后端
                let options = StorageOptions::default()
                    .with_local_path(backup_path.to_str().unwrap());
                let local_storage = LocalStorage::new(options).await?;
                Arc::new(local_storage)
            },
            "s3" => {
                // 初始化 S3 存储后端
                todo!()
            },
            "gcs" => {
                // 初始化 GCS 存储后端
                todo!()
            },
            "minio" => {
                // 初始化 MinIO 存储后端
                todo!()
            },
            _ => {
                return Err(crate::error::Error::Internal(
                    format!("Unsupported storage backend: {}", config.storage_backend)
                ));
            }
        };

        Ok(Self {
            config,
            backend,
            last_backup_id: None,
        })
    }

    /// 执行备份
    pub async fn perform_backup(&mut self, data_dir: &str) -> Result<String> {
        // 使用毫秒级时间戳和随机数确保备份 ID 唯一
        let timestamp = Utc::now().timestamp_millis();
        let random_suffix = rand::random::<u32>();
        let backup_id = format!("backup_{}_{}", timestamp, random_suffix);
        let backup_path = Path::new(&self.config.backup_dir).join(&backup_id);

        info!("Starting backup: {}", backup_id);
        info!("Backup type: {}", self.config.backup_type);
        info!("Source directory: {}", data_dir);
        info!("Backup directory: {:?}", backup_path);

        // 创建备份目录
        std::fs::create_dir_all(&backup_path)?;

        // 执行备份
        let (files_count, total_size) = match self.config.backup_type.as_str() {
            "full" => {
                self.perform_full_backup(data_dir, &backup_path).await?
            },
            "incremental" => {
                self.perform_incremental_backup(data_dir, &backup_path).await?
            },
            _ => {
                return Err(crate::error::Error::Internal(
                    format!("Unsupported backup type: {}", self.config.backup_type)
                ));
            }
        };

        // 创建备份元数据
        let metadata = BackupMetadata {
            backup_id: backup_id.clone(),
            backup_type: self.config.backup_type.clone(),
            timestamp: Utc::now(),
            source_dir: data_dir.to_string(),
            storage_backend: self.config.storage_backend.clone(),
            files_count,
            total_size,
            previous_backup_id: self.last_backup_id.clone(),
            verification_status: None,
        };

        // 保存备份元数据
        self.save_metadata(&backup_path, &metadata).await?;

        // 验证备份
        if self.config.enable_verification {
            let verification_status = self.verify_backup(&backup_path).await?;
            // 更新备份元数据
            let mut metadata = metadata;
            metadata.verification_status = Some(verification_status);
            self.save_metadata(&backup_path, &metadata).await?;
        }

        // 清理过期备份
        self.cleanup_old_backups().await?;

        // 更新最后备份 ID
        self.last_backup_id = Some(backup_id.clone());

        info!("Backup completed: {}", backup_id);
        info!("Files: {}, Size: {} bytes", files_count, total_size);

        Ok(backup_id)
    }

    /// 执行全量备份
    async fn perform_full_backup(&self, data_dir: &str, backup_path: &Path) -> Result<(u64, u64)> {
        info!("Performing full backup");

        let source = Path::new(data_dir);
        let mut files_count = 0;
        let mut total_size = 0;

        // 复制所有文件
        for entry in walkdir::WalkDir::new(source) {
            let entry: walkdir::DirEntry = entry?;
            let path = entry.path();

            if path.is_file() {
                let relative_path = path.strip_prefix(source)?;
                let dest_path = backup_path.join(relative_path);

                // 创建目标目录
                if let Some(parent) = dest_path.parent() {
                    std::fs::create_dir_all(parent)?;
                }

                // 复制文件
                std::fs::copy(path, &dest_path)?;

                // 更新统计信息
                let metadata = entry.metadata()?;
                total_size += metadata.len();
                files_count += 1;
            }
        }

        Ok((files_count, total_size))
    }

    /// 执行增量备份
    async fn perform_incremental_backup(&self, data_dir: &str, backup_path: &Path) -> Result<(u64, u64)> {
        info!("Performing incremental backup");

        let source = Path::new(data_dir);
        let mut files_count = 0;
        let mut total_size = 0;

        // 获取上次备份时间
        let last_backup_time = if let Some(last_backup_id) = &self.last_backup_id {
            let last_backup_path = Path::new(&self.config.backup_dir).join(last_backup_id);
            let metadata_path = last_backup_path.join("backup.metadata");
            if metadata_path.exists() {
                let metadata_content = std::fs::read_to_string(metadata_path)?;
                let metadata: BackupMetadata = serde_json::from_str(&metadata_content)?;
                Some(metadata.timestamp)
            } else {
                None
            }
        } else {
            None
        };

        // 复制修改过的文件
        for entry in walkdir::WalkDir::new(source) {
            let entry: walkdir::DirEntry = entry?;
            let path = entry.path();

            if path.is_file() {
                // 检查文件是否在最后备份后修改过
                let modified_time = entry.metadata()?.modified()?;
                let modified_datetime: DateTime<Utc> = modified_time.into();

                if last_backup_time.as_ref().map(|t| modified_datetime > *t).unwrap_or(true) {
                    let relative_path = path.strip_prefix(source)?;
                    let dest_path = backup_path.join(relative_path);

                    // 创建目标目录
                    if let Some(parent) = dest_path.parent() {
                        std::fs::create_dir_all(parent)?;
                    }

                    // 复制文件
                    std::fs::copy(path, &dest_path)?;

                    // 更新统计信息
                    let metadata = entry.metadata()?;
                    total_size += metadata.len();
                    files_count += 1;
                }
            }
        }

        Ok((files_count, total_size))
    }

    /// 验证备份
    async fn verify_backup(&self, backup_path: &Path) -> Result<VerificationStatus> {
        info!("Verifying backup: {:?}", backup_path);

        let mut errors = Vec::new();
        let metadata_path = backup_path.join("backup.metadata");

        // 检查备份元数据文件
        if !metadata_path.exists() {
            errors.push("Backup metadata file not found".to_string());
        }

        // 检查文件完整性
        let mut files_checked = 0;
        for entry in walkdir::WalkDir::new(backup_path) {
            let entry: walkdir::DirEntry = entry?;
            let path = entry.path();

            if path.is_file() && path != metadata_path {
                // 检查文件是否可读
                match std::fs::File::open(path) {
                    Ok(_) => files_checked += 1,
                    Err(e) => {
                        errors.push(format!("Cannot read file {:?}: {}", path, e));
                    }
                }
            }
        }
        
        // 如果备份中没有文件（只有元数据），也认为是成功的
        // 因为这可能是一个空数据目录的备份

        let success = errors.is_empty();
        let error_count = errors.len();
        let error_clone = errors.clone();
        let verification_status = VerificationStatus {
            verified_at: Utc::now(),
            success,
            errors,
        };

        if success {
            info!("Backup verification successful");
        } else {
            warn!("Backup verification failed with {} errors", error_count);
            for error in &error_clone {
                warn!("  - {}", error);
            }
        }

        Ok(verification_status)
    }

    /// 保存备份元数据
    async fn save_metadata(&self, backup_path: &Path, metadata: &BackupMetadata) -> Result<()> {
        let metadata_path = backup_path.join("backup.metadata");
        let metadata_content = serde_json::to_string_pretty(metadata)?;
        std::fs::write(metadata_path, metadata_content)?;
        Ok(())
    }

    /// 清理过期备份
    async fn cleanup_old_backups(&self) -> Result<()> {
        info!("Cleaning up old backups");

        let backup_dir = Path::new(&self.config.backup_dir);
        let cutoff_time = Utc::now() - chrono::Duration::days(self.config.retention_days as i64);

        let mut removed_count = 0;
        let mut removed_size = 0;

        // 遍历备份目录
        for entry in std::fs::read_dir(backup_dir)? {
            let entry: std::fs::DirEntry = entry?;
            let path = entry.path();

            if path.is_dir() {
                // 检查备份元数据
                let metadata_path = path.join("backup.metadata");
                if metadata_path.exists() {
                    let metadata_content = std::fs::read_to_string(metadata_path)?;
                    let metadata: BackupMetadata = serde_json::from_str(&metadata_content)?;

                    // 检查是否过期
                    if metadata.timestamp < cutoff_time {
                        // 计算目录大小
                        let size = self.calculate_dir_size(&path)?;
                        removed_size += size;

                        // 删除备份目录
                        std::fs::remove_dir_all(&path)?;
                        removed_count += 1;

                        info!("Removed expired backup: {:?} ({} bytes)", path, size);
                    }
                }
            }
        }

        info!("Cleanup completed: removed {} backups ({} bytes)", removed_count, removed_size);
        Ok(())
    }

    /// 计算目录大小
    fn calculate_dir_size(&self, dir: &Path) -> Result<u64> {
        let mut size = 0;
        for entry in walkdir::WalkDir::new(dir) {
            let entry: walkdir::DirEntry = entry?;
            if entry.path().is_file() {
                size += entry.metadata()?.len();
            }
        }
        Ok(size)
    }

    /// 恢复备份
    pub async fn restore_backup(&self, backup_id: &str, data_dir: &str) -> Result<()> {
        info!("Restoring backup: {}", backup_id);
        info!("Target directory: {}", data_dir);

        let backup_path = Path::new(&self.config.backup_dir).join(backup_id);

        // 检查备份是否存在
        if !backup_path.exists() {
            return Err(crate::error::Error::Internal(
                format!("Backup not found: {}", backup_id)
            ));
        }

        // 检查备份元数据
        let metadata_path = backup_path.join("backup.metadata");
        if !metadata_path.exists() {
            return Err(crate::error::Error::Internal(
                format!("Backup metadata not found for: {}", backup_id)
            ));
        }

        // 读取备份元数据
        let metadata_content = std::fs::read_to_string(&metadata_path)?;
        let metadata: BackupMetadata = serde_json::from_str(&metadata_content)?;

        // 确认恢复操作
        if Path::new(data_dir).exists() {
            warn!("Target directory already exists: {}", data_dir);
            // 创建备份
            let backup_timestamp = Utc::now().timestamp();
            let existing_backup = format!("{}.backup.{}", data_dir, backup_timestamp);
            info!("Backing up existing data to: {}", existing_backup);
            std::fs::rename(data_dir, existing_backup)?;
        }

        // 创建目标目录
        std::fs::create_dir_all(data_dir)?;

        // 复制备份文件
        let mut files_restored = 0;
        let mut bytes_restored = 0;

        for entry in walkdir::WalkDir::new(&backup_path) {
            let entry: walkdir::DirEntry = entry?;
            let path = entry.path();

            if path.is_file() && path != metadata_path {
                let relative_path = path.strip_prefix(&backup_path)?;
                let dest_path = Path::new(data_dir).join(relative_path);

                // 创建目标目录
                if let Some(parent) = dest_path.parent() {
                    std::fs::create_dir_all(parent)?;
                }

                // 复制文件
                std::fs::copy(path, &dest_path)?;

                // 更新统计信息
                let file_size = entry.metadata()?.len();
                bytes_restored += file_size;
                files_restored += 1;
            }
        }

        info!("Restore completed:");
        info!("  Backup ID: {}", backup_id);
        info!("  Files restored: {}", files_restored);
        info!("  Bytes restored: {}", bytes_restored);
        info!("  Backup type: {}", metadata.backup_type);

        Ok(())
    }

    /// 列出所有备份
    pub async fn list_backups(&self) -> Result<Vec<BackupMetadata>> {
        let backup_dir = Path::new(&self.config.backup_dir);
        let mut backups = Vec::new();

        // 遍历备份目录
        for entry in std::fs::read_dir(backup_dir)? {
            let entry: std::fs::DirEntry = entry?;
            let path = entry.path();

            if path.is_dir() {
                // 检查备份元数据
                let metadata_path = path.join("backup.metadata");
                if metadata_path.exists() {
                    let metadata_content = std::fs::read_to_string(metadata_path)?;
                    if let Ok(metadata) = serde_json::from_str::<BackupMetadata>(&metadata_content) {
                        backups.push(metadata);
                    }
                }
            }
        }

        // 按时间排序（最新的在前）
        backups.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

        Ok(backups)
    }

    /// 启动定期备份任务
    pub async fn start_backup_task(&mut self, data_dir: &str) -> Result<tokio::task::JoinHandle<()>> {
        let config = self.config.clone();
        let data_dir = data_dir.to_string();
        let mut manager = self.clone();

        let handle = tokio::spawn(async move {
            let interval = Duration::from_secs(config.backup_interval_secs);
            let mut interval_timer = tokio::time::interval(interval);

            info!("Started backup task with interval: {:?}", interval);

            loop {
                interval_timer.tick().await;

                info!("Running scheduled backup");
                match manager.perform_backup(&data_dir).await {
                    Ok(backup_id) => {
                        info!("Scheduled backup completed: {}", backup_id);
                    },
                    Err(e) => {
                        warn!("Scheduled backup failed: {:?}", e);
                    }
                }
            }
        });

        Ok(handle)
    }
}
