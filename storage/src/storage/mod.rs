pub mod backend;
pub mod s3;
pub mod gcs;
pub mod local;

pub use backend::{StorageBackend, StorageOptions, ObjectMetadata, StorageConfig, StorageObject, ListOptions};
pub use s3::S3Storage;
pub use gcs::{GcsStorage, GcsConfig};
pub use local::LocalStorage;

use crate::error::Result;
use async_trait::async_trait;
use bytes::Bytes;
use std::collections::HashMap;
use std::path::Path;
use tracing::{info, debug};

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
    backend: Box<dyn StorageBackend>,
}

impl ObjectStorage {
    /// 创建新的存储实例
    pub async fn new(backend_type: BackendType, options: StorageOptions) -> Result<Self> {
        let backend: Box<dyn StorageBackend> = match backend_type {
            BackendType::Local => Box::new(LocalStorage::new(options).await?),
            BackendType::S3 => Box::new(S3Storage::new(options).await?),
            BackendType::Gcs => {
                let gcs_config = GcsConfig::default();
                Box::new(GcsStorage::new(gcs_config).await?)
            }
            BackendType::Azure => {
                return Err(crate::error::Error::NotImplemented(
                    "Azure storage not yet implemented".to_string()
                ));
            }
            BackendType::Minio => {
                // MinIO 使用 S3 兼容接口
                Box::new(S3Storage::new(options).await?)
            }
        };

        info!("Object storage initialized: {:?}", backend_type);
        
        Ok(Self { backend })
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
        for (key, data) in objects {
            self.put(&key, data).await?;
        }
        Ok(())
    }

    /// 批量下载
    pub async fn batch_get(&self, keys: &[String]) -> Result<HashMap<String, Bytes>> {
        let mut results = HashMap::new();
        for key in keys {
            let data = self.get(key).await?;
            results.insert(key.clone(), data);
        }
        Ok(results)
    }

    /// 批量删除
    pub async fn batch_delete(&self, keys: &[String]) -> Result<()> {
        for key in keys {
            self.delete(key).await?;
        }
        Ok(())
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
