use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};
use chronodb_storage::{MemStore, StorageConfig};
use chronodb_storage::downsample::{DownsampleManager, DownsampleConfig as StorageDownsampleConfig};
use chronodb_storage::query::{QueryEngine, FrequencyTracker, FrequencyConfig};
use crate::config::ServerConfig;
use crate::rules::{RuleManager, AlertManager, PreAggregationManager, PreAggregationScheduler};
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
    
    /// 降采样管理器
    pub downsample_manager: Arc<RwLock<DownsampleManager>>,
    
    /// 预聚合管理器
    pub pre_aggregation_manager: Arc<RwLock<PreAggregationManager>>,
    
    /// 预聚合调度器
    pub pre_aggregation_scheduler: Arc<PreAggregationScheduler>,
}

impl ServerState {
    pub async fn new(config: ServerConfig) -> crate::Result<Arc<Self>> {
        // 创建内存存储 - 使用配置中的 data_dir
        let storage_config = StorageConfig {
            data_dir: config.data_dir.to_string_lossy().to_string(),
            ..StorageConfig::default()
        };
        let memstore = Arc::new(MemStore::new(storage_config)?);
        
        // 创建规则管理器
        let mut rule_manager = RuleManager::new();
        
        // 加载告警规则文件
        for rule_file in &config.rules.rule_files {
            if rule_file.exists() {
                info!("Loading rules from file: {:?}", rule_file);
                rule_manager.load_from_file(rule_file)?;
            } else {
                warn!("Rule file not found: {:?}", rule_file);
            }
        }
        
        let rule_manager = Arc::new(RwLock::new(rule_manager));
        
        // 创建告警管理器
        let alert_manager = Arc::new(RwLock::new(AlertManager::new()));
        
        // 创建目标管理器
        let target_manager = Arc::new(RwLock::new(TargetManager::new()));
        
        // 创建降采样配置
        let downsample_config = StorageDownsampleConfig {
            enabled: config.downsampling.enabled,
            interval: std::time::Duration::from_secs(config.downsampling.interval),
            concurrency: config.downsampling.concurrency,
            timeout: std::time::Duration::from_secs(config.downsampling.timeout),
            levels: config.downsampling.levels.iter().map(|l| {
                let level = match l.level.as_str() {
                    "L1" => chronodb_storage::columnstore::DownsampleLevel::L1,
                    "L2" => chronodb_storage::columnstore::DownsampleLevel::L2,
                    "L3" => chronodb_storage::columnstore::DownsampleLevel::L3,
                    "L4" => chronodb_storage::columnstore::DownsampleLevel::L4,
                    _ => chronodb_storage::columnstore::DownsampleLevel::L1,
                };
                chronodb_storage::downsample::LevelConfig {
                    level,
                    enabled: l.enabled,
                    functions: l.functions.clone(),
                }
            }).collect(),
        };
        
        // 创建降采样管理器
        let downsample_manager = DownsampleManager::new(
            downsample_config,
            memstore.clone(),
            config.data_dir.clone(),
        );
        
        // 启动降采样管理器
        let mut downsample_manager_mut = downsample_manager;
        downsample_manager_mut.start().await?;
        
        let downsample_manager = Arc::new(RwLock::new(downsample_manager_mut));
        
        // 创建 QueryEngine
        let query_engine = Arc::new(QueryEngine::new(memstore.clone()));
        
        // 创建 FrequencyTracker
        let frequency_config = FrequencyConfig::default();
        let frequency_tracker = Arc::new(FrequencyTracker::new(frequency_config));
        
        // 创建预聚合管理器
        let pre_aggregation_manager = PreAggregationManager::new(frequency_tracker);
        let pre_aggregation_manager = Arc::new(RwLock::new(pre_aggregation_manager));
        
        // 创建预聚合调度器
        let pre_aggregation_scheduler = PreAggregationScheduler::new(query_engine);
        let pre_aggregation_scheduler = Arc::new(pre_aggregation_scheduler);
        
        // 启动预聚合调度器
        let scheduler_clone = pre_aggregation_scheduler.clone();
        tokio::spawn(async move {
            scheduler_clone.start().await;
        });
        
        Ok(Arc::new(Self {
            config,
            memstore,
            rule_manager,
            alert_manager,
            target_manager,
            downsample_manager,
            pre_aggregation_manager,
            pre_aggregation_scheduler,
        }))
    }
}
