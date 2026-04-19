use chronodb_storage::query::{parse_promql, QueryPlanner};
use chronodb_storage::query::planner::{PlanType, AggregationPlan};
use chronodb_storage::query::parser::Function;

#[test]
fn test_parse_sum_by() {
    let expr = parse_promql("sum by (job) (http_requests_total)").unwrap();
    
    match &expr.expr_type {
        chronodb_storage::query::parser::ExprType::Aggregation(agg) => {
            assert_eq!(agg.op, Function::Sum);
            assert_eq!(agg.grouping, vec!["job"]);
            assert_eq!(agg.without, false);
        }
        _ => panic!("Expected Aggregation expression"),
    }
}

#[test]
fn test_parse_avg_without() {
    let expr = parse_promql("avg without (instance) (http_requests_total)").unwrap();
    
    match &expr.expr_type {
        chronodb_storage::query::parser::ExprType::Aggregation(agg) => {
            assert_eq!(agg.op, Function::Avg);
            assert_eq!(agg.grouping, vec!["instance"]);
            assert_eq!(agg.without, true);
        }
        _ => panic!("Expected Aggregation expression"),
    }
}

#[test]
fn test_plan_sum_by() {
    let planner = QueryPlanner::new();
    let expr = parse_promql("sum by (job) (http_requests_total)").unwrap();
    let plan = planner.plan(&expr, 0, 1000, 100).unwrap();
    
    match plan.plan_type {
        PlanType::Aggregation(agg) => {
            assert_eq!(agg.op, Function::Sum);
            assert_eq!(agg.grouping, vec!["job"]);
            assert_eq!(agg.without, false);
        }
        _ => panic!("Expected Aggregation plan"),
    }
}

#[test]
fn test_plan_count_without() {
    let planner = QueryPlanner::new();
    let expr = parse_promql("count without (instance, job) (up)").unwrap();
    let plan = planner.plan(&expr, 0, 1000, 100).unwrap();
    
    match plan.plan_type {
        PlanType::Aggregation(agg) => {
            assert_eq!(agg.op, Function::Count);
            assert_eq!(agg.grouping, vec!["instance", "job"]);
            assert_eq!(agg.without, true);
        }
        _ => panic!("Expected Aggregation plan"),
    }
}

#[test]
fn test_parse_max_by_multiple_labels() {
    let expr = parse_promql("max by (job, instance) (cpu_usage)").unwrap();
    
    match &expr.expr_type {
        chronodb_storage::query::parser::ExprType::Aggregation(agg) => {
            assert_eq!(agg.op, Function::Max);
            assert_eq!(agg.grouping, vec!["job", "instance"]);
            assert_eq!(agg.without, false);
        }
        _ => panic!("Expected Aggregation expression"),
    }
}

#[test]
fn test_parse_min_without_multiple_labels() {
    let expr = parse_promql("min without (pod, container) (memory_usage)").unwrap();
    
    match &expr.expr_type {
        chronodb_storage::query::parser::ExprType::Aggregation(agg) => {
            assert_eq!(agg.op, Function::Min);
            assert_eq!(agg.grouping, vec!["pod", "container"]);
            assert_eq!(agg.without, true);
        }
        _ => panic!("Expected Aggregation expression"),
    }
}

#[test]
fn test_plan_stddev_by() {
    let planner = QueryPlanner::new();
    let expr = parse_promql("stddev by (cluster) (request_duration_seconds)").unwrap();
    let plan = planner.plan(&expr, 0, 1000, 100).unwrap();
    
    match plan.plan_type {
        PlanType::Aggregation(agg) => {
            assert_eq!(agg.op, Function::Stddev);
            assert_eq!(agg.grouping, vec!["cluster"]);
            assert_eq!(agg.without, false);
        }
        _ => panic!("Expected Aggregation plan"),
    }
}

#[test]
fn test_plan_stdvar_without() {
    let planner = QueryPlanner::new();
    let expr = parse_promql("stdvar without (replica) (temperature_celsius)").unwrap();
    let plan = planner.plan(&expr, 0, 1000, 100).unwrap();
    
    match plan.plan_type {
        PlanType::Aggregation(agg) => {
            assert_eq!(agg.op, Function::Stdvar);
            assert_eq!(agg.grouping, vec!["replica"]);
            assert_eq!(agg.without, true);
        }
        _ => panic!("Expected Aggregation plan"),
    }
}
