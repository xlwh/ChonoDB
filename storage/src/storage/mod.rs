pub mod backend;
pub mod s3;
pub mod gcs;
pub mod local;

pub use backend::{StorageBackend, StorageOptions, ObjectMetadata, StorageConfig, StorageObject, ListOptions};
pub use s3::S3Storage;
pub use gcs::{GcsStorage, GcsConfig};
pub use local::LocalStorage;

use crate::error::Result;
use bytes::Bytes;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::info;

/// 存储后端类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendType {
    Local,
    S3,
    Gcs,
    Azure,
    Minio,
}

impl BackendType {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "local" => Some(BackendType::Local),
            "s3" => Some(BackendType::S3),
            "gcs" | "google" => Some(BackendType::Gcs),
            "azure" => Some(BackendType::Azure),
            "minio" => Some(BackendType::Minio),
            _ => None,
        }
    }
}

/// 统一存储接口
pub struct ObjectStorage {
    backend: Arc<dyn StorageBackend>,
    backend_type: BackendType,
    options: StorageOptions,
}

impl ObjectStorage {
    /// 创建新的存储实例
    pub async fn new(backend_type: BackendType, options: StorageOptions) -> Result<Self> {
        let backend: Arc<dyn StorageBackend> = match backend_type {
            BackendType::Local => Arc::new(LocalStorage::new(options.clone()).await?),
            BackendType::S3 => Arc::new(S3Storage::new(options.clone()).await?),
            BackendType::Gcs => {
                let gcs_config = GcsConfig::default();
                Arc::new(GcsStorage::new(gcs_config).await?)
            }
            BackendType::Azure => {
                return Err(crate::error::Error::NotImplemented(
                    "Azure storage not yet implemented".to_string()
                ));
            }
            BackendType::Minio => {
                // MinIO 使用 S3 兼容接口
                Arc::new(S3Storage::new(options.clone()).await?)
            }
        };

        info!("Object storage initialized: {:?}", backend_type);
        
        Ok(Self { 
            backend, 
            backend_type, 
            options 
        })
    }

    /// 获取后端类型
    pub fn backend_type(&self) -> BackendType {
        self.backend_type
    }

    /// 获取存储选项
    pub fn options(&self) -> &StorageOptions {
        &self.options
    }

    /// 上传对象
    pub async fn put(&self, key: &str, data: Bytes) -> Result<()> {
        self.backend.put(key, data).await
    }

    /// 下载对象
    pub async fn get(&self, key: &str) -> Result<Bytes> {
        self.backend.get(key).await
    }

    /// 删除对象
    pub async fn delete(&self, key: &str) -> Result<()> {
        self.backend.delete(key).await
    }

    /// 检查对象是否存在
    pub async fn exists(&self, key: &str) -> Result<bool> {
        self.backend.exists(key).await
    }

    /// 列出对象
    pub async fn list(&self, prefix: &str) -> Result<Vec<String>> {
        self.backend.list(prefix).await
    }

    /// 获取对象元数据
    pub async fn metadata(&self, key: &str) -> Result<ObjectMetadata> {
        self.backend.metadata(key).await
    }

    /// 批量上传
    pub async fn batch_put(&self, objects: HashMap<String, Bytes>) -> Result<()> {
        // 并行上传
        let mut tasks = Vec::new();
        for (key, data) in objects {
            let backend = self.backend.clone();
            let key = key.clone();
            tasks.push(tokio::spawn(async move {
                backend.put(&key, data).await
            }));
        }

        for task in tasks {
            task.await??;
        }

        Ok(())
    }

    /// 批量下载
    pub async fn batch_get(&self, keys: &[String]) -> Result<HashMap<String, Bytes>> {
        // 并行下载
        let mut tasks: Vec<tokio::task::JoinHandle<Result<(String, Bytes)>>> = Vec::new();
        for key in keys {
            let backend = self.backend.clone();
            let key = key.clone();
            tasks.push(tokio::spawn(async move {
                let data = backend.get(&key).await?;
                Ok((key, data))
            }));
        }

        let mut results = HashMap::new();
        for task in tasks {
            let (key, data) = task.await??;
            results.insert(key, data);
        }

        Ok(results)
    }

    /// 批量删除
    pub async fn batch_delete(&self, keys: &[String]) -> Result<()> {
        // 并行删除
        let mut tasks: Vec<tokio::task::JoinHandle<Result<()>>> = Vec::new();
        for key in keys {
            let backend = self.backend.clone();
            let key = key.clone();
            tasks.push(tokio::spawn(async move {
                backend.delete(&key).await
            }));
        }

        for task in tasks {
            task.await??;
        }

        Ok(())
    }

    /// 上传大对象（分块上传）
    pub async fn put_large_object(&self, key: &str, data: Bytes, chunk_size: usize) -> Result<()> {
        // 这里可以实现分块上传逻辑
        // 简化实现，直接调用 put
        self.put(key, data).await
    }

    /// 下载大对象（分块下载）
    pub async fn get_large_object(&self, key: &str, chunk_size: usize) -> Result<Bytes> {
        // 这里可以实现分块下载逻辑
        // 简化实现，直接调用 get
        self.get(key).await
    }

    /// 复制对象
    pub async fn copy_object(&self, source_key: &str, destination_key: &str) -> Result<()> {
        // 先下载再上传
        let data = self.get(source_key).await?;
        self.put(destination_key, data).await
    }

    /// 移动对象
    pub async fn move_object(&self, source_key: &str, destination_key: &str) -> Result<()> {
        // 先复制再删除
        self.copy_object(source_key, destination_key).await?;
        self.delete(source_key).await
    }

    /// 获取存储使用情况
    pub async fn get_usage(&self) -> Result<(u64, u64)> {
        // 这里可以实现获取存储使用情况的逻辑
        // 简化实现，返回 0
        Ok((0, 0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backend_type_from_str() {
        assert_eq!(BackendType::from_str("local"), Some(BackendType::Local));
        assert_eq!(BackendType::from_str("S3"), Some(BackendType::S3));
        assert_eq!(BackendType::from_str("gcs"), Some(BackendType::Gcs));
        assert_eq!(BackendType::from_str("minio"), Some(BackendType::Minio));
        assert_eq!(BackendType::from_str("unknown"), None);
    }
}
