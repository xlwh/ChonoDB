use crate::error::{Error, Result};
use crate::storage::{StorageBackend, ObjectMetadata};
use async_trait::async_trait;
use bytes::Bytes;
use google_cloud_storage::client::{Client, ClientConfig};
use google_cloud_storage::http::objects::download::Range;
use google_cloud_storage::http::objects::get::GetObjectRequest;
use google_cloud_storage::http::objects::upload::{UploadObjectRequest, UploadType};
use google_cloud_storage::http::objects::delete::DeleteObjectRequest;
use google_cloud_storage::http::objects::list::ListObjectsRequest;
use std::collections::HashMap;
use std::path::Path;
use tracing::{debug, info};

/// GCS对象存储配置
#[derive(Debug, Clone)]
pub struct GcsConfig {
    /// 项目ID
    pub project_id: String,
    /// 存储桶名称
    pub bucket: String,
    /// 前缀路径
    pub prefix: String,
    /// 区域
    pub location: String,
    /// 服务账号密钥路径（可选，使用默认认证时可不设置）
    pub credentials_path: Option<String>,
}

impl Default for GcsConfig {
    fn default() -> Self {
        Self {
            project_id: "my-project".to_string(),
            bucket: "chronodb-data".to_string(),
            prefix: "data".to_string(),
            location: "us-central1".to_string(),
            credentials_path: None,
        }
    }
}

/// GCS对象存储实现
pub struct GcsStorage {
    client: Client,
    config: GcsConfig,
}

impl GcsStorage {
    /// 创建新的GCS存储实例
    pub async fn new(config: GcsConfig) -> Result<Self> {
        let client_config = ClientConfig::default()
            .with_auth()
            .await
            .map_err(|e| Error::Internal(format!("Failed to create GCS client config: {}", e)))?;

        let client = Client::new(client_config);

        info!(
            "Created GCS storage client for bucket: {}",
            config.bucket
        );

        Ok(Self { client, config })
    }

    /// 使用服务账号密钥创建
    pub async fn with_credentials(config: GcsConfig, _credentials_path: &Path) -> Result<Self> {
        // 简化实现，使用默认认证
        let client_config = ClientConfig::default()
            .with_auth()
            .await
            .map_err(|e| Error::Internal(format!("Failed to load GCS credentials: {}", e)))?;

        let client = Client::new(client_config);

        info!(
            "Created GCS storage client with credentials for bucket: {}",
            config.bucket
        );

        Ok(Self { client, config })
    }

    fn build_key(&self, path: &str) -> String {
        if self.config.prefix.is_empty() {
            path.to_string()
        } else {
            format!("{}/{}", self.config.prefix, path)
        }
    }
}

#[async_trait]
impl StorageBackend for GcsStorage {
    async fn put(&self, path: &str, data: Bytes) -> Result<()> {
        let key = self.build_key(path);
        
        debug!("Uploading object to GCS: {}", key);

        let upload_type = UploadType::Simple(
            google_cloud_storage::http::objects::upload::Media::new(key.clone())
        );

        let request = UploadObjectRequest {
            bucket: self.config.bucket.clone(),
            ..Default::default()
        };

        self.client
            .upload_object(&request, data.to_vec(), &upload_type)
            .await
            .map_err(|e| Error::Internal(format!("Failed to upload object to GCS: {}", e)))?;

        debug!("Successfully uploaded object to GCS: {}", key);
        Ok(())
    }

    async fn get(&self, path: &str) -> Result<Bytes> {
        let key = self.build_key(path);
        
        debug!("Downloading object from GCS: {}", key);

        let request = GetObjectRequest {
            bucket: self.config.bucket.clone(),
            object: key.clone(),
            ..Default::default()
        };

        let data = self.client
            .download_object(&request, &Range::default())
            .await
            .map_err(|e| Error::Internal(format!("Failed to download object from GCS: {}", e)))?;

        debug!("Successfully downloaded object from GCS: {} ({} bytes)", key, data.len());
        Ok(Bytes::from(data))
    }

    async fn delete(&self, path: &str) -> Result<()> {
        let key = self.build_key(path);
        
        debug!("Deleting object from GCS: {}", key);

        let request = DeleteObjectRequest {
            bucket: self.config.bucket.clone(),
            object: key.clone(),
            ..Default::default()
        };

        self.client
            .delete_object(&request)
            .await
            .map_err(|e| Error::Internal(format!("Failed to delete object from GCS: {}", e)))?;

        debug!("Successfully deleted object from GCS: {}", key);
        Ok(())
    }

    async fn exists(&self, path: &str) -> Result<bool> {
        let key = self.build_key(path);
        
        debug!("Checking object existence in GCS: {}", key);

        let request = GetObjectRequest {
            bucket: self.config.bucket.clone(),
            object: key.clone(),
            ..Default::default()
        };

        match self.client.get_object(&request).await {
            Ok(_) => Ok(true),
            Err(e) => {
                if e.to_string().contains("404") {
                    Ok(false)
                } else {
                    Err(Error::Internal(format!("Failed to check object existence: {}", e)))
                }
            }
        }
    }

    async fn list(&self, prefix: &str) -> Result<Vec<String>> {
        let full_prefix = self.build_key(prefix);
        
        debug!("Listing objects in GCS with prefix: {}", full_prefix);

        let request = ListObjectsRequest {
            bucket: self.config.bucket.clone(),
            prefix: Some(full_prefix),
            ..Default::default()
        };

        let response = self.client
            .list_objects(&request)
            .await
            .map_err(|e| Error::Internal(format!("Failed to list objects in GCS: {}", e)))?;

        let objects: Vec<String> = response
            .items
            .unwrap_or_default()
            .into_iter()
            .map(|obj| obj.name)
            .collect();

        debug!("Listed {} objects from GCS", objects.len());
        Ok(objects)
    }

    async fn metadata(&self, path: &str) -> Result<ObjectMetadata> {
        let key = self.build_key(path);
        
        debug!("Getting metadata for GCS object: {}", key);

        let request = GetObjectRequest {
            bucket: self.config.bucket.clone(),
            object: key.clone(),
            ..Default::default()
        };

        let object = self.client
            .get_object(&request)
            .await
            .map_err(|e| Error::Internal(format!("Failed to get object metadata from GCS: {}", e)))?;

        Ok(ObjectMetadata {
            key: object.name,
            size: object.size as u64,
            last_modified: object.updated.map(|t| t.unix_timestamp() * 1000).unwrap_or(0),
            etag: Some(object.etag),
            content_type: object.content_type,
            metadata: HashMap::new(),
        })
    }
}

/// GCS存储统计信息
#[derive(Debug, Clone, Default)]
pub struct GcsStats {
    pub uploads_total: u64,
    pub downloads_total: u64,
    pub deletes_total: u64,
    pub bytes_uploaded: u64,
    pub bytes_downloaded: u64,
    pub errors_total: u64,
}

/// GCS存储监控
pub struct GcsMonitor {
    stats: GcsStats,
}

impl GcsMonitor {
    pub fn new() -> Self {
        Self {
            stats: GcsStats::default(),
        }
    }

    pub fn record_upload(&mut self, bytes: u64) {
        self.stats.uploads_total += 1;
        self.stats.bytes_uploaded += bytes;
    }

    pub fn record_download(&mut self, bytes: u64) {
        self.stats.downloads_total += 1;
        self.stats.bytes_downloaded += bytes;
    }

    pub fn record_delete(&mut self) {
        self.stats.deletes_total += 1;
    }

    pub fn record_error(&mut self) {
        self.stats.errors_total += 1;
    }

    pub fn get_stats(&self) -> &GcsStats {
        &self.stats
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gcs_config_default() {
        let config = GcsConfig::default();
        assert_eq!(config.project_id, "my-project");
        assert_eq!(config.bucket, "chronodb-data");
        assert_eq!(config.prefix, "data");
        assert_eq!(config.location, "us-central1");
    }

    #[test]
    fn test_build_key() {
        let config = GcsConfig::default();
        let storage = GcsStorage {
            client: Client::new(ClientConfig::default()),
            config,
        };
        
        assert_eq!(storage.build_key("test/block-1"), "data/test/block-1");
    }

    #[test]
    fn test_gcs_stats() {
        let mut monitor = GcsMonitor::new();
        monitor.record_upload(1024);
        monitor.record_download(512);
        monitor.record_delete();
        monitor.record_error();

        let stats = monitor.get_stats();
        assert_eq!(stats.uploads_total, 1);
        assert_eq!(stats.downloads_total, 1);
        assert_eq!(stats.deletes_total, 1);
        assert_eq!(stats.errors_total, 1);
        assert_eq!(stats.bytes_uploaded, 1024);
        assert_eq!(stats.bytes_downloaded, 512);
    }
}
