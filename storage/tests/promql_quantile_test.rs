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
async fn test_quantile_median() {
    let store = create_test_store();
    let executor = QueryExecutor::new(store.clone());

    // Create test data with known values: 10, 20, 30, 40, 50
    for i in 1..=5 {
        let labels = vec![
            Label::new("__name__", "response_time"),
            Label::new("instance", &format!("server{}", i)),
        ];
        store.write(labels, vec![Sample::new(1000, i as f64 * 10.0)]).unwrap();
    }

    let vector_plan = PlanType::VectorQuery(VectorQueryPlan {
        name: Some("response_time".to_string()),
        matchers: vec![("__name__".to_string(), "response_time".to_string())],
        at: None,
        offset: None,
    });

    let call_plan = PlanType::Call(CallPlan {
        func: Function::Quantile,
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
        k: None,
        quantile: Some(0.5), // Median
    });

    let plan = QueryPlan {
        plan_type: call_plan,
        start: 0,
        end: 2000,
        step: 1000,
    };

    let result = executor.execute(&plan).await.unwrap();

    assert_eq!(result.series_count(), 1);
    // Median of [10, 20, 30, 40, 50] is 30
    assert_eq!(result.series[0].samples[0].value, 30.0);
}

#[tokio::test]
async fn test_quantile_95th() {
    let store = create_test_store();
    let executor = QueryExecutor::new(store.clone());

    // Create 100 values from 1 to 100
    for i in 1..=100 {
        let labels = vec![
            Label::new("__name__", "request_duration"),
            Label::new("instance", &format!("server{}", i)),
        ];
        store.write(labels, vec![Sample::new(1000, i as f64)]).unwrap();
    }

    let vector_plan = PlanType::VectorQuery(VectorQueryPlan {
        name: Some("request_duration".to_string()),
        matchers: vec![("__name__".to_string(), "request_duration".to_string())],
        at: None,
        offset: None,
    });

    let call_plan = PlanType::Call(CallPlan {
        func: Function::Quantile,
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
        k: None,
        quantile: Some(0.95),
    });

    let plan = QueryPlan {
        plan_type: call_plan,
        start: 0,
        end: 2000,
        step: 1000,
    };

    let result = executor.execute(&plan).await.unwrap();

    assert_eq!(result.series_count(), 1);
    // 95th percentile of 1..100 should be around 95-96
    let value = result.series[0].samples[0].value;
    assert!(value >= 95.0 && value <= 96.0, "95th percentile should be around 95-96, got {}", value);
}

#[tokio::test]
async fn test_quantile_boundary_values() {
    let store = create_test_store();
    let executor = QueryExecutor::new(store.clone());

    // Create test data
    for i in 1..=5 {
        let labels = vec![
            Label::new("__name__", "metric"),
            Label::new("instance", &format!("server{}", i)),
        ];
        store.write(labels, vec![Sample::new(1000, i as f64 * 10.0)]).unwrap();
    }

    // Test quantile 0 (min)
    let vector_plan = PlanType::VectorQuery(VectorQueryPlan {
        name: Some("metric".to_string()),
        matchers: vec![("__name__".to_string(), "metric".to_string())],
        at: None,
        offset: None,
    });

    let call_plan_0 = PlanType::Call(CallPlan {
        func: Function::Quantile,
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
                plan_type: vector_plan.clone(),
                start: 0,
                end: 2000,
                step: 1000,
            },
        ],
        k: None,
        quantile: Some(0.0),
    });

    let plan_0 = QueryPlan {
        plan_type: call_plan_0,
        start: 0,
        end: 2000,
        step: 1000,
    };

    let result_0 = executor.execute(&plan_0).await.unwrap();
    assert_eq!(result_0.series[0].samples[0].value, 10.0, "Quantile 0 should be min value");

    // Test quantile 1 (max)
    let call_plan_1 = PlanType::Call(CallPlan {
        func: Function::Quantile,
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
        k: None,
        quantile: Some(1.0),
    });

    let plan_1 = QueryPlan {
        plan_type: call_plan_1,
        start: 0,
        end: 2000,
        step: 1000,
    };

    let result_1 = executor.execute(&plan_1).await.unwrap();
    assert_eq!(result_1.series[0].samples[0].value, 50.0, "Quantile 1 should be max value");
}

#[tokio::test]
async fn test_quantile_empty_data() {
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
        func: Function::Quantile,
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
        k: None,
        quantile: Some(0.5),
    });

    let plan = QueryPlan {
        plan_type: call_plan,
        start: 0,
        end: 2000,
        step: 1000,
    };

    let result = executor.execute(&plan).await.unwrap();

    // Empty data should return empty result
    assert_eq!(result.series_count(), 0);
}

#[tokio::test]
async fn test_quantile_single_value() {
    let store = create_test_store();
    let executor = QueryExecutor::new(store.clone());

    // Only one value
    let labels = vec![
        Label::new("__name__", "single_metric"),
        Label::new("instance", "server1"),
    ];
    store.write(labels, vec![Sample::new(1000, 42.0)]).unwrap();

    let vector_plan = PlanType::VectorQuery(VectorQueryPlan {
        name: Some("single_metric".to_string()),
        matchers: vec![("__name__".to_string(), "single_metric".to_string())],
        at: None,
        offset: None,
    });

    let call_plan = PlanType::Call(CallPlan {
        func: Function::Quantile,
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
        k: None,
        quantile: Some(0.5),
    });

    let plan = QueryPlan {
        plan_type: call_plan,
        start: 0,
        end: 2000,
        step: 1000,
    };

    let result = executor.execute(&plan).await.unwrap();

    assert_eq!(result.series_count(), 1);
    assert_eq!(result.series[0].samples[0].value, 42.0);
}

#[tokio::test]
async fn test_quantile_interpolation() {
    let store = create_test_store();
    let executor = QueryExecutor::new(store.clone());

    // Create 4 values: 10, 20, 30, 40
    for i in 1..=4 {
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

    // Test 0.25 quantile (should interpolate between 10 and 20)
    let call_plan = PlanType::Call(CallPlan {
        func: Function::Quantile,
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
        k: None,
        quantile: Some(0.25),
    });

    let plan = QueryPlan {
        plan_type: call_plan,
        start: 0,
        end: 2000,
        step: 1000,
    };

    let result = executor.execute(&plan).await.unwrap();

    assert_eq!(result.series_count(), 1);
    // 0.25 quantile of [10, 20, 30, 40] with linear interpolation
    // position = 0.25 * 3 = 0.75
    // result = 10 * (1 - 0.75) + 20 * 0.75 = 2.5 + 15 = 17.5
    let value = result.series[0].samples[0].value;
    assert!((value - 17.5).abs() < 0.01, "Expected ~17.5, got {}", value);
}
