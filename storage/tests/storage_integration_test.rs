use bytes::Bytes;
use std::collections::HashMap;
use tempfile::TempDir;

use chronodb_storage::storage::{
    ObjectStorage, BackendType, StorageOptions,
    LocalStorage, StorageBackend, ObjectMetadata
};

#[tokio::test]
async fn test_local_storage_integration() {
    let temp_dir = TempDir::new().unwrap();
    let options = StorageOptions::default()
        .with_local_path(temp_dir.path().to_str().unwrap());
    
    let storage = ObjectStorage::new(BackendType::Local, options).await.unwrap();
    
    // Test single object operations
    let key1 = "metrics/cpu/2024/01/01/data.bin";
    let data1 = Bytes::from(vec![1u8, 2, 3, 4, 5]);
    
    // Put
    storage.put(key1, data1.clone()).await.unwrap();
    
    // Get
    let retrieved = storage.get(key1).await.unwrap();
    assert_eq!(retrieved, data1);
    
    // Exists
    assert!(storage.exists(key1).await.unwrap());
    
    // Metadata
    let metadata = storage.metadata(key1).await.unwrap();
    assert_eq!(metadata.key, key1);
    assert_eq!(metadata.size, data1.len() as u64);
    
    // List
    let keys = storage.list("metrics/").await.unwrap();
    assert!(keys.contains(&key1.to_string()));
    
    // Delete
    storage.delete(key1).await.unwrap();
    assert!(!storage.exists(key1).await.unwrap());
}

#[tokio::test]
async fn test_batch_operations() {
    let temp_dir = TempDir::new().unwrap();
    let options = StorageOptions::default()
        .with_local_path(temp_dir.path().to_str().unwrap());
    
    let storage = ObjectStorage::new(BackendType::Local, options).await.unwrap();
    
    // Batch put
    let mut objects = HashMap::new();
    for i in 0..10 {
        let key = format!("batch/key_{}.txt", i);
        let data = Bytes::from(format!("data_{}", i));
        objects.insert(key, data);
    }
    
    storage.batch_put(objects.clone()).await.unwrap();
    
    // Verify all objects exist
    for key in objects.keys() {
        assert!(storage.exists(key).await.unwrap());
    }
    
    // Batch get
    let keys: Vec<String> = objects.keys().cloned().collect();
    let retrieved = storage.batch_get(&keys).await.unwrap();
    
    for (key, expected_data) in &objects {
        assert_eq!(retrieved.get(key).unwrap(), expected_data);
    }
    
    // Batch delete
    storage.batch_delete(&keys).await.unwrap();
    
    // Verify all objects deleted
    for key in &keys {
        assert!(!storage.exists(key).await.unwrap());
    }
}

#[tokio::test]
async fn test_nested_directories() {
    let temp_dir = TempDir::new().unwrap();
    let options = StorageOptions::default()
        .with_local_path(temp_dir.path().to_str().unwrap());
    
    let storage = ObjectStorage::new(BackendType::Local, options).await.unwrap();
    
    // Create nested structure
    let paths = vec![
        "2024/01/01/metrics.bin",
        "2024/01/02/metrics.bin",
        "2024/02/01/metrics.bin",
        "2024/02/15/metrics.bin",
        "2023/12/31/metrics.bin",
    ];
    
    for (i, path) in paths.iter().enumerate() {
        let data = Bytes::from(vec![i as u8; 100]);
        storage.put(path, data).await.unwrap();
    }
    
    // List by year
    let keys_2024 = storage.list("2024/").await.unwrap();
    assert_eq!(keys_2024.len(), 4);
    
    // List by month
    let keys_jan = storage.list("2024/01/").await.unwrap();
    assert_eq!(keys_jan.len(), 2);
    
    let keys_feb = storage.list("2024/02/").await.unwrap();
    assert_eq!(keys_feb.len(), 2);
    
    // List by specific date
    let keys_feb_15 = storage.list("2024/02/15/").await.unwrap();
    assert_eq!(keys_feb_15.len(), 1);
}

#[tokio::test]
async fn test_large_object() {
    let temp_dir = TempDir::new().unwrap();
    let options = StorageOptions::default()
        .with_local_path(temp_dir.path().to_str().unwrap());
    
    let storage = ObjectStorage::new(BackendType::Local, options).await.unwrap();
    
    // Create 1MB object
    let large_data = Bytes::from(vec![0u8; 1024 * 1024]);
    let key = "large_object.bin";
    
    storage.put(key, large_data.clone()).await.unwrap();
    
    let retrieved = storage.get(key).await.unwrap();
    assert_eq!(retrieved.len(), large_data.len());
    assert_eq!(retrieved, large_data);
    
    let metadata = storage.metadata(key).await.unwrap();
    assert_eq!(metadata.size, 1024 * 1024);
}

#[tokio::test]
async fn test_concurrent_access() {
    let temp_dir = TempDir::new().unwrap();
    let options = StorageOptions::default()
        .with_local_path(temp_dir.path().to_str().unwrap());
    
    let storage = std::sync::Arc::new(
        ObjectStorage::new(BackendType::Local, options).await.unwrap()
    );
    
    let mut handles = vec![];
    
    // Spawn multiple concurrent writers
    for i in 0..10 {
        let storage_clone = storage.clone();
        let handle = tokio::spawn(async move {
            let key = format!("concurrent/key_{}.txt", i);
            let data = Bytes::from(format!("data_from_task_{}", i));
            storage_clone.put(&key, data.clone()).await.unwrap();
            
            let retrieved = storage_clone.get(&key).await.unwrap();
            assert_eq!(retrieved, data);
        });
        handles.push(handle);
    }
    
    // Wait for all tasks
    for handle in handles {
        handle.await.unwrap();
    }
    
    // Verify all objects exist
    for i in 0..10 {
        let key = format!("concurrent/key_{}.txt", i);
        assert!(storage.exists(&key).await.unwrap());
    }
}

#[tokio::test]
async fn test_not_found_error() {
    let temp_dir = TempDir::new().unwrap();
    let options = StorageOptions::default()
        .with_local_path(temp_dir.path().to_str().unwrap());
    
    let storage = ObjectStorage::new(BackendType::Local, options).await.unwrap();
    
    // Try to get non-existent object
    let result = storage.get("nonexistent/key.txt").await;
    assert!(result.is_err());
    
    // Try to delete non-existent object
    let result = storage.delete("nonexistent/key.txt").await;
    assert!(result.is_err());
    
    // Try to get metadata for non-existent object
    let result = storage.metadata("nonexistent/key.txt").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_empty_object() {
    let temp_dir = TempDir::new().unwrap();
    let options = StorageOptions::default()
        .with_local_path(temp_dir.path().to_str().unwrap());
    
    let storage = ObjectStorage::new(BackendType::Local, options).await.unwrap();
    
    let key = "empty_object.txt";
    let empty_data = Bytes::from(vec![]);
    
    storage.put(key, empty_data.clone()).await.unwrap();
    
    let retrieved = storage.get(key).await.unwrap();
    assert_eq!(retrieved, empty_data);
    
    let metadata = storage.metadata(key).await.unwrap();
    assert_eq!(metadata.size, 0);
}

#[tokio::test]
async fn test_special_characters_in_key() {
    let temp_dir = TempDir::new().unwrap();
    let options = StorageOptions::default()
        .with_local_path(temp_dir.path().to_str().unwrap());
    
    let storage = ObjectStorage::new(BackendType::Local, options).await.unwrap();
    
    // Test keys with special characters
    let special_keys = vec![
        "key with spaces.txt",
        "key-with-dashes.txt",
        "key_with_underscores.txt",
        "key.multiple.dots.txt",
        "UPPERCASE.TXT",
        "mixedCase.TXT",
    ];
    
    for (i, key) in special_keys.iter().enumerate() {
        let data = Bytes::from(format!("data_{}", i));
        storage.put(key, data.clone()).await.unwrap();
        
        let retrieved = storage.get(key).await.unwrap();
        assert_eq!(retrieved, data);
    }
}

#[tokio::test]
async fn test_storage_options_builder() {
    // Test StorageOptions builder pattern
    let options = StorageOptions::new("my-bucket", "us-west-2")
        .with_credentials("access_key", "secret_key")
        .with_endpoint("http://localhost:9000")
        .with_local_path("/custom/path");
    
    assert_eq!(options.bucket, "my-bucket");
    assert_eq!(options.region, "us-west-2");
    assert_eq!(options.access_key, Some("access_key".to_string()));
    assert_eq!(options.secret_key, Some("secret_key".to_string()));
    assert_eq!(options.endpoint, Some("http://localhost:9000".to_string()));
    assert_eq!(options.local_path, Some("/custom/path".to_string()));
}

#[tokio::test]
async fn test_backend_type_parsing() {
    use chronodb_storage::storage::BackendType;
    
    assert_eq!(BackendType::from_str("local"), Some(BackendType::Local));
    assert_eq!(BackendType::from_str("LOCAL"), Some(BackendType::Local));
    assert_eq!(BackendType::from_str("s3"), Some(BackendType::S3));
    assert_eq!(BackendType::from_str("S3"), Some(BackendType::S3));
    assert_eq!(BackendType::from_str("gcs"), Some(BackendType::Gcs));
    assert_eq!(BackendType::from_str("google"), Some(BackendType::Gcs));
    assert_eq!(BackendType::from_str("minio"), Some(BackendType::Minio));
    assert_eq!(BackendType::from_str("unknown"), None);
}

// Note: S3 and GCS tests are ignored by default as they require
// actual cloud credentials. To run them, set the appropriate
// environment variables and use:
// cargo test -- --ignored

#[tokio::test]
#[ignore]
async fn test_s3_storage_integration() {
    // Requires S3 credentials in environment
    let bucket = std::env::var("S3_BUCKET").expect("S3_BUCKET not set");
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
    
    let storage = ObjectStorage::new(BackendType::S3, options).await.unwrap();
    
    // Run the same tests as local storage
    let key = "test/integration/test_object.bin";
    let data = Bytes::from(vec![1u8, 2, 3, 4, 5]);
    
    storage.put(key, data.clone()).await.unwrap();
    let retrieved = storage.get(key).await.unwrap();
    assert_eq!(retrieved, data);
    
    storage.delete(key).await.unwrap();
}

#[tokio::test]
#[ignore]
async fn test_gcs_storage_integration() {
    // Requires GCS credentials in environment
    let bucket = std::env::var("GCS_BUCKET").expect("GCS_BUCKET not set");
    
    let options = StorageOptions::new(&bucket, "us-central1");
    
    let storage = ObjectStorage::new(BackendType::Gcs, options).await.unwrap();
    
    // Run the same tests as local storage
    let key = "test/integration/test_object.bin";
    let data = Bytes::from(vec![1u8, 2, 3, 4, 5]);
    
    storage.put(key, data.clone()).await.unwrap();
    let retrieved = storage.get(key).await.unwrap();
    assert_eq!(retrieved, data);
    
    storage.delete(key).await.unwrap();
}
