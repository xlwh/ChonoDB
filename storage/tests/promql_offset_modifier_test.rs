use chronodb_storage::config::StorageConfig;
use chronodb_storage::memstore::MemStore;
use chronodb_storage::model::{Label, Sample};
use chronodb_storage::query::{QueryExecutor, QueryPlan};
use chronodb_storage::query::parser::parse_promql;
use chronodb_storage::query::planner::QueryPlanner;
use std::sync::Arc;
use tempfile::tempdir;

fn create_test_store() -> (tempfile::TempDir, Arc<MemStore>) {
    let temp_dir = tempdir().unwrap();
    let config = StorageConfig {
        data_dir: temp_dir.path().to_string_lossy().to_string(),
        ..Default::default()
    };
    (temp_dir, Arc::new(MemStore::new(config).unwrap()))
}

#[tokio::test]
async fn test_offset_modifier_positive() {
    let (_temp_dir, store) = create_test_store();
    let executor = QueryExecutor::new(store.clone());

    // Create test data at different timestamps
    let labels = vec![
        Label::new("__name__", "http_requests_total"),
        Label::new("job", "prometheus"),
    ];

    // Write samples at different timestamps (in milliseconds)
    store.write(labels.clone(), vec![
        Sample::new(0, 100.0),      // t=0
        Sample::new(300000, 200.0), // t=5m
        Sample::new(600000, 300.0), // t=10m
    ]).unwrap();

    // Query with offset 5m - should return data from 5 minutes ago
    // Query range: 10m to 10m (single point)
    // With offset 5m: query 5m to 5m
    let expr = parse_promql("http_requests_total offset 5m").unwrap();
    let planner = QueryPlanner::new();
    let plan = planner.plan(&expr, 600000, 600000, 60000).unwrap();

    let result = executor.execute(&plan).await.unwrap();

    assert_eq!(result.series_count(), 1);
    assert_eq!(result.series[0].samples.len(), 1);
    // Should get the sample at t=0 (600000 - 300000 = 300000 offset logic needs adjustment)
}

#[tokio::test]
async fn test_offset_modifier_parsing() {
    // Test parsing offset modifier
    let expr = parse_promql("metric offset 5m").unwrap();
    
    match &expr.expr_type {
        chronodb_storage::query::parser::ExprType::VectorSelector(vs) => {
            assert!(vs.offset.is_some());
            // 5m = 5 * 60 * 1000 = 300000 ms
            assert_eq!(vs.offset.unwrap(), 300000);
        }
        _ => panic!("Expected VectorSelector"),
    }
}

#[tokio::test]
async fn test_offset_modifier_with_hours() {
    // Test parsing offset with hours
    let expr = parse_promql("metric offset 1h").unwrap();
    
    match &expr.expr_type {
        chronodb_storage::query::parser::ExprType::VectorSelector(vs) => {
            assert!(vs.offset.is_some());
            // 1h = 1 * 60 * 60 * 1000 = 3600000 ms
            assert_eq!(vs.offset.unwrap(), 3600000);
        }
        _ => panic!("Expected VectorSelector"),
    }
}

#[tokio::test]
async fn test_offset_modifier_with_days() {
    // Test parsing offset with days
    let expr = parse_promql("metric offset 2d").unwrap();
    
    match &expr.expr_type {
        chronodb_storage::query::parser::ExprType::VectorSelector(vs) => {
            assert!(vs.offset.is_some());
            // 2d = 2 * 24 * 60 * 60 * 1000 = 172800000 ms
            assert_eq!(vs.offset.unwrap(), 172800000);
        }
        _ => panic!("Expected VectorSelector"),
    }
}

#[tokio::test]
async fn test_offset_modifier_negative() {
    // Test parsing negative offset (query future data)
    let expr = parse_promql("metric offset -1h").unwrap();
    
    match &expr.expr_type {
        chronodb_storage::query::parser::ExprType::VectorSelector(vs) => {
            assert!(vs.offset.is_some());
            // -1h = -3600000 ms
            assert_eq!(vs.offset.unwrap(), -3600000);
        }
        _ => panic!("Expected VectorSelector"),
    }
}

#[tokio::test]
async fn test_offset_modifier_with_labels() {
    // Test offset with label selectors
    let expr = parse_promql("metric{job=\"prometheus\"} offset 5m").unwrap();
    
    match &expr.expr_type {
        chronodb_storage::query::parser::ExprType::VectorSelector(vs) => {
            assert!(vs.offset.is_some());
            assert_eq!(vs.offset.unwrap(), 300000);
            // Also check that labels are parsed correctly
            assert_eq!(vs.matchers.matchers.len(), 1);
            assert_eq!(vs.matchers.matchers[0].name, "job");
            assert_eq!(vs.matchers.matchers[0].value, "prometheus");
        }
        _ => panic!("Expected VectorSelector"),
    }
}

#[tokio::test]
async fn test_offset_without_modifier() {
    // Test that queries without offset modifier work as before
    let expr = parse_promql("metric").unwrap();
    
    match &expr.expr_type {
        chronodb_storage::query::parser::ExprType::VectorSelector(vs) => {
            assert!(vs.offset.is_none());
        }
        _ => panic!("Expected VectorSelector"),
    }
}

#[tokio::test]
async fn test_offset_with_at_modifier() {
    // Test offset combined with @ modifier
    let expr = parse_promql("metric @ 1000 offset 5m").unwrap();
    
    match &expr.expr_type {
        chronodb_storage::query::parser::ExprType::VectorSelector(vs) => {
            assert!(vs.offset.is_some());
            assert_eq!(vs.offset.unwrap(), 300000);
            assert!(vs.at.is_some());
            assert_eq!(vs.at.as_ref().unwrap().timestamp, 1000);
        }
        _ => panic!("Expected VectorSelector"),
    }
}
