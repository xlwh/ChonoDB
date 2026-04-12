# ChronoDB 性能优化方案

## 问题描述

在 Medium 规模测试（2,500,000 样本）中，ChronoDB 的查询性能比 Prometheus 慢约 **92%**：

| 系统 | 平均查询耗时 |
|------|-------------|
| Prometheus | 1.96 ms |
| ChronoDB | 3.77 ms |

## 性能瓶颈分析

### 1. 配置层面

#### 1.1 查询缓存未启用
```yaml
query:
  enable_query_cache: false  # 当前配置
  query_cache_size: "256MB"
```
**影响**: 重复查询无法命中缓存，每次都需要重新计算。

#### 1.2 内存配置偏小
```yaml
memory:
  memstore_size: "512MB"  # 对于 250万样本可能不足
  query_cache_size: "256MB"
```
**影响**: 数据可能频繁换入换出，增加 IO 开销。

#### 1.3 压缩算法开销
```yaml
compression:
  time_column:
    algorithm: "zstd"
    level: 3  # 压缩级别较高，解压缩耗时
  value_column:
    algorithm: "zstd"
    level: 3
```
**影响**: ZSTD level 3 提供良好压缩比，但解压缩需要 CPU 时间。

### 2. 代码层面

#### 2.1 查询执行器
- **问题**: `executor.rs` 中每次查询都重新计算 downsample level
- **问题**: 聚合操作使用 HashMap，对于大量数据效率不高
- **问题**: 缺少批处理和向量化执行

#### 2.2 内存存储
- **问题**: MemStore 查询时可能需要遍历大量数据
- **问题**: 缺少索引优化

#### 2.3 异步开销
- **问题**: 每个查询操作都使用 async/await，可能引入额外开销

## 优化方案

### 方案一：配置优化（快速见效）

#### 1. 启用查询缓存
```yaml
query:
  enable_query_cache: true
  query_cache_size: "512MB"  # 增加缓存大小
  query_cache_ttl: 600  # 增加缓存时间
```

#### 2. 增加内存配置
```yaml
memory:
  memstore_size: "2GB"  # 增加内存存储
  query_cache_size: "512MB"
  max_memory_usage: "85%"
```

#### 3. 调整压缩级别
```yaml
compression:
  time_column:
    algorithm: "zstd"
    level: 1  # 降低压缩级别，提高解压速度
  value_column:
    algorithm: "zstd"
    level: 1
```

**预期效果**: 查询延迟降低 20-30%

### 方案二：查询优化（中等投入）

#### 1. 实现查询结果缓存
在 `query/engine.rs` 中添加查询结果缓存：

```rust
use std::collections::HashMap;
use std::sync::Mutex;

pub struct QueryEngine {
    memstore: Arc<MemStore>,
    planner: QueryPlanner,
    executor: QueryExecutor,
    cache: Mutex<HashMap<String, QueryResult>>,  // 添加缓存
}
```

#### 2. 优化聚合操作
使用更高效的数据结构：

```rust
// 使用 BTreeMap 替代 HashMap，保持有序
use std::collections::BTreeMap;

async fn execute_sum(&self, plan: &CallPlan, ctx: &ExecutionContext) -> Result<Vec<TimeSeries>> {
    let series = self.execute_plan(&plan.args[0].plan_type, ctx).await?;
    
    // 使用 SIMD 加速求和
    let mut timestamp_sums: BTreeMap<i64, f64> = BTreeMap::new();
    
    for ts in &series {
        for sample in &ts.samples {
            *timestamp_sums.entry(sample.timestamp).or_insert(0.0) += sample.value;
        }
    }
    
    // BTreeMap 已经有序，无需额外排序
    // ...
}
```

#### 3. 实现预聚合
对于常用查询，预先计算聚合结果：

```rust
pub struct PreAggregation {
    metric: String,
    aggregation: String,
    interval: i64,
    data: Arc<RwLock<HashMap<i64, f64>>>,
}
```

**预期效果**: 查询延迟降低 40-50%

### 方案三：架构优化（长期投入）

#### 1. 实现列式存储优化
- 使用 Arrow 格式存储数据
- 实现向量化查询执行

#### 2. 添加索引
- 为标签添加倒排索引
- 为时间戳添加范围索引

```rust
pub struct Index {
    label_index: HashMap<String, HashMap<String, Vec<TimeSeriesId>>>,
    time_index: BTreeMap<i64, Vec<TimeSeriesId>>,
}
```

#### 3. 实现并行查询
利用 Rust 的并行处理能力：

```rust
use rayon::prelude::*;

async fn execute_parallel(&self, plan: &QueryPlan) -> Result<QueryResult> {
    let chunks: Vec<_> = plan.chunks().collect();
    
    let results: Vec<_> = chunks
        .par_iter()
        .map(|chunk| self.execute_chunk(chunk))
        .collect();
    
    // 合并结果
    Ok(merge_results(results))
}
```

**预期效果**: 查询延迟降低 60-70%

## 实施建议

### 短期（1-2 天）
1. 实施配置优化（方案一）
2. 测试验证性能提升

### 中期（1-2 周）
1. 实现查询结果缓存
2. 优化聚合操作
3. 添加性能监控

### 长期（1-2 月）
1. 实现列式存储
2. 添加索引系统
3. 实现并行查询

## 验证方法

使用集成测试框架验证优化效果：

```bash
cd integration_tests

# 测试优化前性能
python3 run_local_test.py --scale medium --compare

# 应用优化后
# ... 修改配置和代码 ...

# 测试优化后性能
python3 run_local_test.py --scale medium --compare
```

## 预期目标

| 优化阶段 | 目标延迟 | 相对 Prometheus |
|---------|---------|----------------|
| 当前 | 3.77 ms | 慢 92% |
| 配置优化后 | 2.5-3.0 ms | 慢 30-50% |
| 查询优化后 | 1.5-2.0 ms | 快 0-25% |
| 架构优化后 | < 1.5 ms | 快 25%+ |

## 参考文档

- [ChronoDB 配置文档](../config/chronodb.yaml)
- [集成测试报告](./integration_test_report.md)
- [Prometheus 存储文档](https://prometheus.io/docs/prometheus/latest/storage/)
