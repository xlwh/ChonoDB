use chronodb_storage::query::{parse_promql, QueryPlanner};
use chronodb_storage::query::parser::Function;

#[test]
fn test_parse_scalar_function() {
    let expr = parse_promql("scalar(up)").unwrap();
    
    match &expr.expr_type {
        chronodb_storage::query::parser::ExprType::Call(call) => {
            assert_eq!(call.func, Function::Scalar);
            assert_eq!(call.args.len(), 1);
        }
        _ => panic!("Expected Call expression"),
    }
}

#[test]
fn test_parse_vector_function() {
    let expr = parse_promql("vector(1)").unwrap();
    
    match &expr.expr_type {
        chronodb_storage::query::parser::ExprType::Call(call) => {
            assert_eq!(call.func, Function::Vector);
            assert_eq!(call.args.len(), 1);
        }
        _ => panic!("Expected Call expression"),
    }
}

#[test]
fn test_plan_scalar_function() {
    let planner = QueryPlanner::new();
    let expr = parse_promql("scalar(up)").unwrap();
    let plan = planner.plan(&expr, 0, 1000, 100).unwrap();
    
    match plan.plan_type {
        chronodb_storage::query::planner::PlanType::Call(call) => {
            assert_eq!(call.func, Function::Scalar);
        }
        _ => panic!("Expected Call plan"),
    }
}

#[test]
fn test_plan_vector_function() {
    let planner = QueryPlanner::new();
    let expr = parse_promql("vector(42)").unwrap();
    let plan = planner.plan(&expr, 0, 1000, 100).unwrap();
    
    match plan.plan_type {
        chronodb_storage::query::planner::PlanType::Call(call) => {
            assert_eq!(call.func, Function::Vector);
        }
        _ => panic!("Expected Call plan"),
    }
}

#[test]
fn test_parse_scalar_with_expression() {
    let expr = parse_promql("scalar(sum by (job) (up))").unwrap();
    
    match &expr.expr_type {
        chronodb_storage::query::parser::ExprType::Call(call) => {
            assert_eq!(call.func, Function::Scalar);
            // The argument should be an aggregation
            match &call.args[0].expr_type {
                chronodb_storage::query::parser::ExprType::Aggregation(agg) => {
                    assert_eq!(agg.op, Function::Sum);
                }
                _ => panic!("Expected Aggregation as argument"),
            }
        }
        _ => panic!("Expected Call expression"),
    }
}

#[test]
fn test_parse_vector_with_arithmetic() {
    let expr = parse_promql("vector(1 + 2)").unwrap();
    
    match &expr.expr_type {
        chronodb_storage::query::parser::ExprType::Call(call) => {
            assert_eq!(call.func, Function::Vector);
            // The argument should be a binary expression
            match &call.args[0].expr_type {
                chronodb_storage::query::parser::ExprType::BinaryExpr(_) => {
                    // Binary expression found
                }
                _ => panic!("Expected BinaryExpr as argument"),
            }
        }
        _ => panic!("Expected Call expression"),
    }
}

#[test]
fn test_parse_complex_scalar_usage() {
    // scalar() is often used in comparisons
    let expr = parse_promql("up == scalar(sum(up) / count(up))").unwrap();
    
    match &expr.expr_type {
        chronodb_storage::query::parser::ExprType::BinaryExpr(binary) => {
            assert_eq!(binary.op, chronodb_storage::query::parser::BinaryOp::Eq);
        }
        _ => panic!("Expected BinaryExpr expression"),
    }
}
