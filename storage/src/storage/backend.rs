use crate::error::Result;
use async_trait::async_trait;
use bytes::Bytes;
use std::collections::HashMap;

/// 存储选项
#[derive(Debug, Clone)]
pub struct StorageOptions {
    /// 存储桶名称
    pub bucket: String,
    /// 区域
    pub region: String,
    /// 访问密钥
    pub access_key: Option<String>,
    /// 访问密钥 Secret
    pub secret_key: Option<String>,
    /// 端点 URL（用于 MinIO 等兼容服务）
    pub endpoint: Option<String>,
    /// 本地存储路径
    pub local_path: Option<String>,
    /// 其他选项
    pub extra: HashMap<String, String>,
}

impl StorageOptions {
    pub fn new(bucket: &str, region: &str) -> Self {
        Self {
            bucket: bucket.to_string(),
            region: region.to_string(),
            access_key: None,
            secret_key: None,
            endpoint: None,
            local_path: None,
            extra: HashMap::new(),
        }
    }

    pub fn with_credentials(mut self, access_key: &str, secret_key: &str) -> Self {
        self.access_key = Some(access_key.to_string());
        self.secret_key = Some(secret_key.to_string());
        self
    }

    pub fn with_endpoint(mut self, endpoint: &str) -> Self {
        self.endpoint = Some(endpoint.to_string());
        self
    }

    pub fn with_local_path(mut self, path: &str) -> Self {
        self.local_path = Some(path.to_string());
        self
    }
}

impl Default for StorageOptions {
    fn default() -> Self {
        Self {
            bucket: "chronodb".to_string(),
            region: "us-east-1".to_string(),
            access_key: None,
            secret_key: None,
            endpoint: None,
            local_path: Some("/tmp/chronodb".to_string()),
            extra: HashMap::new(),
        }
    }
}

/// 对象元数据
#[derive(Debug, Clone)]
pub struct ObjectMetadata {
    /// 对象键
    pub key: String,
    /// 对象大小（字节）
    pub size: u64,
    /// 最后修改时间
    pub last_modified: i64,
    /// ETag
    pub etag: Option<String>,
    /// 内容类型
    pub content_type: Option<String>,
    /// 自定义元数据
    pub metadata: HashMap<String, String>,
}

/// 存储后端 trait
#[async_trait]
pub trait StorageBackend: Send + Sync {
    /// 上传对象
    async fn put(&self, key: &str, data: Bytes) -> Result<()>;

    /// 下载对象
    async fn get(&self, key: &str) -> Result<Bytes>;

    /// 删除对象
    async fn delete(&self, key: &str) -> Result<()>;

    /// 检查对象是否存在
    async fn exists(&self, key: &str) -> Result<bool>;

    /// 列出对象
    async fn list(&self, prefix: &str) -> Result<Vec<String>>;

    /// 获取对象元数据
    async fn metadata(&self, key: &str) -> Result<ObjectMetadata>;
}

/// 存储配置（别名，用于兼容）
pub type StorageConfig = StorageOptions;

/// 存储对象（别名，用于兼容）
pub type StorageObject = ObjectMetadata;

/// 列表选项
#[derive(Debug, Clone, Default)]
pub struct ListOptions {
    /// 最大返回数量
    pub max_keys: Option<usize>,
    /// 分页令牌
    pub continuation_token: Option<String>,
}
