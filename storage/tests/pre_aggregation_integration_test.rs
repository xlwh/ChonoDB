mod pre_aggregation_integration_test {
    use chronodb_storage::model::{PreAggregationRule, RuleStatus};
    use chronodb_storage::query::{FrequencyTracker, FrequencyConfig, normalize_query, QueryEngine};
    use chronodb_storage::memstore::MemStore;
    use chronodb_storage::config::StorageConfig;
    use chronodb_storage::distributed::{DistributedPreAggregationCoordinator, DistributedPreAggregationConfig, TaskStatus};
    use std::sync::Arc;
    use std::collections::HashMap;
    use tempfile::tempdir;

    async fn create_test_environment() -> (Arc<QueryEngine>, Arc<FrequencyTracker>) {
        let temp_dir = tempdir().unwrap();
        let config = StorageConfig {
            data_dir: temp_dir.path().to_string_lossy().to_string(),
            ..Default::default()
        };
        
        let memstore = Arc::new(MemStore::new(config).unwrap());
        let query_engine = Arc::new(QueryEngine::new(memstore));
        
        let frequency_config = FrequencyConfig {
            window_size_hours: 24,
            frequency_threshold: 20,
            cleanup_interval_hours: 1,
            max_tracked_queries: 100,
        };
        let frequency_tracker = Arc::new(FrequencyTracker::new(frequency_config));
        
        (query_engine, frequency_tracker)
    }

    #[tokio::test]
    async fn test_pre_aggregation_rule_creation() {
        let rule = PreAggregationRule::new(
            "rule-1".to_string(),
            "test_rule".to_string(),
            "sum(rate(http_requests_total[5m]))".to_string(),
            HashMap::new(),
            false,
        );
        
        assert_eq!(rule.id, "rule-1");
        assert_eq!(rule.name, "test_rule");
        assert_eq!(rule.status, RuleStatus::Pending);
        assert!(!rule.is_auto_created);
    }

    #[tokio::test]
    async fn test_query_frequency_tracking() {
        let config = FrequencyConfig {
            window_size_hours: 1,
            frequency_threshold: 5,
            cleanup_interval_hours: 1,
            max_tracked_queries: 100,
        };
        let tracker = FrequencyTracker::new(config);
        
        for _ in 0..10 {
            tracker.record_query("up");
        }
        
        assert_eq!(tracker.get_frequency("up"), 10);
        
        let high_freq = tracker.get_high_frequency_queries();
        assert_eq!(high_freq.len(), 1);
        assert_eq!(high_freq[0].0, "up");
        assert_eq!(high_freq[0].1, 10);
    }

    #[tokio::test]
    async fn test_query_normalization() {
        let query1 = "sum(rate(http_requests_total{job=\"api\"}[5m])) by (status)";
        let query2 = "sum(rate(http_requests_total{job=\"api\"}[10m])) by (status)";
        
        let normalized1 = normalize_query(query1);
        let normalized2 = normalize_query(query2);
        
        // 不同的时间范围会被标准化为相同的 [DURATION]
        assert_eq!(normalized1, normalized2);
        assert!(normalized1.contains("[DURATION]"));
        
        let query3 = "sum(rate(http_requests_total{job=\"api\"}[5m])) by (status)";
        let normalized3 = normalize_query(query3);
        assert_eq!(normalized1, normalized3);
        
        // 不同的查询应该产生不同的标准化结果
        let query4 = "avg(cpu_usage)";
        let normalized4 = normalize_query(query4);
        assert_ne!(normalized1, normalized4);
    }

    #[tokio::test]
    async fn test_distributed_coordination() {
        let coordinator = Arc::new(DistributedPreAggregationCoordinator::new(
            "node-1".to_string(),
            DistributedPreAggregationConfig::default(),
        ));
        
        coordinator.start().await.unwrap();
        
        let rule = PreAggregationRule::new(
            "rule-1".to_string(),
            "test_rule".to_string(),
            "up".to_string(),
            HashMap::new(),
            false,
        );
        
        let available_nodes = vec!["node-1".to_string(), "node-2".to_string()];
        
        let assigned_node = coordinator.assign_task(&rule, &available_nodes).await.unwrap();
        assert!(!assigned_node.is_empty());
        
        let assignment = coordinator.get_task_assignment("rule-1").await;
        assert!(assignment.is_some());
        
        coordinator.update_task_status("rule-1", TaskStatus::Running).await.unwrap();
        
        let updated_assignment = coordinator.get_task_assignment("rule-1").await.unwrap();
        assert_eq!(updated_assignment.status, TaskStatus::Running);
    }

    #[tokio::test]
    async fn test_frequency_threshold() {
        let config = FrequencyConfig {
            window_size_hours: 1,
            frequency_threshold: 3,
            cleanup_interval_hours: 1,
            max_tracked_queries: 100,
        };
        let tracker = FrequencyTracker::new(config);
        
        tracker.record_query("query1");
        tracker.record_query("query1");
        tracker.record_query("query1");
        
        tracker.record_query("query2");
        tracker.record_query("query2");
        
        let high_freq = tracker.get_high_frequency_queries();
        assert_eq!(high_freq.len(), 1);
        assert_eq!(high_freq[0].0, "query1");
    }

    #[tokio::test]
    async fn test_rule_status_management() {
        let mut rule = PreAggregationRule::new(
            "rule-1".to_string(),
            "test_rule".to_string(),
            "up".to_string(),
            HashMap::new(),
            false,
        );
        
        assert_eq!(rule.status, RuleStatus::Pending);
        
        rule.set_status(RuleStatus::Active);
        assert_eq!(rule.status, RuleStatus::Active);
        
        rule.update_query_frequency();
        assert_eq!(rule.query_frequency, 1);
        
        rule.update_evaluation();
        assert!(rule.last_evaluation > 0);
    }

    #[tokio::test]
    async fn test_distributed_task_assignment() {
        let coordinator = DistributedPreAggregationCoordinator::new(
            "node-1".to_string(),
            DistributedPreAggregationConfig::default(),
        );
        
        let rule1 = PreAggregationRule::new(
            "rule-1".to_string(),
            "test_rule1".to_string(),
            "up".to_string(),
            HashMap::new(),
            false,
        );
        
        let rule2 = PreAggregationRule::new(
            "rule-2".to_string(),
            "test_rule2".to_string(),
            "down".to_string(),
            HashMap::new(),
            false,
        );
        
        let available_nodes = vec!["node-1".to_string(), "node-2".to_string()];
        
        let node1 = coordinator.assign_task(&rule1, &available_nodes).await.unwrap();
        let node2 = coordinator.assign_task(&rule2, &available_nodes).await.unwrap();
        
        assert!(!node1.is_empty());
        assert!(!node2.is_empty());
        
        let stats = coordinator.get_coordination_stats().await;
        assert_eq!(stats.total_tasks, 2);
    }

    #[tokio::test]
    async fn test_distributed_node_failure_handling() {
        let coordinator = DistributedPreAggregationCoordinator::new(
            "node-1".to_string(),
            DistributedPreAggregationConfig::default(),
        );
        
        let rule = PreAggregationRule::new(
            "rule-1".to_string(),
            "test_rule".to_string(),
            "up".to_string(),
            HashMap::new(),
            false,
        );
        
        let available_nodes = vec!["node-2".to_string()];
        coordinator.assign_task(&rule, &available_nodes).await.unwrap();
        
        let affected_rules = coordinator.handle_node_failure("node-2").await.unwrap();
        
        assert_eq!(affected_rules.len(), 1);
        assert_eq!(affected_rules[0], "rule-1");
    }

    #[tokio::test]
    async fn test_frequency_per_hour_calculation() {
        let config = FrequencyConfig {
            window_size_hours: 24,
            frequency_threshold: 20,
            cleanup_interval_hours: 1,
            max_tracked_queries: 100,
        };
        let tracker = FrequencyTracker::new(config);
        
        for _ in 0..24 {
            tracker.record_query("query1");
        }
        
        let freq_per_hour = tracker.get_frequency_per_hour("query1");
        
        assert!(freq_per_hour > 0.0);
        assert!(freq_per_hour <= 1.0);
    }

    #[tokio::test]
    async fn test_multiple_queries_frequency() {
        let tracker = FrequencyTracker::new(FrequencyConfig::default());
        
        tracker.record_query("query1");
        tracker.record_query("query1");
        tracker.record_query("query2");
        tracker.record_query("query3");
        
        assert_eq!(tracker.get_frequency("query1"), 2);
        assert_eq!(tracker.get_frequency("query2"), 1);
        assert_eq!(tracker.get_frequency("query3"), 1);
        
        let all_stats = tracker.get_all_stats();
        assert_eq!(all_stats.len(), 3);
    }

    #[tokio::test]
    async fn test_task_heartbeat() {
        let coordinator = DistributedPreAggregationCoordinator::new(
            "node-1".to_string(),
            DistributedPreAggregationConfig::default(),
        );
        
        let rule = PreAggregationRule::new(
            "rule-1".to_string(),
            "test_rule".to_string(),
            "up".to_string(),
            HashMap::new(),
            false,
        );
        
        let available_nodes = vec!["node-1".to_string()];
        coordinator.assign_task(&rule, &available_nodes).await.unwrap();
        
        coordinator.heartbeat("rule-1").await.unwrap();
        
        let assignment = coordinator.get_task_assignment("rule-1").await.unwrap();
        assert!(assignment.last_heartbeat > 0);
    }

    #[tokio::test]
    async fn test_task_removal() {
        let coordinator = DistributedPreAggregationCoordinator::new(
            "node-1".to_string(),
            DistributedPreAggregationConfig::default(),
        );
        
        let rule = PreAggregationRule::new(
            "rule-1".to_string(),
            "test_rule".to_string(),
            "up".to_string(),
            HashMap::new(),
            false,
        );
        
        let available_nodes = vec!["node-1".to_string()];
        coordinator.assign_task(&rule, &available_nodes).await.unwrap();
        
        coordinator.remove_task("rule-1").await.unwrap();
        
        let assignment = coordinator.get_task_assignment("rule-1").await;
        assert!(assignment.is_none());
    }
}
