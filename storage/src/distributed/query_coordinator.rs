use crate::error::{Error, Result};
use crate::model::{TimeSeries, TimeSeriesId};
use crate::query::{QueryPlan, QueryResult};
use crate::rpc::{ClusterRpcManager, QueryRequest, QueryResponse, RpcClient};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

/// 分布式查询协调器
pub struct QueryCoordinator {
    rpc_manager: Arc<ClusterRpcManager>,
    shard_manager: Arc<RwLock<ShardManager>>,
    config: CoordinatorConfig,
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
}

impl Default for CoordinatorConfig {
    fn default() -> Self {
        Self {
            query_timeout_ms: 30000,
            max_concurrent_queries: 100,
            enable_query_cache: true,
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
}

impl ShardManager {
    pub fn new(shard_count: u64) -> Self {
        Self {
            shard_to_nodes: HashMap::new(),
            series_to_shard: HashMap::new(),
            shard_count,
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
}

impl QueryCoordinator {
    pub fn new(
        rpc_manager: Arc<ClusterRpcManager>,
        shard_manager: Arc<RwLock<ShardManager>>,
        config: CoordinatorConfig,
    ) -> Self {
        Self {
            rpc_manager,
            shard_manager,
            config,
        }
    }

    /// 执行分布式查询
    pub async fn execute_query(&self, plan: &QueryPlan) -> Result<QueryResult> {
        let start_time = std::time::Instant::now();

        info!("Executing distributed query: {:?}", plan);

        // 1. 解析查询计划，获取涉及的系列
        let series_ids = self.extract_series_ids(plan).await?;

        if series_ids.is_empty() {
            return Ok(QueryResult::new(Vec::new(), plan.start, plan.end, plan.step));
        }

        // 2. 路由查询到各个分片
        let shard_queries = self.route_to_shards(&series_ids).await?;

        // 3. 并行执行分片查询
        let shard_results = self.execute_shard_queries(shard_queries, plan).await?;

        // 4. 合并结果
        let merged_result = self.merge_results(shard_results, plan)?;

        let duration = start_time.elapsed();
        info!("Distributed query completed in {:?}", duration);

        Ok(merged_result)
    }

    /// 从查询计划中提取系列ID
    async fn extract_series_ids(&self, plan: &QueryPlan) -> Result<Vec<TimeSeriesId>> {
        // 简化实现：根据查询计划中的匹配器获取系列ID
        // 实际实现需要查询元数据服务
        debug!("Extracting series IDs from query plan");
        Ok(Vec::new())
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
                node_id, response.message
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
        let mut seen_ids = std::collections::HashSet::new();
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
        for (group_key, group_series) in groups {
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
        use crate::model::{Label, Sample};

        // 合并所有样本
        let mut all_samples: Vec<Sample> = series
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
}
