use chronodb_storage::config::StorageConfig;
use chronodb_storage::memstore::MemStore;
use chronodb_storage::model::{Label, Sample};
use chronodb_storage::query::{QueryExecutor, QueryPlan};
use chronodb_storage::query::parser::Function;
use chronodb_storage::query::planner::{PlanType, VectorQueryPlan, CallPlan};
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
async fn test_topk_basic() {
    let store = create_test_store();
    let executor = QueryExecutor::new(store.clone());

    // Create test data with different values
    for i in 1..=10 {
        let labels = vec![
            Label::new("__name__", "cpu_usage"),
            Label::new("instance", &format!("server{}", i)),
        ];
        store.write(labels, vec![Sample::new(1000, i as f64 * 10.0)]).unwrap();
    }

    let vector_plan = PlanType::VectorQuery(VectorQueryPlan {
        name: Some("cpu_usage".to_string()),
        matchers: vec![("__name__".to_string(), "cpu_usage".to_string())],
        at: None,
        offset: None,
    });

    let call_plan = PlanType::Call(CallPlan {
        func: Function::TopK,
        args: vec![
            QueryPlan {
                plan_type: PlanType::VectorQuery(VectorQueryPlan {
                    name: None,
                    matchers: vec![],
                    at: None,
                    offset: None,
                }),
                start: 0,
                end: 2000,
                step: 1000,
            },
            QueryPlan {
                plan_type: vector_plan,
                start: 0,
                end: 2000,
                step: 1000,
            },
        ],
        k: Some(3),
        quantile: None,
    });

    let plan = QueryPlan {
        plan_type: call_plan,
        start: 0,
        end: 2000,
        step: 1000,
    };

    let result = executor.execute(&plan).await.unwrap();

    // Should return top 3 values: 100, 90, 80
    assert_eq!(result.series_count(), 3, "Should return exactly 3 series");
    
    // Check that we got the highest values
    let values: Vec<f64> = result.series.iter()
        .map(|s| s.samples[0].value)
        .collect();
    
    assert!(values.contains(&100.0), "Should contain max value 100");
    assert!(values.contains(&90.0), "Should contain second max value 90");
    assert!(values.contains(&80.0), "Should contain third max value 80");
}

#[tokio::test]
async fn test_bottomk_basic() {
    let store = create_test_store();
    let executor = QueryExecutor::new(store.clone());

    // Create test data with different values
    for i in 1..=10 {
        let labels = vec![
            Label::new("__name__", "memory_usage"),
            Label::new("instance", &format!("server{}", i)),
        ];
        store.write(labels, vec![Sample::new(1000, i as f64 * 10.0)]).unwrap();
    }

    let vector_plan = PlanType::VectorQuery(VectorQueryPlan {
        name: Some("memory_usage".to_string()),
        matchers: vec![("__name__".to_string(), "memory_usage".to_string())],
        at: None,
        offset: None,
    });

    let call_plan = PlanType::Call(CallPlan {
        func: Function::BottomK,
        args: vec![
            QueryPlan {
                plan_type: PlanType::VectorQuery(VectorQueryPlan {
                    name: None,
                    matchers: vec![],
                    at: None,
                    offset: None,
                }),
                start: 0,
                end: 2000,
                step: 1000,
            },
            QueryPlan {
                plan_type: vector_plan,
                start: 0,
                end: 2000,
                step: 1000,
            },
        ],
        k: Some(3),
        quantile: None,
    });

    let plan = QueryPlan {
        plan_type: call_plan,
        start: 0,
        end: 2000,
        step: 1000,
    };

    let result = executor.execute(&plan).await.unwrap();

    // Should return bottom 3 values: 10, 20, 30
    assert_eq!(result.series_count(), 3, "Should return exactly 3 series");
    
    // Check that we got the lowest values
    let values: Vec<f64> = result.series.iter()
        .map(|s| s.samples[0].value)
        .collect();
    
    assert!(values.contains(&10.0), "Should contain min value 10");
    assert!(values.contains(&20.0), "Should contain second min value 20");
    assert!(values.contains(&30.0), "Should contain third min value 30");
}

#[tokio::test]
async fn test_topk_k_zero() {
    let store = create_test_store();
    let executor = QueryExecutor::new(store.clone());

    let labels = vec![
        Label::new("__name__", "metric"),
        Label::new("instance", "server1"),
    ];
    store.write(labels, vec![Sample::new(1000, 100.0)]).unwrap();

    let vector_plan = PlanType::VectorQuery(VectorQueryPlan {
        name: Some("metric".to_string()),
        matchers: vec![("__name__".to_string(), "metric".to_string())],
        at: None,
        offset: None,
    });

    let call_plan = PlanType::Call(CallPlan {
        func: Function::TopK,
        args: vec![
            QueryPlan {
                plan_type: PlanType::VectorQuery(VectorQueryPlan {
                    name: None,
                    matchers: vec![],
                    at: None,
                    offset: None,
                }),
                start: 0,
                end: 2000,
                step: 1000,
            },
            QueryPlan {
                plan_type: vector_plan,
                start: 0,
                end: 2000,
                step: 1000,
            },
        ],
        k: Some(0),
        quantile: None,
    });

    let plan = QueryPlan {
        plan_type: call_plan,
        start: 0,
        end: 2000,
        step: 1000,
    };

    let result = executor.execute(&plan).await.unwrap();

    // k=0 should return empty result
    assert_eq!(result.series_count(), 0, "k=0 should return empty result");
}

#[tokio::test]
async fn test_topk_k_greater_than_series() {
    let store = create_test_store();
    let executor = QueryExecutor::new(store.clone());

    // Create only 3 series
    for i in 1..=3 {
        let labels = vec![
            Label::new("__name__", "metric"),
            Label::new("instance", &format!("server{}", i)),
        ];
        store.write(labels, vec![Sample::new(1000, i as f64 * 10.0)]).unwrap();
    }

    let vector_plan = PlanType::VectorQuery(VectorQueryPlan {
        name: Some("metric".to_string()),
        matchers: vec![("__name__".to_string(), "metric".to_string())],
        at: None,
        offset: None,
    });

    let call_plan = PlanType::Call(CallPlan {
        func: Function::TopK,
        args: vec![
            QueryPlan {
                plan_type: PlanType::VectorQuery(VectorQueryPlan {
                    name: None,
                    matchers: vec![],
                    at: None,
                    offset: None,
                }),
                start: 0,
                end: 2000,
                step: 1000,
            },
            QueryPlan {
                plan_type: vector_plan,
                start: 0,
                end: 2000,
                step: 1000,
            },
        ],
        k: Some(10), // k > series count
        quantile: None,
    });

    let plan = QueryPlan {
        plan_type: call_plan,
        start: 0,
        end: 2000,
        step: 1000,
    };

    let result = executor.execute(&plan).await.unwrap();

    // Should return all available series (3)
    assert_eq!(result.series_count(), 3, "Should return all available series when k > count");
}

#[tokio::test]
async fn test_topk_empty_data() {
    let store = create_test_store();
    let executor = QueryExecutor::new(store.clone());

    // No data written

    let vector_plan = PlanType::VectorQuery(VectorQueryPlan {
        name: Some("nonexistent_metric".to_string()),
        matchers: vec![("__name__".to_string(), "nonexistent_metric".to_string())],
        at: None,
        offset: None,
    });

    let call_plan = PlanType::Call(CallPlan {
        func: Function::TopK,
        args: vec![
            QueryPlan {
                plan_type: PlanType::VectorQuery(VectorQueryPlan {
                    name: None,
                    matchers: vec![],
                    at: None,
                    offset: None,
                }),
                start: 0,
                end: 2000,
                step: 1000,
            },
            QueryPlan {
                plan_type: vector_plan,
                start: 0,
                end: 2000,
                step: 1000,
            },
        ],
        k: Some(5),
        quantile: None,
    });

    let plan = QueryPlan {
        plan_type: call_plan,
        start: 0,
        end: 2000,
        step: 1000,
    };

    let result = executor.execute(&plan).await.unwrap();

    // Empty data should return empty result
    assert_eq!(result.series_count(), 0, "Empty data should return empty result");
}
