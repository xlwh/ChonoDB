use crate::error::{Error, Result};
use crate::query::Expr;
use crate::query::parser::{ExprType, VectorSelector, MatrixSelector, Call, Function};
use std::fmt;

#[derive(Debug, Clone, Default)]
pub struct QueryPlanner;

#[derive(Debug, Clone)]
pub struct QueryPlan {
    pub plan_type: PlanType,
    pub start: i64,
    pub end: i64,
    pub step: i64,
}

#[derive(Debug, Clone)]
pub enum PlanType {
    VectorQuery(VectorQueryPlan),
    MatrixQuery(MatrixQueryPlan),
    Call(CallPlan),
    BinaryExpr(BinaryExprPlan),
    UnaryExpr(UnaryExprPlan),
    Aggregation(AggregationPlan),
}

#[derive(Debug, Clone)]
pub struct VectorQueryPlan {
    pub name: Option<String>,
    pub matchers: Vec<(String, String)>,
}

#[derive(Debug, Clone)]
pub struct MatrixQueryPlan {
    pub vector_plan: VectorQueryPlan,
    pub range: i64,
}

#[derive(Debug, Clone)]
pub struct CallPlan {
    pub func: Function,
    pub args: Vec<QueryPlan>,
}

#[derive(Debug, Clone)]
pub struct BinaryExprPlan {
    pub op: crate::query::parser::BinaryOp,
    pub lhs: Box<QueryPlan>,
    pub rhs: Box<QueryPlan>,
}

#[derive(Debug, Clone)]
pub struct UnaryExprPlan {
    pub op: crate::query::parser::UnaryOp,
    pub expr: Box<QueryPlan>,
}

#[derive(Debug, Clone)]
pub struct AggregationPlan {
    pub op: Function,
    pub expr: Box<QueryPlan>,
    pub grouping: Vec<String>,
    pub without: bool,
}

impl fmt::Display for QueryPlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.plan_type {
            PlanType::VectorQuery(plan) => write!(f, "VectorQuery({:?})
", plan),
            PlanType::MatrixQuery(plan) => write!(f, "MatrixQuery({:?})
", plan),
            PlanType::Call(plan) => write!(f, "Call({:?})
", plan),
            PlanType::BinaryExpr(plan) => write!(f, "BinaryExpr({:?})
", plan),
            PlanType::UnaryExpr(plan) => write!(f, "UnaryExpr({:?})
", plan),
            PlanType::Aggregation(plan) => write!(f, "Aggregation({:?})
", plan),
        }
    }
}

impl QueryPlanner {
    pub fn new() -> Self {
        Self {}
    }

    pub fn plan(&self, expr: &Expr, start: i64, end: i64, step: i64) -> Result<QueryPlan> {
        let plan_type = self.plan_expr(expr)?;
        Ok(QueryPlan {
            plan_type,
            start,
            end,
            step,
        })
    }

    fn plan_expr(&self, expr: &Expr) -> Result<PlanType> {
        match &expr.expr_type {
            ExprType::VectorSelector(vs) => self.plan_vector_selector(vs),
            ExprType::MatrixSelector(ms) => self.plan_matrix_selector(ms),
            ExprType::Call(call) => self.plan_call(call),
            ExprType::BinaryExpr(bin) => self.plan_binary_expr(bin),
            ExprType::UnaryExpr(unary) => self.plan_unary_expr(unary),
            ExprType::Aggregation(agg) => self.plan_aggregation(agg),
            ExprType::NumberLiteral(_) | ExprType::StringLiteral(_) => {
                Err(Error::InvalidData("Literal expression not supported".to_string()))
            }
        }
    }

    fn plan_vector_selector(&self, vs: &VectorSelector) -> Result<PlanType> {
        let mut matchers = Vec::new();
        
        if let Some(name) = &vs.name {
            matchers.push(("__name__".to_string(), name.clone()));
        }
        
        for matcher in &vs.matchers.matchers {
            match matcher.op {
                crate::query::parser::MatchOp::Equal => {
                    matchers.push((matcher.name.clone(), matcher.value.clone()));
                }
                _ => {
                    return Err(Error::InvalidData("Only equal matchers supported".to_string()));
                }
            }
        }
        
        Ok(PlanType::VectorQuery(VectorQueryPlan {
            name: vs.name.clone(),
            matchers,
        }))
    }

    fn plan_matrix_selector(&self, ms: &MatrixSelector) -> Result<PlanType> {
        let vector_plan = self.plan_vector_selector(&ms.vector_selector)?;
        
        match vector_plan {
            PlanType::VectorQuery(vector_plan) => Ok(PlanType::MatrixQuery(MatrixQueryPlan {
                vector_plan,
                range: ms.range,
            })),
            _ => Err(Error::InvalidData("Expected vector selector".to_string())),
        }
    }

    fn plan_call(&self, call: &Call) -> Result<PlanType> {
        let mut args = Vec::new();
        for arg in &call.args {
            let arg_plan = self.plan_expr(arg)?;
            args.push(QueryPlan {
                plan_type: arg_plan,
                start: 0, // Will be filled in by parent plan
                end: 0,
                step: 0,
            });
        }
        
        Ok(PlanType::Call(CallPlan {
            func: call.func.clone(),
            args,
        }))
    }

    fn plan_binary_expr(&self, bin: &crate::query::parser::BinaryExpr) -> Result<PlanType> {
        let lhs_plan = self.plan_expr(&bin.lhs)?;
        let rhs_plan = self.plan_expr(&bin.rhs)?;
        
        Ok(PlanType::BinaryExpr(BinaryExprPlan {
            op: bin.op,
            lhs: Box::new(QueryPlan {
                plan_type: lhs_plan,
                start: 0,
                end: 0,
                step: 0,
            }),
            rhs: Box::new(QueryPlan {
                plan_type: rhs_plan,
                start: 0,
                end: 0,
                step: 0,
            }),
        }))
    }

    fn plan_unary_expr(&self, unary: &crate::query::parser::UnaryExpr) -> Result<PlanType> {
        let expr_plan = self.plan_expr(&unary.expr)?;
        
        Ok(PlanType::UnaryExpr(UnaryExprPlan {
            op: unary.op,
            expr: Box::new(QueryPlan {
                plan_type: expr_plan,
                start: 0,
                end: 0,
                step: 0,
            }),
        }))
    }

    fn plan_aggregation(&self, agg: &crate::query::parser::Aggregation) -> Result<PlanType> {
        let expr_plan = self.plan_expr(&agg.expr)?;
        
        Ok(PlanType::Aggregation(AggregationPlan {
            op: agg.op.clone(),
            expr: Box::new(QueryPlan {
                plan_type: expr_plan,
                start: 0,
                end: 0,
                step: 0,
            }),
            grouping: agg.grouping.clone(),
            without: agg.without,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::query::parse_promql;

    #[test]
    fn test_plan_vector_selector() {
        let planner = QueryPlanner::new();
        let expr = parse_promql("http_requests_total{job=\"prometheus\"}").unwrap();
        let plan = planner.plan(&expr, 0, 1000, 100).unwrap();
        
        match plan.plan_type {
            PlanType::VectorQuery(vq) => {
                assert_eq!(vq.name, Some("http_requests_total".to_string()));
                assert_eq!(vq.matchers.len(), 2);
            }
            _ => panic!("Expected VectorQuery"),
        }
    }

    #[test]
    fn test_plan_matrix_selector() {
        let planner = QueryPlanner::new();
        let expr = parse_promql("http_requests_total[5m]").unwrap();
        let plan = planner.plan(&expr, 0, 1000, 100).unwrap();
        
        match plan.plan_type {
            PlanType::MatrixQuery(mq) => {
                assert_eq!(mq.range, 300000); // 5 minutes in milliseconds
                assert_eq!(mq.vector_plan.name, Some("http_requests_total".to_string()));
            }
            _ => panic!("Expected MatrixQuery"),
        }
    }

    #[test]
    fn test_plan_call() {
        let planner = QueryPlanner::new();
        let expr = parse_promql("rate(http_requests_total[5m])").unwrap();
        let plan = planner.plan(&expr, 0, 1000, 100).unwrap();
        
        match plan.plan_type {
            PlanType::Call(call) => {
                assert_eq!(call.func.name(), "rate");
                assert_eq!(call.args.len(), 1);
            }
            _ => panic!("Expected Call"),
        }
    }
}
