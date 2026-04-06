use crate::error::{Result, Error};
use crate::storage::backend::{StorageBackend, StorageOptions, ObjectMetadata};
use async_trait::async_trait;
use bytes::Bytes;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::fs;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tracing::{info, debug};

/// 本地文件系统存储后端
pub struct LocalStorage {
    base_path: PathBuf,
    bucket: String,
}

impl LocalStorage {
    pub async fn new(options: StorageOptions) -> Result<Self> {
        let base_path = options.local_path
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/tmp/chronodb"));
        
        let bucket_path = base_path.join(&options.bucket);
        
        // 创建目录
        fs::create_dir_all(&bucket_path).await
            .map_err(|e| Error::Storage(format!("Failed to create directory: {}", e)))?;
        
        info!("Local storage initialized at: {:?}", bucket_path);
        
        Ok(Self {
            base_path,
            bucket: options.bucket,
        })
    }

    fn get_full_path(&self, key: &str) -> PathBuf {
        self.base_path.join(&self.bucket).join(key)
    }

    fn sanitize_key(key: &str) -> String {
        // 移除开头的斜杠
        key.trim_start_matches('/').to_string()
    }
}

#[async_trait]
impl StorageBackend for LocalStorage {
    async fn put(&self, key: &str, data: Bytes) -> Result<()> {
        let key = Self::sanitize_key(key);
        let path = self.get_full_path(&key);
        
        debug!("Put object: {:?}", path);
        
        // 确保父目录存在
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await
                .map_err(|e| Error::Storage(format!("Failed to create directory: {}", e)))?;
        }
        
        // 写入文件
        let mut file = fs::File::create(&path).await
            .map_err(|e| Error::Storage(format!("Failed to create file: {}", e)))?;
        
        file.write_all(&data).await
            .map_err(|e| Error::Storage(format!("Failed to write file: {}", e)))?;
        
        file.flush().await
            .map_err(|e| Error::Storage(format!("Failed to flush file: {}", e)))?;
        
        Ok(())
    }

    async fn get(&self, key: &str) -> Result<Bytes> {
        let key = Self::sanitize_key(key);
        let path = self.get_full_path(&key);
        
        debug!("Get object: {:?}", path);
        
        let mut file = fs::File::open(&path).await
            .map_err(|e| match e.kind() {
                std::io::ErrorKind::NotFound => Error::NotFound(format!("Object not found: {}", key)),
                _ => Error::Storage(format!("Failed to open file: {}", e)),
            })?;
        
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer).await
            .map_err(|e| Error::Storage(format!("Failed to read file: {}", e)))?;
        
        Ok(Bytes::from(buffer))
    }

    async fn delete(&self, key: &str) -> Result<()> {
        let key = Self::sanitize_key(key);
        let path = self.get_full_path(&key);
        
        debug!("Delete object: {:?}", path);
        
        fs::remove_file(&path).await
            .map_err(|e| match e.kind() {
                std::io::ErrorKind::NotFound => Error::NotFound(format!("Object not found: {}", key)),
                _ => Error::Storage(format!("Failed to delete file: {}", e)),
            })?;
        
        Ok(())
    }

    async fn exists(&self, key: &str) -> Result<bool> {
        let key = Self::sanitize_key(key);
        let path = self.get_full_path(&key);
        
        Ok(path.exists())
    }

    async fn list(&self, prefix: &str) -> Result<Vec<String>> {
        let prefix = Self::sanitize_key(prefix);
        let bucket_path = self.base_path.join(&self.bucket);
        
        debug!("List objects with prefix: {}", prefix);
        
        let mut keys = Vec::new();
        
        if !bucket_path.exists() {
            return Ok(keys);
        }
        
        let prefix_path = bucket_path.join(&prefix);
        let _prefix_str = prefix_path.to_string_lossy();
        
        let mut entries = fs::read_dir(&bucket_path).await
            .map_err(|e| Error::Storage(format!("Failed to read directory: {}", e)))?;
        
        while let Some(entry) = entries.next_entry().await
            .map_err(|e| Error::Storage(format!("Failed to read entry: {}", e)))? {
            
            let path = entry.path();
            let metadata = entry.metadata().await
                .map_err(|e| Error::Storage(format!("Failed to get metadata: {}", e)))?;
            
            if metadata.is_file() {
                let relative_path = path.strip_prefix(&bucket_path)
                    .map_err(|e| Error::Storage(format!("Failed to get relative path: {}", e)))?;
                let key = relative_path.to_string_lossy().replace('\\', "/");
                
                if key.starts_with(&prefix) {
                    keys.push(key);
                }
            } else if metadata.is_dir() {
                // 递归遍历子目录
                Self::list_recursive(&path, &bucket_path, &prefix, &mut keys).await?;
            }
        }
        
        Ok(keys)
    }

    async fn metadata(&self, key: &str) -> Result<ObjectMetadata> {
        let key = Self::sanitize_key(key);
        let path = self.get_full_path(&key);
        
        debug!("Get metadata: {:?}", path);
        
        let metadata = fs::metadata(&path).await
            .map_err(|e| match e.kind() {
                std::io::ErrorKind::NotFound => Error::NotFound(format!("Object not found: {}", key)),
                _ => Error::Storage(format!("Failed to get metadata: {}", e)),
            })?;
        
        let modified = metadata.modified()
            .map_err(|e| Error::Storage(format!("Failed to get modified time: {}", e)))?;
        
        let last_modified = modified.duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| Error::Storage(format!("Failed to convert time: {}", e)))?
            .as_secs() as i64;
        
        Ok(ObjectMetadata {
            key: key.clone(),
            size: metadata.len(),
            last_modified,
            etag: None,
            content_type: None,
            metadata: HashMap::new(),
        })
    }
}

impl LocalStorage {
    async fn list_recursive(
        dir: &Path,
        base_path: &Path,
        prefix: &str,
        keys: &mut Vec<String>,
    ) -> Result<()> {
        let mut entries = fs::read_dir(dir).await
            .map_err(|e| Error::Storage(format!("Failed to read directory: {}", e)))?;
        
        while let Some(entry) = entries.next_entry().await
            .map_err(|e| Error::Storage(format!("Failed to read entry: {}", e)))? {
            
            let path = entry.path();
            let metadata = entry.metadata().await
                .map_err(|e| Error::Storage(format!("Failed to get metadata: {}", e)))?;
            
            if metadata.is_file() {
                let relative_path = path.strip_prefix(base_path)
                    .map_err(|e| Error::Storage(format!("Failed to get relative path: {}", e)))?;
                let key = relative_path.to_string_lossy().replace('\\', "/");
                
                if key.starts_with(prefix) {
                    keys.push(key);
                }
            } else if metadata.is_dir() {
                Box::pin(Self::list_recursive(&path, base_path, prefix, keys)).await?;
            }
        }
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_local_storage_basic() {
        let temp_dir = TempDir::new().unwrap();
        let options = StorageOptions::default()
            .with_local_path(temp_dir.path().to_str().unwrap());
        
        let storage = LocalStorage::new(options).await.unwrap();
        
        // Test put and get
        let key = "test/key.txt";
        let data = Bytes::from("Hello, World!");
        
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

    #[tokio::test]
    async fn test_local_storage_not_found() {
        let temp_dir = TempDir::new().unwrap();
        let options = StorageOptions::default()
            .with_local_path(temp_dir.path().to_str().unwrap());
        
        let storage = LocalStorage::new(options).await.unwrap();
        
        let result = storage.get("nonexistent").await;
        assert!(matches!(result, Err(Error::NotFound(_))));
    }
}
