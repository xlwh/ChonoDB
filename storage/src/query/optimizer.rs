use crate::error::Result;
use crate::query::planner::{QueryPlan, PlanType, VectorQueryPlan, MatrixQueryPlan, CallPlan, BinaryExprPlan, UnaryExprPlan, AggregationPlan};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, info};

/// 查询优化器
pub struct QueryOptimizer;

impl QueryOptimizer {
    pub fn new() -> Self {
        Self
    }

    /// 优化查询计划
    pub fn optimize(&self, plan: QueryPlan) -> Result<QueryPlan> {
        info!("Optimizing query plan");
        
        let optimized_plan = match &plan.plan_type {
            PlanType::VectorQuery(query) => {
                let optimized = self.optimize_vector_query(query)?;
                QueryPlan {
                    plan_type: PlanType::VectorQuery(optimized),
                    ..plan
                }
            }
            PlanType::MatrixQuery(query) => {
                let optimized = self.optimize_matrix_query(query)?;
                QueryPlan {
                    plan_type: PlanType::MatrixQuery(optimized),
                    ..plan
                }
            }
            PlanType::Call(call) => {
                let optimized = self.optimize_call(call)?;
                QueryPlan {
                    plan_type: PlanType::Call(optimized),
                    ..plan
                }
            }
            PlanType::BinaryExpr(binary) => {
                let optimized = self.optimize_binary(binary)?;
                QueryPlan {
                    plan_type: PlanType::BinaryExpr(optimized),
                    ..plan
                }
            }
            PlanType::UnaryExpr(unary) => {
                let optimized = self.optimize_unary(unary)?;
                QueryPlan {
                    plan_type: PlanType::UnaryExpr(optimized),
                    ..plan
                }
            }
            PlanType::Aggregation(agg) => {
                let optimized = self.optimize_aggregation(agg)?;
                QueryPlan {
                    plan_type: PlanType::Aggregation(optimized),
                    ..plan
                }
            }
        };

        Ok(optimized_plan)
    }

    /// 优化向量查询
    fn optimize_vector_query(&self, query: &VectorQueryPlan) -> Result<VectorQueryPlan> {
        debug!("Optimizing vector query: {:?}", query.name);
        
        // 1. 移除冗余条件
        let optimized_matchers = self.eliminate_redundant_conditions(&query.matchers)?;
        
        // 2. 排序匹配器，将 __name__ 放在前面，提高查询效率
        let sorted_matchers = self.sort_matchers(&optimized_matchers);
        
        Ok(VectorQueryPlan {
            name: query.name.clone(),
            matchers: sorted_matchers,
        })
    }
    
    /// 消除冗余条件
    fn eliminate_redundant_conditions(&self, matchers: &[(String, String)]) -> Result<Vec<(String, String)>> {
        let mut unique_matchers = HashMap::new();
        
        for (name, value) in matchers {
            // 只保留最新的同名条件
            unique_matchers.insert(name.clone(), value.clone());
        }
        
        Ok(unique_matchers.into_iter().collect())
    }
    
    /// 排序匹配器
    fn sort_matchers(&self, matchers: &[(String, String)]) -> Vec<(String, String)> {
        let mut sorted = matchers.to_vec();
        sorted.sort_by(|a, b| {
            // 将 __name__ 放在最前面
            if a.0 == "__name__" && b.0 != "__name__" {
                return std::cmp::Ordering::Less;
            }
            if a.0 != "__name__" && b.0 == "__name__" {
                return std::cmp::Ordering::Greater;
            }
            // 其他按名称排序
            a.0.cmp(&b.0)
        });
        sorted
    }

    /// 优化矩阵查询
    fn optimize_matrix_query(&self, query: &MatrixQueryPlan) -> Result<MatrixQueryPlan> {
        debug!("Optimizing matrix query");
        
        let optimized_vector = self.optimize_vector_query(&query.vector_plan)?;
        
        // 优化时间范围
        let optimized_range = self.optimize_time_range(query.range);
        
        Ok(MatrixQueryPlan {
            vector_plan: optimized_vector,
            range: optimized_range,
        })
    }
    
    /// 优化时间范围
    fn optimize_time_range(&self, range: i64) -> i64 {
        // 确保时间范围合理
        std::cmp::max(range, 1) // 至少 1ms
    }

    /// 优化函数调用
    fn optimize_call(&self, call: &CallPlan) -> Result<CallPlan> {
        debug!("Optimizing call: {:?}", call.func);
        
        // 优化函数参数
        let mut optimized_args = Vec::new();
        for arg in &call.args {
            let optimized_arg = self.optimize(arg.clone())?;
            optimized_args.push(optimized_arg);
        }
        
        Ok(CallPlan {
            func: call.func.clone(),
            args: optimized_args,
        })
    }

    /// 优化二元表达式
    fn optimize_binary(&self, binary: &BinaryExprPlan) -> Result<BinaryExprPlan> {
        debug!("Optimizing binary expression");
        
        // 优化左右操作数
        let optimized_lhs = self.optimize(*binary.lhs.clone())?;
        let optimized_rhs = self.optimize(*binary.rhs.clone())?;
        
        // 常量折叠
        if let Some(constant_result) = self.constant_fold_binary(binary.op, &optimized_lhs, &optimized_rhs) {
            // 如果可以常量折叠，返回常量结果
            return Ok(BinaryExprPlan {
                op: binary.op,
                lhs: Box::new(optimized_lhs),
                rhs: Box::new(optimized_rhs),
            });
        }
        
        Ok(BinaryExprPlan {
            op: binary.op,
            lhs: Box::new(optimized_lhs),
            rhs: Box::new(optimized_rhs),
        })
    }
    
    /// 常量折叠二元表达式
    fn constant_fold_binary(&self, op: crate::query::parser::BinaryOp, lhs: &QueryPlan, rhs: &QueryPlan) -> Option<QueryPlan> {
        // 这里可以实现常量折叠逻辑
        // 例如，如果左右都是常量，则直接计算结果
        None
    }

    /// 优化一元表达式
    fn optimize_unary(&self, unary: &UnaryExprPlan) -> Result<UnaryExprPlan> {
        debug!("Optimizing unary expression");
        
        // 优化表达式
        let optimized_expr = self.optimize(*unary.expr.clone())?;
        
        Ok(UnaryExprPlan {
            op: unary.op,
            expr: Box::new(optimized_expr),
        })
    }

    /// 优化聚合操作
    fn optimize_aggregation(&self, agg: &AggregationPlan) -> Result<AggregationPlan> {
        debug!("Optimizing aggregation");
        
        // 优化表达式
        let optimized_expr = self.optimize(*agg.expr.clone())?;
        
        // 优化分组
        let optimized_grouping = self.optimize_grouping(&agg.grouping);
        
        Ok(AggregationPlan {
            op: agg.op.clone(),
            expr: Box::new(optimized_expr),
            grouping: optimized_grouping,
            without: agg.without,
        })
    }
    
    /// 优化分组
    fn optimize_grouping(&self, grouping: &[String]) -> Vec<String> {
        // 移除重复的分组标签
        let mut unique_grouping = HashMap::new();
        for label in grouping {
            unique_grouping.insert(label.clone(), ());
        }
        unique_grouping.into_keys().collect()
    }
}

impl Default for QueryOptimizer {
    fn default() -> Self {
        Self::new()
    }
}

/// 优化统计
#[derive(Debug, Clone, Default)]
pub struct OptimizationStats {
    pub predicates_pushed_down: u64,
    pub columns_pruned: u64,
    pub redundant_conditions_eliminated: u64,
    pub constants_folded: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::query::parser::{Matchers, Matcher, MatchOp};

    #[test]
    fn test_optimize_vector_query() {
        let optimizer = QueryOptimizer::new();
        
        let query = VectorQueryPlan {
            name: Some("http_requests_total".to_string()),
            matchers: vec![("__name__".to_string(), "http_requests_total".to_string())],
        };

        let plan = QueryPlan {
            plan_type: PlanType::VectorQuery(query),
            start: 0,
            end: 1000,
            step: 100,
        };

        let optimized = optimizer.optimize(plan).unwrap();
        
        match optimized.plan_type {
            PlanType::VectorQuery(q) => {
                assert_eq!(q.name, Some("http_requests_total".to_string()));
            }
            _ => panic!("Expected Vector plan"),
        }
    }
}
