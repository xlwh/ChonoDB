use chronodb_storage::config::StorageConfig;
use chronodb_storage::memstore::MemStore;
use chronodb_storage::model::{Label, Sample};
use chronodb_storage::query::{QueryExecutor, QueryPlan};
use chronodb_storage::query::parser::parse_promql;
use chronodb_storage::query::planner::QueryPlanner;
use std::sync::Arc;
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
async fn test_at_modifier_timestamp() {
    let store = create_test_store();
    let executor = QueryExecutor::new(store.clone());

    // Create test data at different timestamps
    let labels = vec![
        Label::new("__name__", "http_requests_total"),
        Label::new("job", "prometheus"),
    ];

    // Write samples at different timestamps
    store.write(labels.clone(), vec![
        Sample::new(1000, 100.0),
        Sample::new(2000, 200.0),
        Sample::new(3000, 300.0),
    ]).unwrap();

    // Parse query with @ modifier
    let expr = parse_promql("http_requests_total @ 2000").unwrap();
    let planner = QueryPlanner::new();
    let plan = planner.plan(&expr, 0, 4000, 1000).unwrap();

    let result = executor.execute(&plan).await.unwrap();

    // Should return data at timestamp 2000
    assert_eq!(result.series_count(), 1);
    assert_eq!(result.series[0].samples.len(), 1);
    assert_eq!(result.series[0].samples[0].timestamp, 2000);
    assert_eq!(result.series[0].samples[0].value, 200.0);
}

#[tokio::test]
async fn test_at_modifier_with_labels() {
    let store = create_test_store();
    let executor = QueryExecutor::new(store.clone());

    let labels = vec![
        Label::new("__name__", "http_requests_total"),
        Label::new("job", "prometheus"),
        Label::new("instance", "localhost:9090"),
    ];

    store.write(labels.clone(), vec![
        Sample::new(1000, 100.0),
        Sample::new(2000, 200.0),
        Sample::new(3000, 300.0),
    ]).unwrap();

    // Parse query with @ modifier and labels
    let expr = parse_promql("http_requests_total{job=\"prometheus\"} @ 3000").unwrap();
    let planner = QueryPlanner::new();
    let plan = planner.plan(&expr, 0, 4000, 1000).unwrap();

    let result = executor.execute(&plan).await.unwrap();

    assert_eq!(result.series_count(), 1);
    assert_eq!(result.series[0].samples.len(), 1);
    assert_eq!(result.series[0].samples[0].timestamp, 3000);
    assert_eq!(result.series[0].samples[0].value, 300.0);
}

#[tokio::test]
async fn test_at_modifier_nonexistent_timestamp() {
    let store = create_test_store();
    let executor = QueryExecutor::new(store.clone());

    let labels = vec![
        Label::new("__name__", "http_requests_total"),
        Label::new("job", "prometheus"),
    ];

    store.write(labels.clone(), vec![
        Sample::new(1000, 100.0),
        Sample::new(2000, 200.0),
    ]).unwrap();

    // Query at a timestamp where no data exists
    let expr = parse_promql("http_requests_total @ 5000").unwrap();
    let planner = QueryPlanner::new();
    let plan = planner.plan(&expr, 0, 6000, 1000).unwrap();

    let result = executor.execute(&plan).await.unwrap();

    // Should return empty result (no data at timestamp 5000)
    assert_eq!(result.series_count(), 0);
}

#[tokio::test]
async fn test_at_modifier_parsing() {
    // Test that @ modifier is correctly parsed
    let expr = parse_promql("metric @ 1234567890").unwrap();
    
    match &expr.expr_type {
        chronodb_storage::query::parser::ExprType::VectorSelector(vs) => {
            assert!(vs.at.is_some());
            assert_eq!(vs.at.as_ref().unwrap().timestamp, 1234567890);
        }
        _ => panic!("Expected VectorSelector"),
    }
}

#[tokio::test]
async fn test_at_modifier_with_start_function() {
    // Test parsing @ start()
    let expr = parse_promql("metric @ start()").unwrap();
    
    match &expr.expr_type {
        chronodb_storage::query::parser::ExprType::VectorSelector(vs) => {
            assert!(vs.at.is_some());
            assert_eq!(vs.at.as_ref().unwrap().timestamp, -1); // -1 represents start()
        }
        _ => panic!("Expected VectorSelector"),
    }
}

#[tokio::test]
async fn test_at_modifier_with_end_function() {
    // Test parsing @ end()
    let expr = parse_promql("metric @ end()").unwrap();
    
    match &expr.expr_type {
        chronodb_storage::query::parser::ExprType::VectorSelector(vs) => {
            assert!(vs.at.is_some());
            assert_eq!(vs.at.as_ref().unwrap().timestamp, -2); // -2 represents end()
        }
        _ => panic!("Expected VectorSelector"),
    }
}

#[tokio::test]
async fn test_at_modifier_without_modifier() {
    // Test that queries without @ modifier work as before
    let expr = parse_promql("metric").unwrap();
    
    match &expr.expr_type {
        chronodb_storage::query::parser::ExprType::VectorSelector(vs) => {
            assert!(vs.at.is_none());
        }
        _ => panic!("Expected VectorSelector"),
    }
}
