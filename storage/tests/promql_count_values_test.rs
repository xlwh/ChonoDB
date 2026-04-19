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
async fn test_count_values_basic() {
    let store = create_test_store();
    let executor = QueryExecutor::new(store.clone());

    // Create test data with different values
    let labels1 = vec![
        Label::new("__name__", "build_version"),
        Label::new("instance", "server1"),
    ];
    let labels2 = vec![
        Label::new("__name__", "build_version"),
        Label::new("instance", "server2"),
    ];
    let labels3 = vec![
        Label::new("__name__", "build_version"),
        Label::new("instance", "server3"),
    ];
    let labels4 = vec![
        Label::new("__name__", "build_version"),
        Label::new("instance", "server4"),
    ];

    // server1 and server2 have version 1.0
    store.write(labels1, vec![Sample::new(1000, 1.0)]).unwrap();
    store.write(labels2, vec![Sample::new(1000, 1.0)]).unwrap();
    // server3 and server4 have version 2.0
    store.write(labels3, vec![Sample::new(1000, 2.0)]).unwrap();
    store.write(labels4, vec![Sample::new(1000, 2.0)]).unwrap();

    // Execute count_values query
    let vector_plan = PlanType::VectorQuery(VectorQueryPlan {
        name: Some("build_version".to_string()),
        matchers: vec![("__name__".to_string(), "build_version".to_string())],
        at: None,
        offset: None,
    });

    // First argument: label name (as a vector query with the label name)
    let label_arg = PlanType::VectorQuery(VectorQueryPlan {
        name: None,
        matchers: vec![("version".to_string(), "version".to_string())],
        at: None,
        offset: None,
    });

    let call_plan = PlanType::Call(CallPlan {
        func: Function::CountValues,
        args: vec![
            QueryPlan {
                plan_type: label_arg,
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
        quantile: None,
    });

    let plan = QueryPlan {
        plan_type: call_plan,
        start: 0,
        end: 2000,
        step: 1000,
    };

    let result = executor.execute(&plan).await.unwrap();

    // Should have 2 series (one for each unique value: 1.0 and 2.0)
    assert_eq!(result.series_count(), 2, "Expected 2 series for 2 unique values");

    // Check that we have counts of 2 for each value
    let mut found_1_0 = false;
    let mut found_2_0 = false;

    for series in &result.series {
        let count = series.samples[0].value;
        if count == 2.0 {
            // Check which value this count is for
            if let Some(label) = series.labels.iter().find(|l| l.name == "version") {
                if label.value == "1" {
                    found_1_0 = true;
                } else if label.value == "2" {
                    found_2_0 = true;
                }
            }
        }
    }

    assert!(found_1_0, "Should have count for value 1.0");
    assert!(found_2_0, "Should have count for value 2.0");
}

#[tokio::test]
async fn test_count_values_single_value() {
    let store = create_test_store();
    let executor = QueryExecutor::new(store.clone());

    // All servers have the same version
    for i in 1..=5 {
        let labels = vec![
            Label::new("__name__", "app_version"),
            Label::new("instance", &format!("server{}", i)),
        ];
        store.write(labels, vec![Sample::new(1000, 3.5)]).unwrap();
    }

    let vector_plan = PlanType::VectorQuery(VectorQueryPlan {
        name: Some("app_version".to_string()),
        matchers: vec![("__name__".to_string(), "app_version".to_string())],
        at: None,
        offset: None,
    });

    let label_arg = PlanType::VectorQuery(VectorQueryPlan {
        name: None,
        matchers: vec![("version".to_string(), "version".to_string())],
        at: None,
        offset: None,
    });

    let call_plan = PlanType::Call(CallPlan {
        func: Function::CountValues,
        args: vec![
            QueryPlan {
                plan_type: label_arg,
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
        quantile: None,
    });

    let plan = QueryPlan {
        plan_type: call_plan,
        start: 0,
        end: 2000,
        step: 1000,
    };

    let result = executor.execute(&plan).await.unwrap();

    // Should have 1 series (only one unique value: 3.5)
    assert_eq!(result.series_count(), 1);
    assert_eq!(result.series[0].samples[0].value, 5.0); // Count should be 5
}

#[tokio::test]
async fn test_count_values_empty_data() {
    let store = create_test_store();
    let executor = QueryExecutor::new(store.clone());

    // No data written

    let vector_plan = PlanType::VectorQuery(VectorQueryPlan {
        name: Some("nonexistent_metric".to_string()),
        matchers: vec![("__name__".to_string(), "nonexistent_metric".to_string())],
        at: None,
        offset: None,
    });

    let label_arg = PlanType::VectorQuery(VectorQueryPlan {
        name: None,
        matchers: vec![("version".to_string(), "version".to_string())],
        at: None,
        offset: None,
    });

    let call_plan = PlanType::Call(CallPlan {
        func: Function::CountValues,
        args: vec![
            QueryPlan {
                plan_type: label_arg,
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
        quantile: None,
    });

    let plan = QueryPlan {
        plan_type: call_plan,
        start: 0,
        end: 2000,
        step: 1000,
    };

    let result = executor.execute(&plan).await.unwrap();

    // Should have 0 series (no data)
    assert_eq!(result.series_count(), 0);
}

#[tokio::test]
async fn test_count_values_many_unique_values() {
    let store = create_test_store();
    let executor = QueryExecutor::new(store.clone());

    // Create 100 series with 10 unique values (10 series per value)
    for i in 0..100 {
        let labels = vec![
            Label::new("__name__", "metric_with_many_values"),
            Label::new("instance", &format!("instance{}", i)),
        ];
        let value = (i % 10) as f64;
        store.write(labels, vec![Sample::new(1000, value)]).unwrap();
    }

    let vector_plan = PlanType::VectorQuery(VectorQueryPlan {
        name: Some("metric_with_many_values".to_string()),
        matchers: vec![("__name__".to_string(), "metric_with_many_values".to_string())],
        at: None,
        offset: None,
    });

    let label_arg = PlanType::VectorQuery(VectorQueryPlan {
        name: None,
        matchers: vec![("value".to_string(), "value".to_string())],
        at: None,
        offset: None,
    });

    let call_plan = PlanType::Call(CallPlan {
        func: Function::CountValues,
        args: vec![
            QueryPlan {
                plan_type: label_arg,
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
        quantile: None,
    });

    let plan = QueryPlan {
        plan_type: call_plan,
        start: 0,
        end: 2000,
        step: 1000,
    };

    let result = executor.execute(&plan).await.unwrap();

    // Should have 10 series (10 unique values: 0-9)
    assert_eq!(result.series_count(), 10);

    // Each value should have count of 10
    for series in &result.series {
        assert_eq!(series.samples[0].value, 10.0, "Each unique value should appear 10 times");
    }
}
