use crate::error::{Result, Error};
use crate::storage::backend::{StorageBackend, StorageOptions, ObjectMetadata};
use async_trait::async_trait;
use aws_config::Region;
use aws_credential_types::Credentials;
use aws_sdk_s3::Client;
use aws_sdk_s3::config::{Builder, SharedCredentialsProvider};
use bytes::Bytes;
use std::collections::HashMap;
use tracing::{info, debug};

/// S3 存储后端
pub struct S3Storage {
    client: Client,
    bucket: String,
}

impl S3Storage {
    pub async fn new(options: StorageOptions) -> Result<Self> {
        let region = Region::new(options.region.clone());
        
        // 构建 S3 客户端配置
        let mut config_builder = Builder::new()
            .region(region);
        
        // 配置凭证
        if let (Some(access_key), Some(secret_key)) = (&options.access_key, &options.secret_key) {
            let credentials = Credentials::new(
                access_key,
                secret_key,
                None,
                None,
                "manual",
            );
            config_builder = config_builder.credentials_provider(SharedCredentialsProvider::new(credentials));
        }
        
        // 配置端点（用于 MinIO 等兼容服务）
        if let Some(endpoint) = &options.endpoint {
            config_builder = config_builder.endpoint_url(endpoint);
            config_builder = config_builder.force_path_style(true);
        }
        
        let config = config_builder.build();
        let client = Client::from_conf(config);
        
        // 检查 bucket 是否存在，不存在则创建
        let bucket = options.bucket.clone();
        Self::ensure_bucket(&client, &bucket).await?;
        
        info!("S3 storage initialized: bucket={}, region={}", bucket, options.region);
        
        Ok(Self { client, bucket })
    }

    async fn ensure_bucket(client: &Client, bucket: &str) -> Result<()> {
        // 检查 bucket 是否存在
        let list_result = client.list_buckets().send().await
            .map_err(|e| Error::Storage(format!("Failed to list buckets: {}", e)))?;
        
        let exists = list_result.buckets()
            .iter()
            .any(|b| b.name().map(|n| n == bucket).unwrap_or(false));
        
        if !exists {
            debug!("Creating S3 bucket: {}", bucket);
            client.create_bucket()
                .bucket(bucket)
                .send()
                .await
                .map_err(|e| Error::Storage(format!("Failed to create bucket: {}", e)))?;
        }
        
        Ok(())
    }

    fn sanitize_key(key: &str) -> String {
        // 移除开头的斜杠
        key.trim_start_matches('/').to_string()
    }
}

#[async_trait]
impl StorageBackend for S3Storage {
    async fn put(&self, key: &str, data: Bytes) -> Result<()> {
        let key = Self::sanitize_key(key);
        
        debug!("S3 put object: {}/{}", self.bucket, key);
        
        self.client.put_object()
            .bucket(&self.bucket)
            .key(&key)
            .body(data.into())
            .send()
            .await
            .map_err(|e| Error::Storage(format!("Failed to put object: {}", e)))?;
        
        Ok(())
    }

    async fn get(&self, key: &str) -> Result<Bytes> {
        let key = Self::sanitize_key(key);
        
        debug!("S3 get object: {}/{}", self.bucket, key);
        
        let result = self.client.get_object()
            .bucket(&self.bucket)
            .key(&key)
            .send()
            .await
            .map_err(|e| {
                if e.to_string().contains("NoSuchKey") {
                    Error::NotFound(format!("Object not found: {}", key))
                } else {
                    Error::Storage(format!("Failed to get object: {}", e))
                }
            })?;
        
        let data = result.body.collect().await
            .map_err(|e| Error::Storage(format!("Failed to collect body: {}", e)))?;
        
        Ok(data.into_bytes())
    }

    async fn delete(&self, key: &str) -> Result<()> {
        let key = Self::sanitize_key(key);
        
        debug!("S3 delete object: {}/{}", self.bucket, key);
        
        self.client.delete_object()
            .bucket(&self.bucket)
            .key(&key)
            .send()
            .await
            .map_err(|e| Error::Storage(format!("Failed to delete object: {}", e)))?;
        
        Ok(())
    }

    async fn exists(&self, key: &str) -> Result<bool> {
        let key = Self::sanitize_key(key);
        
        debug!("S3 check exists: {}/{}", self.bucket, key);
        
        match self.client.head_object()
            .bucket(&self.bucket)
            .key(&key)
            .send()
            .await {
            Ok(_) => Ok(true),
            Err(e) => {
                if e.to_string().contains("NotFound") || e.to_string().contains("NoSuchKey") {
                    Ok(false)
                } else {
                    Err(Error::Storage(format!("Failed to check existence: {}", e)))
                }
            }
        }
    }

    async fn list(&self, prefix: &str) -> Result<Vec<String>> {
        let prefix = Self::sanitize_key(prefix);
        
        debug!("S3 list objects: {}/{}", self.bucket, prefix);
        
        let mut keys = Vec::new();
        let mut continuation_token = None;
        
        loop {
            let mut request = self.client.list_objects_v2()
                .bucket(&self.bucket)
                .prefix(&prefix);
            
            if let Some(token) = continuation_token {
                request = request.continuation_token(token);
            }
            
            let result = request.send().await
                .map_err(|e| Error::Storage(format!("Failed to list objects: {}", e)))?;
            
            for object in result.contents() {
                if let Some(key) = object.key() {
                    keys.push(key.to_string());
                }
            }
            
            if result.is_truncated.unwrap_or(false) {
                continuation_token = result.next_continuation_token.clone();
            } else {
                break;
            }
        }
        
        Ok(keys)
    }

    async fn metadata(&self, key: &str) -> Result<ObjectMetadata> {
        let key = Self::sanitize_key(key);
        
        debug!("S3 get metadata: {}/{}", self.bucket, key);
        
        let result = self.client.head_object()
            .bucket(&self.bucket)
            .key(&key)
            .send()
            .await
            .map_err(|e| {
                if e.to_string().contains("NotFound") || e.to_string().contains("NoSuchKey") {
                    Error::NotFound(format!("Object not found: {}", key))
                } else {
                    Error::Storage(format!("Failed to get metadata: {}", e))
                }
            })?;
        
        let last_modified = result.last_modified
            .map(|t| t.secs())
            .unwrap_or(0);
        
        let size = result.content_length.unwrap_or(0) as u64;
        
        let etag = result.e_tag.clone();
        let content_type = result.content_type.clone();
        
        // 获取自定义元数据
        let metadata: HashMap<String, String> = result.metadata.clone().unwrap_or_default();
        
        Ok(ObjectMetadata {
            key: key.clone(),
            size,
            last_modified,
            etag,
            content_type,
            metadata,
        })
    }
}

/// 创建 S3 存储实例的辅助函数
pub async fn create_s3_storage(
    bucket: &str,
    region: &str,
    access_key: Option<&str>,
    secret_key: Option<&str>,
    endpoint: Option<&str>,
) -> Result<S3Storage> {
    let mut options = StorageOptions::new(bucket, region);
    
    if let (Some(ak), Some(sk)) = (access_key, secret_key) {
        options = options.with_credentials(ak, sk);
    }
    
    if let Some(ep) = endpoint {
        options = options.with_endpoint(ep);
    }
    
    S3Storage::new(options).await
}

#[cfg(test)]
mod tests {
    use super::*;

    // 注意：这些测试需要实际的 S3 凭证或 MinIO 实例
    // 在生产环境中应该使用 mock 或本地 MinIO 进行测试
    
    #[tokio::test]
    #[ignore] // 默认忽略，需要配置 S3 凭证
    async fn test_s3_storage_basic() {
        // 从环境变量读取配置
        let bucket = std::env::var("S3_BUCKET").unwrap_or_else(|_| "chronodb-test".to_string());
        let region = std::env::var("S3_REGION").unwrap_or_else(|_| "us-east-1".to_string());
        let access_key = std::env::var("S3_ACCESS_KEY").ok();
        let secret_key = std::env::var("S3_SECRET_KEY").ok();
        let endpoint = std::env::var("S3_ENDPOINT").ok();
        
        let mut options = StorageOptions::new(&bucket, &region);
        
        if let (Some(ak), Some(sk)) = (&access_key, &secret_key) {
            options = options.with_credentials(ak, sk);
        }
        
        if let Some(ep) = &endpoint {
            options = options.with_endpoint(ep);
        }
        
        let storage = S3Storage::new(options).await.unwrap();
        
        // Test put and get
        let key = "test/key.txt";
        let data = Bytes::from("Hello, S3!");
        
        storage.put(key, data.clone()).await.unwrap();
        
        let retrieved = storage.get(key).await.unwrap();
        assert_eq!(retrieved, data);
        
        // Test exists
        assert!(storage.exists(key).await.unwrap());
        assert!(!storage.exists("nonexistent").await.unwrap());
        
        // Test metadata
        let metadata = storage.metadata(key).await.unwrap();
        assert_eq!(metadata.key, key);
        assert_eq!(metadata.size, data.len() as u64);
        
        // Test list
        let keys = storage.list("test/").await.unwrap();
        assert!(keys.contains(&key.to_string()));
        
        // Test delete
        storage.delete(key).await.unwrap();
        assert!(!storage.exists(key).await.unwrap());
    }
}
