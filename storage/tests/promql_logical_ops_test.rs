use chronodb_storage::query::{parse_promql, QueryPlanner};
use chronodb_storage::query::parser::BinaryOp;

#[test]
fn test_parse_and_operator() {
    let expr = parse_promql("up == 1 and on (job) up == 1").unwrap();
    
    match &expr.expr_type {
        chronodb_storage::query::parser::ExprType::BinaryExpr(binary) => {
            assert_eq!(binary.op, BinaryOp::And);
        }
        _ => panic!("Expected BinaryExpr expression"),
    }
}

#[test]
fn test_parse_or_operator() {
    let expr = parse_promql("up == 0 or on (instance) up == 0").unwrap();
    
    match &expr.expr_type {
        chronodb_storage::query::parser::ExprType::BinaryExpr(binary) => {
            assert_eq!(binary.op, BinaryOp::Or);
        }
        _ => panic!("Expected BinaryExpr expression"),
    }
}

#[test]
fn test_parse_unless_operator() {
    let expr = parse_promql("http_requests_total unless http_requests_total == 0").unwrap();
    
    match &expr.expr_type {
        chronodb_storage::query::parser::ExprType::BinaryExpr(binary) => {
            assert_eq!(binary.op, BinaryOp::Unless);
        }
        _ => panic!("Expected BinaryExpr expression"),
    }
}

#[test]
fn test_plan_and_operator() {
    let planner = QueryPlanner::new();
    let expr = parse_promql("metric1 and metric2").unwrap();
    let plan = planner.plan(&expr, 0, 1000, 100).unwrap();
    
    match plan.plan_type {
        chronodb_storage::query::planner::PlanType::BinaryExpr(binary) => {
            assert_eq!(binary.op, BinaryOp::And);
        }
        _ => panic!("Expected BinaryExpr plan"),
    }
}

#[test]
fn test_plan_or_operator() {
    let planner = QueryPlanner::new();
    let expr = parse_promql("metric1 or metric2").unwrap();
    let plan = planner.plan(&expr, 0, 1000, 100).unwrap();
    
    match plan.plan_type {
        chronodb_storage::query::planner::PlanType::BinaryExpr(binary) => {
            assert_eq!(binary.op, BinaryOp::Or);
        }
        _ => panic!("Expected BinaryExpr plan"),
    }
}

#[test]
fn test_plan_unless_operator() {
    let planner = QueryPlanner::new();
    let expr = parse_promql("metric1 unless metric2").unwrap();
    let plan = planner.plan(&expr, 0, 1000, 100).unwrap();
    
    match plan.plan_type {
        chronodb_storage::query::planner::PlanType::BinaryExpr(binary) => {
            assert_eq!(binary.op, BinaryOp::Unless);
        }
        _ => panic!("Expected BinaryExpr plan"),
    }
}

#[test]
fn test_parse_complex_logical_expr() {
    let expr = parse_promql("(up == 1 and on (job) process_start_time_seconds) or on (instance) up == 0").unwrap();
    
    // This should parse successfully
    match &expr.expr_type {
        chronodb_storage::query::parser::ExprType::BinaryExpr(binary) => {
            assert_eq!(binary.op, BinaryOp::Or);
        }
        _ => panic!("Expected BinaryExpr expression"),
    }
}

#[test]
fn test_parse_and_with_vector_matching() {
    let expr = parse_promql("node_cpu_seconds_total and ignoring(cpu) node_cpu_guest_seconds_total").unwrap();
    
    match &expr.expr_type {
        chronodb_storage::query::parser::ExprType::BinaryExpr(binary) => {
            assert_eq!(binary.op, BinaryOp::And);
            // Check vector matching options
            assert!(binary.matching.is_some());
        }
        _ => panic!("Expected BinaryExpr expression"),
    }
}
