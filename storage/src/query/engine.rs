use crate::error::Result;
use crate::model::TimeSeries;
use crate::query::planner::QueryPlanner;
use crate::query::executor::QueryExecutor;
use crate::memstore::MemStore;
use std::sync::Arc;

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

#[derive(Clone)]
pub struct QueryEngine {
    memstore: Arc<MemStore>,
    planner: QueryPlanner,
    executor: QueryExecutor,
}

impl QueryEngine {
    pub fn new(memstore: Arc<MemStore>) -> Self {
        let executor = QueryExecutor::new(memstore.clone());
        Self {
            memstore,
            planner: QueryPlanner::new(),
            executor,
        }
    }

    pub async fn query(&self, query: &str, start: i64, end: i64, step: i64) -> Result<QueryResult> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::StorageConfig;
    use crate::model::{Label, Sample};
    use tempfile::tempdir;

    fn create_test_store() -> Arc<MemStore> {
        let temp_dir = tempdir().unwrap();
        let config = StorageConfig {
            data_dir: temp_dir.path().to_string_lossy().to_string(),
            ..Default::default()
        };
        Arc::new(MemStore::new(config).unwrap())
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
