use chronodb_storage::query::parser::parse_promql;

#[tokio::test]
async fn test_subquery_parsing() {
    // Test parsing subquery syntax: <expr> [<range>:<resolution>]
    let expr = parse_promql("rate(http_requests_total[5m])[30m:1m]").unwrap();
    
    match &expr.expr_type {
        chronodb_storage::query::parser::ExprType::Subquery(sq) => {
            assert_eq!(sq.range, 30 * 60 * 1000); // 30m in ms
            assert_eq!(sq.resolution, 60 * 1000); // 1m in ms
            
            // Check inner expression is a Call (rate function)
            match &sq.expr.expr_type {
                chronodb_storage::query::parser::ExprType::Call(call) => {
                    assert!(matches!(call.func, chronodb_storage::query::parser::Function::Rate));
                }
                _ => panic!("Expected Call expression inside subquery"),
            }
        }
        _ => panic!("Expected Subquery expression"),
    }
}

#[tokio::test]
async fn test_subquery_simple_vector() {
    // Test simple vector subquery: metric[30m:1m]
    let expr = parse_promql("http_requests_total[30m:1m]").unwrap();
    
    match &expr.expr_type {
        chronodb_storage::query::parser::ExprType::Subquery(sq) => {
            assert_eq!(sq.range, 30 * 60 * 1000);
            assert_eq!(sq.resolution, 60 * 1000);
            
            // Check inner expression is a VectorSelector
            match &sq.expr.expr_type {
                chronodb_storage::query::parser::ExprType::VectorSelector(vs) => {
                    assert_eq!(vs.name, Some("http_requests_total".to_string()));
                }
                _ => panic!("Expected VectorSelector inside subquery"),
            }
        }
        _ => panic!("Expected Subquery expression"),
    }
}

#[tokio::test]
async fn test_subquery_with_labels() {
    // Test subquery with label selectors
    let expr = parse_promql("http_requests_total{job=\"prometheus\"}[30m:1m]").unwrap();
    
    match &expr.expr_type {
        chronodb_storage::query::parser::ExprType::Subquery(sq) => {
            assert_eq!(sq.range, 30 * 60 * 1000);
            assert_eq!(sq.resolution, 60 * 1000);
        }
        _ => panic!("Expected Subquery expression"),
    }
}

#[tokio::test]
async fn test_subquery_different_resolutions() {
    // Test different resolution values
    let expr_5m = parse_promql("metric[1h:5m]").unwrap();
    match &expr_5m.expr_type {
        chronodb_storage::query::parser::ExprType::Subquery(sq) => {
            assert_eq!(sq.range, 60 * 60 * 1000); // 1h
            assert_eq!(sq.resolution, 5 * 60 * 1000); // 5m
        }
        _ => panic!("Expected Subquery expression"),
    }

    let expr_10s = parse_promql("metric[5m:10s]").unwrap();
    match &expr_10s.expr_type {
        chronodb_storage::query::parser::ExprType::Subquery(sq) => {
            assert_eq!(sq.range, 5 * 60 * 1000); // 5m
            assert_eq!(sq.resolution, 10 * 1000); // 10s
        }
        _ => panic!("Expected Subquery expression"),
    }
}
