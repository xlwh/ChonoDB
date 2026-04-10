use crate::error::{Error, Result};
use crate::memstore::MemStore;
use crate::model::{TimeSeries, TimeSeriesId};
use crate::query::planner::{PlanType, QueryPlan};
use crate::query::QueryResult;
use crate::rpc::ClusterRpcManager;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::SystemTime;
use tokio::sync::{RwLock, Semaphore};
use tracing::{debug, error, info, warn};

/// 分布式查询协调器
pub struct QueryCoordinator {
    rpc_manager: Arc<ClusterRpcManager>,
    shard_manager: Arc<RwLock<ShardManager>>,
    mem_store: Option<Arc<MemStore>>,
    config: CoordinatorConfig,
    query_cache: Arc<RwLock<HashMap<String, (QueryResult, SystemTime)>>>,
    semaphore: Arc<Semaphore>,
    stats: Arc<RwLock<QueryStats>>,
}

/// 协调器配置
#[derive(Debug, Clone)]
pub struct CoordinatorConfig {
    /// 查询超时（毫秒）
    pub query_timeout_ms: u64,
    /// 最大并发查询数
    pub max_concurrent_queries: usize,
    /// 是否启用查询缓存
    pub enable_query_cache: bool,
    /// 查询缓存大小
    pub query_cache_size: usize,
    /// 查询缓存过期时间（秒）
    pub query_cache_ttl: u64,
}

impl Default for CoordinatorConfig {
    fn default() -> Self {
        Self {
            query_timeout_ms: 30000,
            max_concurrent_queries: 100,
            enable_query_cache: true,
            query_cache_size: 1000,
            query_cache_ttl: 300,
        }
    }
}

/// 分片管理器
pub struct ShardManager {
    /// 分片到节点的映射
    shard_to_nodes: HashMap<u64, Vec<String>>,
    /// 系列到分片的映射
    series_to_shard: HashMap<TimeSeriesId, u64>,
    /// 分片数量
    shard_count: u64,
    /// 一致性哈希环
    consistent_hash: ConsistentHash,
}

/// 一致性哈希
struct ConsistentHash {
    nodes: Vec<String>,
    virtual_nodes: HashMap<u64, String>,
    sorted_hashes: Vec<u64>,
}

impl ConsistentHash {
    fn new() -> Self {
        Self {
            nodes: Vec::new(),
            virtual_nodes: HashMap::new(),
            sorted_hashes: Vec::new(),
        }
    }

    fn add_node(&mut self, node_id: String) {
        // 添加虚拟节点
        for i in 0..10 { // 每个节点10个虚拟节点
            let hash = self.hash(&format!("{}-{}", node_id, i));
            self.virtual_nodes.insert(hash, node_id.clone());
            self.sorted_hashes.push(hash);
        }
        // 排序哈希值
        self.sorted_hashes.sort();
        // 添加到节点列表
        if !self.nodes.contains(&node_id) {
            self.nodes.push(node_id);
        }
    }

    fn remove_node(&mut self, node_id: &str) {
        // 移除虚拟节点
        let mut hashes_to_remove = Vec::new();
        for (hash, n) in &self.virtual_nodes {
            if n == node_id {
                hashes_to_remove.push(*hash);
            }
        }
        for hash in hashes_to_remove {
            self.virtual_nodes.remove(&hash);
            self.sorted_hashes.retain(|&h| h != hash);
        }
        // 从节点列表中移除
        self.nodes.retain(|n| n != node_id);
    }

    fn get_node(&self, key: &str) -> Option<&String> {
        if self.virtual_nodes.is_empty() {
            return None;
        }
        let hash = self.hash(key);
        // 找到第一个大于等于哈希值的节点
        let index = self.sorted_hashes.binary_search(&hash).unwrap_or_else(|x| x);
        let index = if index >= self.sorted_hashes.len() {
            0
        } else {
            index
        };
        self.virtual_nodes.get(&self.sorted_hashes[index])
    }

    fn hash(&self, key: &str) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::Hasher;
        let mut hasher = DefaultHasher::new();
        hasher.write(key.as_bytes());
        hasher.finish()
    }
}

impl ShardManager {
    pub fn new(shard_count: u64) -> Self {
        Self {
            shard_to_nodes: HashMap::new(),
            series_to_shard: HashMap::new(),
            shard_count,
            consistent_hash: ConsistentHash::new(),
        }
    }

    /// 计算系列所属的分片
    pub fn get_shard_for_series(&self, series_id: TimeSeriesId) -> u64 {
        // 使用一致性哈希
        series_id % self.shard_count
    }

    /// 获取分片所在的节点
    pub fn get_nodes_for_shard(&self, shard_id: u64) -> Vec<String> {
        self.shard_to_nodes
            .get(&shard_id)
            .cloned()
            .unwrap_or_default()
    }

    /// 分配分片到节点
    pub fn assign_shard_to_node(&mut self, shard_id: u64, node_id: String) {
        self.shard_to_nodes
            .entry(shard_id)
            .or_insert_with(Vec::new)
            .push(node_id);
    }

    /// 路由查询到分片
    pub fn route_query(&self, series_ids: &[TimeSeriesId]) -> HashMap<u64, Vec<TimeSeriesId>> {
        let mut shard_queries: HashMap<u64, Vec<TimeSeriesId>> = HashMap::new();

        for &series_id in series_ids {
            let shard_id = self.get_shard_for_series(series_id);
            shard_queries
                .entry(shard_id)
                .or_insert_with(Vec::new)
                .push(series_id);
        }

        shard_queries
    }

    /// 添加节点到一致性哈希环
    pub fn add_node(&mut self, node_id: String) {
        self.consistent_hash.add_node(node_id);
    }

    /// 从一致性哈希环中移除节点
    pub fn remove_node(&mut self, node_id: &str) {
        self.consistent_hash.remove_node(node_id);
    }

    /// 根据键获取节点
    pub fn get_node_for_key(&self, key: &str) -> Option<&String> {
        self.consistent_hash.get_node(key)
    }
}

impl QueryCoordinator {
    pub fn new(
        rpc_manager: Arc<ClusterRpcManager>,
        shard_manager: Arc<RwLock<ShardManager>>,
        config: CoordinatorConfig,
    ) -> Self {
        let max_concurrent_queries = config.max_concurrent_queries;
        Self {
            rpc_manager,
            shard_manager,
            mem_store: None,
            config,
            query_cache: Arc::new(RwLock::new(HashMap::new())),
            semaphore: Arc::new(Semaphore::new(max_concurrent_queries)),
            stats: Arc::new(RwLock::new(QueryStats::default())),
        }
    }

    pub fn with_mem_store(mut self, mem_store: Arc<MemStore>) -> Self {
        self.mem_store = Some(mem_store);
        self
    }

    pub fn set_mem_store(&mut self, mem_store: Arc<MemStore>) {
        self.mem_store = Some(mem_store);
    }

    /// 执行分布式查询
    pub async fn execute_query(&self, plan: &QueryPlan) -> Result<QueryResult> {
        let start_time = std::time::Instant::now();

        // 生成查询缓存键
        let cache_key = format!("{:?}", plan);

        // 检查缓存
        if self.config.enable_query_cache {
            if let Some((result, timestamp)) = self.query_cache.read().await.get(&cache_key) {
                let now = SystemTime::now();
                let duration = now.duration_since(*timestamp).unwrap();
                if duration.as_secs() < self.config.query_cache_ttl {
                    info!("Query cache hit");
                    return Ok(result.clone());
                }
            }
        }

        info!("Executing distributed query: {:?}", plan);

        // 1. 解析查询计划，获取涉及的系列
        let series_ids = self.extract_series_ids(plan).await?;

        if series_ids.is_empty() {
            let result = QueryResult::new(Vec::new(), plan.start, plan.end, plan.step);
            // 缓存结果
            self.cache_result(cache_key, result.clone()).await;
            return Ok(result);
        }

        // 2. 路由查询到各个分片
        let shard_queries = self.route_to_shards(&series_ids).await?;

        // 3. 并行执行分片查询
        let shard_results = self.execute_shard_queries(shard_queries, plan).await?;

        // 4. 合并结果
        let merged_result = self.merge_results(shard_results, plan)?;

        // 缓存结果
        self.cache_result(cache_key, merged_result.clone()).await;

        let duration = start_time.elapsed();
        info!("Distributed query completed in {:?}", duration);

        // 更新统计信息
        let mut stats = self.stats.write().await;
        stats.total_queries += 1;
        stats.successful_queries += 1;
        stats.avg_query_time_ms = stats.avg_query_time_ms * 0.9 + duration.as_millis() as f64 * 0.1 ;
        stats.total_series_queried += merged_result.series.len() as u64;
        stats.total_samples_queried += merged_result.series.iter().map(|s| s.samples.len() as u64).sum::<u64>();

        Ok(merged_result)
    }

    /// 缓存查询结果
    async fn cache_result(&self, key: String, result: QueryResult) {
        let mut cache = self.query_cache.write().await;
        // 如果缓存已满，移除最旧的项
        if cache.len() >= self.config.query_cache_size {
            if let Some(old_key) = cache.iter().min_by_key(|(_, (_, t))| t).map(|(k, _)| k.clone()) {
                cache.remove(&old_key);
            }
        }
        cache.insert(key, (result, SystemTime::now()));
    }

    /// 从查询计划中提取系列ID
    async fn extract_series_ids(&self, plan: &QueryPlan) -> Result<Vec<TimeSeriesId>> {
        let matchers = Self::extract_matchers_from_plan(&plan.plan_type);

        if matchers.is_empty() {
            debug!("No matchers found in query plan, returning all series");
            if let Some(ref mem_store) = self.mem_store {
                return Ok(mem_store.get_all_series_ids());
            }
            return Ok(Vec::new());
        }

        debug!("Extracting series IDs with matchers: {:?}", matchers);

        if let Some(ref mem_store) = self.mem_store {
            let start = plan.start;
            let end = plan.end;
            match mem_store.query(&matchers, start, end) {
                Ok(series_list) => {
                    let series_ids: Vec<TimeSeriesId> =
                        series_list.iter().map(|ts| ts.id).collect();
                    debug!("Found {} series from local index", series_ids.len());
                    Ok(series_ids)
                }
                Err(e) => {
                    warn!("Failed to query local index: {}", e);
                    Ok(Vec::new())
                }
            }
        } else {
            debug!("No local mem_store available, cannot extract series IDs");
            Ok(Vec::new())
        }
    }

    fn extract_matchers_from_plan(plan_type: &PlanType) -> Vec<(String, String)> {
        match plan_type {
            PlanType::VectorQuery(vq) => vq.matchers.clone(),
            PlanType::MatrixQuery(mq) => mq.vector_plan.matchers.clone(),
            PlanType::Call(call) => {
                for arg in &call.args {
                    let matchers = Self::extract_matchers_from_plan(&arg.plan_type);
                    if !matchers.is_empty() {
                        return matchers;
                    }
                }
                Vec::new()
            }
            PlanType::BinaryExpr(bin) => {
                let lhs = Self::extract_matchers_from_plan(&bin.lhs.plan_type);
                if !lhs.is_empty() {
                    return lhs;
                }
                Self::extract_matchers_from_plan(&bin.rhs.plan_type)
            }
            PlanType::UnaryExpr(unary) => {
                Self::extract_matchers_from_plan(&unary.expr.plan_type)
            }
            PlanType::Aggregation(agg) => {
                Self::extract_matchers_from_plan(&agg.expr.plan_type)
            }
        }
    }

    /// 路由查询到分片
    async fn route_to_shards(
        &self,
        series_ids: &[TimeSeriesId],
    ) -> Result<HashMap<u64, Vec<TimeSeriesId>>> {
        let shard_manager = self.shard_manager.read().await;
        let shard_queries = shard_manager.route_query(series_ids);
        drop(shard_manager);

        debug!("Routed query to {} shards", shard_queries.len());
        Ok(shard_queries)
    }

    /// 执行分片查询
    async fn execute_shard_queries(
        &self,
        shard_queries: HashMap<u64, Vec<TimeSeriesId>>,
        plan: &QueryPlan,
    ) -> Result<Vec<QueryResult>> {
        let mut tasks = Vec::new();

        for (shard_id, series_ids) in shard_queries {
            let shard_manager = self.shard_manager.read().await;
            let nodes = shard_manager.get_nodes_for_shard(shard_id);
            drop(shard_manager);

            if nodes.is_empty() {
                warn!("No nodes found for shard {}", shard_id);
                continue;
            }

            // 选择主节点进行查询
            let primary_node = nodes[0].clone();
            let rpc_manager = Arc::clone(&self.rpc_manager);
            let start = plan.start;
            let end = plan.end;

            let task = tokio::spawn(async move {
                Self::query_shard(rpc_manager, primary_node, shard_id, series_ids, start, end).await
            });

            tasks.push(task);
        }

        // 等待所有查询完成
        let mut results = Vec::new();
        for task in tasks {
            match task.await {
                Ok(Ok(result)) => results.push(result),
                Ok(Err(e)) => error!("Shard query failed: {}", e),
                Err(e) => error!("Shard query task panicked: {}", e),
            }
        }

        Ok(results)
    }

    /// 查询单个分片
    async fn query_shard(
        rpc_manager: Arc<ClusterRpcManager>,
        node_id: String,
        shard_id: u64,
        series_ids: Vec<TimeSeriesId>,
        start: i64,
        end: i64,
    ) -> Result<QueryResult> {
        debug!(
            "Querying shard {} on node {} for {} series",
            shard_id,
            node_id,
            series_ids.len()
        );

        // 获取RPC客户端
        let client = rpc_manager
            .get_client(&node_id)
            .await
            .ok_or_else(|| Error::Internal(format!("No RPC client for node {}", node_id)))?;

        // 发送查询请求
        let response = client
            .query(series_ids, start, end)
            .await
            .map_err(|e| Error::Internal(format!("Query failed on node {}: {}", node_id, e)))?;

        if !response.success {
            return Err(Error::Internal(format!(
                "Query failed on node {}: {}",
                node_id,
                response.message
            )));
        }

        // 转换响应为QueryResult
        let series = response.series;
        Ok(QueryResult::new(series, start, end, 0))
    }

    /// 合并查询结果
    fn merge_results(
        &self,
        results: Vec<QueryResult>,
        plan: &QueryPlan,
    ) -> Result<QueryResult> {
        let mut all_series: Vec<TimeSeries> = Vec::new();

        for result in results {
            all_series.extend(result.series);
        }

        // 去重（按系列ID）
        let mut seen_ids = HashSet::new();
        all_series.retain(|ts| seen_ids.insert(ts.id));

        info!("Merged {} series from shard results", all_series.len());

        Ok(QueryResult::new(all_series, plan.start, plan.end, plan.step))
    }

    /// 执行聚合查询
    pub async fn execute_aggregation(
        &self,
        plan: &QueryPlan,
        aggregation_type: AggregationType,
        group_by: Vec<String>,
    ) -> Result<QueryResult> {
        // 1. 先执行基础查询
        let base_result = self.execute_query(plan).await?;

        // 2. 执行聚合
        let aggregated = self.aggregate_series(base_result.series, aggregation_type, group_by)?;

        Ok(QueryResult::new(aggregated, plan.start, plan.end, plan.step))
    }

    /// 聚合系列
    fn aggregate_series(
        &self,
        series: Vec<TimeSeries>,
        aggregation_type: AggregationType,
        group_by: Vec<String>,
    ) -> Result<Vec<TimeSeries>> {
        use std::collections::HashMap;

        // 按分组键分组
        let mut groups: HashMap<Vec<(String, String)>, Vec<TimeSeries>> = HashMap::new();

        for ts in series {
            let group_key: Vec<(String, String)> = ts
                .labels
                .iter()
                .filter(|l| group_by.contains(&l.name))
                .map(|l| (l.name.clone(), l.value.clone()))
                .collect();

            groups.entry(group_key).or_insert_with(Vec::new).push(ts);
        }

        // 对每个组执行聚合
        let mut result = Vec::new();
        for (_group_key, group_series) in groups {
            let aggregated = self.aggregate_group(group_series, aggregation_type)?;
            result.push(aggregated);
        }

        Ok(result)
    }

    /// 聚合单个组
    fn aggregate_group(
        &self,
        series: Vec<TimeSeries>,
        aggregation_type: AggregationType,
    ) -> Result<TimeSeries> {
        use crate::model::Sample;

        // 合并所有样本
        let all_samples: Vec<Sample> = series
            .iter()
            .flat_map(|ts| ts.samples.clone())
            .collect();

        // 按时间戳分组并聚合
        let mut samples_by_time: HashMap<i64, Vec<f64>> = HashMap::new();
        for sample in all_samples {
            samples_by_time
                .entry(sample.timestamp)
                .or_insert_with(Vec::new)
                .push(sample.value);
        }

        // 计算聚合值
        let mut aggregated_samples: Vec<Sample> = samples_by_time
            .into_iter()
            .map(|(timestamp, values)| {
                let aggregated_value = match aggregation_type {
                    AggregationType::Sum => values.iter().sum(),
                    AggregationType::Avg => values.iter().sum::<f64>() / values.len() as f64,
                    AggregationType::Min => values.iter().copied().fold(f64::MAX, f64::min),
                    AggregationType::Max => values.iter().copied().fold(f64::MIN, f64::max),
                    AggregationType::Count => values.len() as f64,
                };
                Sample::new(timestamp, aggregated_value)
            })
            .collect();

        aggregated_samples.sort_by_key(|s| s.timestamp);

        // 创建结果系列
        let labels = if let Some(first) = series.first() {
            first.labels.clone()
        } else {
            Vec::new()
        };

        let mut result_ts = TimeSeries::new(0, labels);
        result_ts.add_samples(aggregated_samples);

        Ok(result_ts)
    }

    /// 获取查询统计信息
    pub async fn get_stats(&self) -> Result<QueryStats> {
        let stats = self.stats.read().await;
        Ok(stats.clone())
    }

    /// 清理过期缓存
    pub async fn cleanup_cache(&self) {
        let mut cache = self.query_cache.write().await;
        let now = SystemTime::now();
        cache.retain(|_, (_, timestamp)| {
            let duration = now.duration_since(*timestamp).unwrap();
            duration.as_secs() < self.config.query_cache_ttl
        });
    }
}

/// 聚合类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AggregationType {
    Sum,
    Avg,
    Min,
    Max,
    Count,
}

/// 查询统计信息
#[derive(Debug, Clone, Default)]
pub struct QueryStats {
    pub total_queries: u64,
    pub successful_queries: u64,
    pub failed_queries: u64,
    pub avg_query_time_ms: f64,
    pub total_series_queried: u64,
    pub total_samples_queried: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shard_manager() {
        let manager = ShardManager::new(128);
        let shard_id = manager.get_shard_for_series(12345);
        assert!(shard_id < 128);
    }

    #[test]
    fn test_aggregation_type() {
        assert_eq!(AggregationType::Sum, AggregationType::Sum);
        assert_ne!(AggregationType::Sum, AggregationType::Avg);
    }

    #[tokio::test]
    async fn test_coordinator_config_default() {
        let config = CoordinatorConfig::default();
        assert_eq!(config.query_timeout_ms, 30000);
        assert_eq!(config.max_concurrent_queries, 100);
        assert!(config.enable_query_cache);
    }

    #[test]
    fn test_consistent_hash() {
        let mut ch = ConsistentHash::new();
        ch.add_node("node1".to_string());
        ch.add_node("node2".to_string());
        
        let node1 = ch.get_node("key1");
        assert!(node1.is_some());
        
        let node2 = ch.get_node("key2");
        assert!(node2.is_some());
        
        ch.remove_node("node1");
        let node3 = ch.get_node("key1");
        assert!(node3.is_some());
    }
}
