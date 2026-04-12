use crate::error::Result;
use crate::model::TimeSeries;
use crate::query::planner::QueryPlanner;
use crate::query::executor::QueryExecutor;
use crate::memstore::MemStore;
use std::sync::Arc;
use std::num::NonZeroUsize;
use lru::LruCache;
use parking_lot::Mutex;

/// 缓存键：查询参数的组合
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
struct CacheKey {
    query: String,
    start: i64,
    end: i64,
    step: i64,
}

impl CacheKey {
    fn new(query: &str, start: i64, end: i64, step: i64) -> Self {
        Self {
            query: query.to_string(),
            start,
            end,
            step,
        }
    }
}

/// 缓存统计信息
#[derive(Debug, Clone, Default)]
pub struct CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}

impl CacheStats {
    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            self.hits as f64 / total as f64
        }
    }
}

#[derive(Debug, Clone)]
pub struct QueryResult {
    pub series: Vec<TimeSeries>,
    pub start: i64,
    pub end: i64,
    pub step: i64,
}

impl QueryResult {
    pub fn new(series: Vec<TimeSeries>, start: i64, end: i64, step: i64) -> Self {
        Self {
            series,
            start,
            end,
            step,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.series.is_empty()
    }

    pub fn series_count(&self) -> usize {
        self.series.len()
    }

    pub fn sample_count(&self) -> usize {
        self.series.iter().map(|s| s.samples.len()).sum()
    }
}

/// 带缓存的查询引擎
pub struct QueryEngine {
    memstore: Arc<MemStore>,
    planner: QueryPlanner,
    executor: QueryExecutor,
    /// LRU 查询结果缓存
    cache: Mutex<LruCache<CacheKey, QueryResult>>,
    /// 缓存统计
    stats: Mutex<CacheStats>,
    /// 是否启用缓存
    enable_cache: bool,
}

impl QueryEngine {
    /// 创建新的查询引擎（不带缓存）
    pub fn new(memstore: Arc<MemStore>) -> Self {
        let executor = QueryExecutor::new(memstore.clone());
        Self {
            memstore,
            planner: QueryPlanner::new(),
            executor,
            cache: Mutex::new(LruCache::new(NonZeroUsize::new(1000).unwrap())),
            stats: Mutex::new(CacheStats::default()),
            enable_cache: false,
        }
    }

    /// 创建带缓存的查询引擎
    pub fn with_cache(memstore: Arc<MemStore>, cache_size: usize) -> Self {
        let executor = QueryExecutor::new(memstore.clone());
        let cache_size = NonZeroUsize::new(cache_size.max(100)).unwrap_or(NonZeroUsize::new(100).unwrap());
        Self {
            memstore,
            planner: QueryPlanner::new(),
            executor,
            cache: Mutex::new(LruCache::new(cache_size)),
            stats: Mutex::new(CacheStats::default()),
            enable_cache: true,
        }
    }

    /// 启用或禁用缓存
    pub fn set_cache_enabled(&mut self, enabled: bool) {
        self.enable_cache = enabled;
        if !enabled {
            self.clear_cache();
        }
    }

    /// 清空缓存
    pub fn clear_cache(&self) {
        let mut cache = self.cache.lock();
        let evicted = cache.len() as u64;
        cache.clear();
        self.stats.lock().evictions += evicted;
    }

    /// 获取缓存统计
    pub fn cache_stats(&self) -> CacheStats {
        self.stats.lock().clone()
    }

    /// 检查查询是否可缓存
    /// 某些查询（如包含时间函数的）不应该被缓存
    fn is_cacheable(&self, query: &str) -> bool {
        // 不包含时间相关函数的查询可以缓存
        let non_cacheable = [
            "time()", "timestamp()", "day_of_month()", "day_of_week()",
            "days_in_month()", "hour()", "minute()", "month()", "year()",
        ];
        !non_cacheable.iter().any(|&func| query.contains(func))
    }

    pub async fn query(&self, query: &str, start: i64, end: i64, step: i64) -> Result<QueryResult> {
        // 检查缓存
        if self.enable_cache && self.is_cacheable(query) {
            let cache_key = CacheKey::new(query, start, end, step);
            
            // 尝试从缓存获取
            {
                let mut cache = self.cache.lock();
                if let Some(result) = cache.get(&cache_key) {
                    self.stats.lock().hits += 1;
                    tracing::debug!("Query cache hit: {}", query);
                    return Ok(result.clone());
                }
            }
            
            self.stats.lock().misses += 1;
            tracing::debug!("Query cache miss: {}", query);
            
            // 执行查询
            let result = self.execute_query(query, start, end, step).await?;
            
            // 存入缓存
            {
                let mut cache = self.cache.lock();
                cache.put(cache_key, result.clone());
            }
            
            Ok(result)
        } else {
            // 缓存未启用或查询不可缓存，直接执行
            self.execute_query(query, start, end, step).await
        }
    }

    /// 实际执行查询
    async fn execute_query(&self, query: &str, start: i64, end: i64, step: i64) -> Result<QueryResult> {
        let expr = crate::query::parse_promql(query)?;
        let plan = self.planner.plan(&expr, start, end, step)?;
        let result = self.executor.execute(&plan).await?;
        Ok(result)
    }

    pub async fn query_range(&self, query: &str, start: i64, end: i64, step: i64) -> Result<QueryResult> {
        self.query(query, start, end, step).await
    }

    pub async fn query_instant(&self, query: &str, timestamp: i64) -> Result<QueryResult> {
        self.query(query, timestamp, timestamp, 0).await
    }
}

impl Clone for QueryEngine {
    fn clone(&self) -> Self {
        Self {
            memstore: self.memstore.clone(),
            planner: self.planner.clone(),
            executor: self.executor.clone(),
            cache: Mutex::new(LruCache::new(NonZeroUsize::new(1000).unwrap())),
            stats: Mutex::new(CacheStats::default()),
            enable_cache: self.enable_cache,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::StorageConfig;
    use crate::model::{Label, Sample};
    use tempfile::tempdir;

    fn create_test_store() -> Arc<MemStore> {
        let temp_dir = tempdir().unwrap();
        let data_dir = temp_dir.path().to_string_lossy().to_string();
        std::fs::create_dir_all(&data_dir).unwrap();
        let config = StorageConfig {
            data_dir,
            ..Default::default()
        };
        let store = Arc::new(MemStore::new(config).unwrap());
        std::mem::forget(temp_dir);
        store
    }

    #[tokio::test]
    async fn test_query_engine_basic() {
        let store = create_test_store();
        let engine = QueryEngine::new(store.clone());

        // Add test data
        let labels = vec![
            Label::new("__name__", "http_requests_total"),
            Label::new("job", "prometheus"),
            Label::new("instance", "localhost:9090"),
        ];

        let samples = vec![
            Sample::new(1000, 100.0),
            Sample::new(2000, 200.0),
            Sample::new(3000, 300.0),
        ];

        store.write(labels, samples).unwrap();

        // Test simple query
        let result = engine.query("http_requests_total", 0, 4000, 1000).await.unwrap();
        assert!(!result.is_empty());
        assert_eq!(result.series_count(), 1);
        assert_eq!(result.sample_count(), 3);
    }

    #[tokio::test]
    async fn test_query_engine_with_matchers() {
        let store = create_test_store();
        let engine = QueryEngine::new(store.clone());

        // Add test data
        let labels1 = vec![
            Label::new("__name__", "http_requests_total"),
            Label::new("job", "prometheus"),
            Label::new("instance", "localhost:9090"),
        ];

        let labels2 = vec![
            Label::new("__name__", "http_requests_total"),
            Label::new("job", "grafana"),
            Label::new("instance", "localhost:3000"),
        ];

        store.write(labels1, vec![Sample::new(1000, 100.0)]).unwrap();
        store.write(labels2, vec![Sample::new(1000, 200.0)]).unwrap();

        // Test query with job matcher
        let result = engine.query("http_requests_total{job=\"prometheus\"}", 0, 2000, 1000).await.unwrap();
        assert_eq!(result.series_count(), 1);
        assert_eq!(result.series[0].samples[0].value, 100.0);
    }
}
