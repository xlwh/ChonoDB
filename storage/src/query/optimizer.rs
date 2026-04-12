use crate::error::Result;
use crate::query::planner::{QueryPlan, PlanType, VectorQueryPlan, MatrixQueryPlan, CallPlan, AggregationPlan};
use crate::query::parser::Function;
use std::collections::HashMap;

/// 查询统计信息
#[derive(Debug, Clone, Default)]
pub struct QueryStats {
    /// 表行数估计
    pub estimated_rows: u64,
    /// 选择率（0-1）
    pub selectivity: f64,
    /// 每行平均大小（字节）
    pub avg_row_size: u64,
    /// 索引可用性
    pub has_index: bool,
}

/// 查询成本模型
#[derive(Debug, Clone)]
pub struct QueryCost {
    /// CPU 成本（操作数）
    pub cpu_cost: f64,
    /// I/O 成本（磁盘读取次数）
    pub io_cost: f64,
    /// 内存成本（字节）
    pub memory_cost: f64,
    /// 网络成本（分布式场景）
    pub network_cost: f64,
    /// 总成本
    pub total_cost: f64,
}

impl QueryCost {
    pub fn new(cpu_cost: f64, io_cost: f64, memory_cost: f64, network_cost: f64) -> Self {
        let total_cost = cpu_cost * 0.4 + io_cost * 0.4 + memory_cost * 0.1 + network_cost * 0.1;
        Self {
            cpu_cost,
            io_cost,
            memory_cost,
            network_cost,
            total_cost,
        }
    }

    pub fn zero() -> Self {
        Self {
            cpu_cost: 0.0,
            io_cost: 0.0,
            memory_cost: 0.0,
            network_cost: 0.0,
            total_cost: 0.0,
        }
    }
}

/// 查询优化器
pub struct QueryOptimizer {
    /// 表统计信息
    table_stats: HashMap<String, QueryStats>,
    /// 成本模型权重
    cpu_weight: f64,
    io_weight: f64,
    memory_weight: f64,
    network_weight: f64,
}

impl QueryOptimizer {
    pub fn new() -> Self {
        Self {
            table_stats: HashMap::new(),
            cpu_weight: 0.4,
            io_weight: 0.4,
            memory_weight: 0.1,
            network_weight: 0.1,
        }
    }

    /// 更新表统计信息
    pub fn update_table_stats(&mut self, table_name: String, stats: QueryStats) {
        self.table_stats.insert(table_name, stats);
    }

    /// 优化查询计划
    pub fn optimize(&self, plan: QueryPlan) -> Result<QueryPlan> {
        // 目前只进行简单的计划重写
        // 未来可以添加更复杂的优化规则
        Ok(plan)
    }

    /// 估算查询成本
    pub fn estimate_cost(&self, plan: &QueryPlan) -> QueryCost {
        self.estimate_plan_cost(&plan.plan_type)
    }

    fn estimate_plan_cost(&self, plan_type: &PlanType) -> QueryCost {
        match plan_type {
            PlanType::VectorQuery(plan) => self.estimate_vector_cost(plan),
            PlanType::MatrixQuery(plan) => self.estimate_matrix_cost(plan),
            PlanType::Call(plan) => self.estimate_call_cost(plan),
            PlanType::Aggregation(plan) => self.estimate_aggregation_cost(plan),
            PlanType::BinaryExpr(bin) => {
                let lhs_cost = self.estimate_plan_cost(&bin.lhs.plan_type);
                let rhs_cost = self.estimate_plan_cost(&bin.rhs.plan_type);
                QueryCost::new(
                    lhs_cost.cpu_cost + rhs_cost.cpu_cost,
                    lhs_cost.io_cost + rhs_cost.io_cost,
                    lhs_cost.memory_cost.max(rhs_cost.memory_cost),
                    lhs_cost.network_cost + rhs_cost.network_cost,
                )
            }
            PlanType::UnaryExpr(unary) => {
                self.estimate_plan_cost(&unary.expr.plan_type)
            }
        }
    }

    fn estimate_vector_cost(&self, plan: &VectorQueryPlan) -> QueryCost {
        // 基础成本
        let base_cpu_cost = 100.0;
        let base_io_cost = 50.0;
        
        // 根据匹配器复杂度调整成本
        let matcher_cost = plan.matchers.len() as f64 * 10.0;
        
        // 获取指标名称
        let metric_name = plan.name.as_deref().unwrap_or("");
        
        // 如果有索引，I/O 成本降低
        let io_cost = if self.has_index(metric_name) {
            base_io_cost * 0.3
        } else {
            base_io_cost
        };

        QueryCost::new(
            base_cpu_cost + matcher_cost,
            io_cost,
            1024.0, // 1KB 内存
            0.0,
        )
    }

    fn estimate_matrix_cost(&self, plan: &MatrixQueryPlan) -> QueryCost {
        // 范围查询成本更高
        let vector_cost = self.estimate_vector_cost(&plan.vector_plan);
        
        // 根据范围大小调整成本
        let range_ms = plan.range as f64;
        let range_factor = (range_ms / 3600000.0).max(1.0); // 每小时一个因子
        
        QueryCost::new(
            vector_cost.cpu_cost * range_factor,
            vector_cost.io_cost * range_factor,
            vector_cost.memory_cost * range_factor,
            vector_cost.network_cost,
        )
    }

    fn estimate_call_cost(&self, plan: &CallPlan) -> QueryCost {
        let input_cost: QueryCost = plan.args.iter()
            .map(|arg| self.estimate_plan_cost(&arg.plan_type))
            .fold(QueryCost::zero(), |acc, cost| QueryCost::new(
                acc.cpu_cost + cost.cpu_cost,
                acc.io_cost + cost.io_cost,
                acc.memory_cost + cost.memory_cost,
                acc.network_cost + cost.network_cost,
            ));

        // 函数调用的额外成本
        let func_cpu_cost = match plan.func {
            Function::Rate | Function::Irate => 200.0,
            Function::Sum | Function::Avg | Function::Min | Function::Max => 150.0,
            Function::HistogramQuantile => 500.0,
            _ => 100.0,
        };

        QueryCost::new(
            input_cost.cpu_cost + func_cpu_cost,
            input_cost.io_cost,
            input_cost.memory_cost,
            input_cost.network_cost,
        )
    }

    fn estimate_aggregation_cost(&self, plan: &AggregationPlan) -> QueryCost {
        let input_cost = self.estimate_plan_cost(&plan.expr.plan_type);
        
        // 聚合操作的成本
        let agg_cpu_cost = input_cost.cpu_cost * 1.5;
        let agg_memory_cost = input_cost.memory_cost * 2.0;
        
        QueryCost::new(
            input_cost.cpu_cost + agg_cpu_cost,
            input_cost.io_cost,
            agg_memory_cost,
            input_cost.network_cost,
        )
    }

    fn has_index(&self, metric: &str) -> bool {
        // 检查是否有索引
        self.table_stats.get(metric)
            .map(|stats| stats.has_index)
            .unwrap_or(false)
    }

    /// 选择最优的降采样级别
    pub fn select_downsample_level(&self, query_range_ms: i64) -> u8 {
        // 根据查询范围选择降采样级别
        let range_hours = query_range_ms / 3_600_000;
        
        if range_hours < 1 {
            0 // L0: 原始数据
        } else if range_hours < 24 {
            1 // L1: 1分钟精度
        } else if range_hours < 168 { // 1周
            2 // L2: 5分钟精度
        } else if range_hours < 720 { // 1月
            3 // L3: 1小时精度
        } else {
            4 // L4: 1天精度
        }
    }

    /// 判断是否使用并行执行
    pub fn should_use_parallel(&self, estimated_rows: u64) -> bool {
        // 当估计行数超过阈值时使用并行执行
        estimated_rows > 1000
    }

    /// 获取最优执行策略
    pub fn get_execution_strategy(&self, plan: &QueryPlan) -> ExecutionStrategy {
        let cost = self.estimate_cost(plan);
        
        ExecutionStrategy {
            use_parallel: self.should_use_parallel(cost.cpu_cost as u64),
            use_index: cost.io_cost < 100.0,
            downsample_level: self.select_downsample_level(plan.end - plan.start),
            cache_result: cost.total_cost > 500.0,
        }
    }
}

/// 执行策略
#[derive(Debug, Clone)]
pub struct ExecutionStrategy {
    /// 是否使用并行执行
    pub use_parallel: bool,
    /// 是否使用索引
    pub use_index: bool,
    /// 降采样级别
    pub downsample_level: u8,
    /// 是否缓存结果
    pub cache_result: bool,
}

/// 优化统计信息
#[derive(Debug, Clone, Default)]
pub struct OptimizationStats {
    pub optimized_plans: u64,
    pub cost_reduction: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cost_calculation() {
        let optimizer = QueryOptimizer::new();
        
        let cost1 = QueryCost::new(100.0, 50.0, 1024.0, 0.0);
        let cost2 = QueryCost::new(200.0, 100.0, 2048.0, 0.0);
        
        assert!(cost2.total_cost > cost1.total_cost);
    }

    #[test]
    fn test_downsample_level_selection() {
        let optimizer = QueryOptimizer::new();
        
        // 小于1小时，使用原始数据
        assert_eq!(optimizer.select_downsample_level(30 * 60 * 1000), 0);
        
        // 1小时到1天，使用L1
        assert_eq!(optimizer.select_downsample_level(2 * 60 * 60 * 1000), 1);
        
        // 1周到1月，使用L3
        assert_eq!(optimizer.select_downsample_level(20 * 24 * 60 * 60 * 1000), 3);
    }
}
