use std::sync::Arc;
use tokio::sync::RwLock;
use chronodb_storage::{MemStore, StorageConfig};
use crate::config::ServerConfig;
use crate::rules::{RuleManager, AlertManager};
use crate::targets::TargetManager;

/// 服务器共享状态
pub struct ServerState {
    /// 服务器配置
    pub config: ServerConfig,
    
    /// 内存存储
    pub memstore: Arc<MemStore>,
    
    /// 规则管理器
    pub rule_manager: Arc<RwLock<RuleManager>>,
    
    /// 告警管理器
    pub alert_manager: Arc<RwLock<AlertManager>>,
    
    /// 目标管理器
    pub target_manager: Arc<RwLock<TargetManager>>,
}

impl ServerState {
    pub async fn new(config: ServerConfig) -> crate::Result<Arc<Self>> {
        // 创建内存存储
        let storage_config = StorageConfig::default();
        let memstore = Arc::new(MemStore::new(storage_config)?);
        
        // 创建规则管理器
        let rule_manager = Arc::new(RwLock::new(RuleManager::new()));
        
        // 创建告警管理器
        let alert_manager = Arc::new(RwLock::new(AlertManager::new()));
        
        // 创建目标管理器
        let target_manager = Arc::new(RwLock::new(TargetManager::new()));
        
        Ok(Arc::new(Self {
            config,
            memstore,
            rule_manager,
            alert_manager,
            target_manager,
        }))
    }
}
