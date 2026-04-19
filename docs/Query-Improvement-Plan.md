# ChronoDB 查询功能完善计划

**规划日期**: 2026-04-19  
**目标**: 提升 PromQL 兼容性到 90%+  

---

## 📊 当前查询功能状态

### 已完成功能 (70%)

| 功能类别 | 完成度 | 状态 |
|---------|--------|------|
| **基础查询** | 100% | ✅ 完整 |
| **聚合操作** | 100% | ✅ 完整 |
| **by/without** | 100% | ✅ 完整 |
| **逻辑操作符** | 100% | ✅ 完整 |
| **标量函数** | 100% | ✅ 完整 |
| **范围向量函数** | 100% | ✅ 完整 |
| **数学函数** | 100% | ✅ 完整 |
| **时间函数** | 100% | ✅ 完整 |
| **标签函数** | 100% | ✅ 完整 |

### 待完善功能 (30%)

| 功能 | 当前状态 | 目标 | 工作量 |
|------|---------|------|--------|
| **count_values** | ❌ 未实现 | ✅ 实现 | 1天 |
| **@ 修饰符** | ❌ 未实现 | ✅ 实现 | 2天 |
| **offset 修饰符** | ⚠️ 部分 | ✅ 完整 | 1天 |
| **topk/bottomk 测试** | ⚠️ 未测试 | ✅ 测试+优化 | 1天 |
| **quantile 测试** | ⚠️ 未测试 | ✅ 测试+优化 | 1天 |
| **子查询** | ❌ 未实现 | ✅ 实现 | 3天 |

---

## 🎯 查询完善路线图

### Week 1: 聚合函数完善

#### Day 1-2: count_values 函数

**功能描述**:
```promql
count_values("version", build_version)
```
统计每个不同值出现的次数。

**实现任务**:
- [ ] 在 `Function` 枚举中添加 `CountValues`
- [ ] 在 `execute_count_values` 中实现逻辑
- [ ] 创建测试用例
- [ ] 更新文档

**实现思路**:
```rust
async fn execute_count_values(&self, plan: &CallPlan, ctx: &ExecutionContext) -> Result<Vec<TimeSeries>> {
    // 1. 执行参数查询
    // 2. 统计每个值的出现次数
    // 3. 创建新的时间序列，标签包含原始值
}
```

#### Day 3-4: @ 修饰符

**功能描述**:
```promql
http_requests_total @ 1609746000
http_requests_total @ start()
```
在指定时间点查询数据。

**实现任务**:
- [ ] 在 `Expr` 结构中添加 `at` 字段
- [ ] 修改解析器支持 `@` 语法
- [ ] 修改执行器使用 `@` 时间
- [ ] 支持 `@ start()` 和 `@ end()`
- [ ] 创建测试用例

**实现思路**:
```rust
pub struct Expr {
    pub expr_type: ExprType,
    pub at: Option<At>,  // 新增
}

pub enum At {
    Timestamp(i64),
    Start,
    End,
}
```

#### Day 5: offset 修饰符完善

**功能描述**:
```promql
http_requests_total offset 5m
http_requests_total offset -1h
sum(http_requests_total) offset 1d
```

**实现任务**:
- [ ] 检查当前 offset 实现
- [ ] 支持负值 offset（查询未来数据）
- [ ] 支持聚合查询中的 offset
- [ ] 创建测试用例

---

### Week 2: 测试与优化

#### Day 1-2: topk/bottomk 测试与优化

**当前状态**: 已实现但未测试

**任务**:
- [ ] 创建 topk 测试用例
- [ ] 创建 bottomk 测试用例
- [ ] 测试边界条件（k=0, k>series count）
- [ ] 性能测试（大规模数据）
- [ ] 修复发现的问题

**测试场景**:
```promql
topk(3, http_requests_total)
bottomk(5, cpu_usage)
topk(10, sum by (job) (rate(http_requests_total[5m])))
```

#### Day 3-4: quantile 测试与优化

**当前状态**: 已实现但未测试

**任务**:
- [ ] 创建 quantile 测试用例
- [ ] 测试不同分位数（0, 0.5, 0.95, 0.99, 1）
- [ ] 测试边界条件
- [ ] 性能测试
- [ ] 修复发现的问题

**测试场景**:
```promql
quantile(0.95, http_request_duration_seconds)
quantile(0.5, temperature_celsius)
```

#### Day 5: 性能优化

**任务**:
- [ ] 分析查询性能瓶颈
- [ ] 优化聚合函数性能
- [ ] 优化向量匹配性能
- [ ] 添加查询性能监控

---

### Week 3: 子查询支持

#### Day 1-3: 子查询实现

**功能描述**:
```promql
# 子查询语法
sum(rate(http_requests_total[5m])[30m:1m])

# 等价于
sum(
  # 子查询: 每1m计算一次rate，持续30m
  rate(http_requests_total[5m])[30m:1m]
)
```

**实现任务**:
- [ ] 在 `Expr` 中添加子查询类型
- [ ] 修改解析器支持子查询语法
- [ ] 修改执行器支持子查询执行
- [ ] 支持子查询中的范围选择
- [ ] 支持子查询中的步长

**实现思路**:
```rust
pub enum ExprType {
    // ... 现有类型
    Subquery(SubqueryExpr),
}

pub struct SubqueryExpr {
    pub expr: Box<Expr>,
    pub range: Duration,
    pub step: Duration,
    pub offset: Option<Duration>,
}
```

#### Day 4-5: 子查询测试

**任务**:
- [ ] 创建基础子查询测试
- [ ] 测试嵌套子查询
- [ ] 测试子查询与聚合组合
- [ ] 性能测试
- [ ] 修复发现的问题

**测试场景**:
```promql
# 基础子查询
rate(http_requests_total[5m])[30m:1m]

# 子查询 + 聚合
max_over_time(rate(http_requests_total[5m])[1h:1m])

# 嵌套子查询
sum(rate(http_requests_total[5m])[30m:1m])[1h:5m]

# 子查询 + offset
rate(http_requests_total[5m] offset 1h)[30m:1m]
```

---

## 📋 详细实现计划

### 1. count_values 实现

**文件**: `storage/src/query/executor.rs`

```rust
async fn execute_count_values(&self, plan: &CallPlan, ctx: &ExecutionContext) -> Result<Vec<TimeSeries>> {
    if plan.args.len() != 2 {
        return Err(Error::InvalidData("count_values() requires exactly 2 arguments".to_string()));
    }
    
    let label_name = match &plan.args[0].plan_type {
        PlanType::Literal(Literal::String(s)) => s.clone(),
        _ => return Err(Error::InvalidData("First argument must be a string label name".to_string())),
    };
    
    let series = self.execute_plan(&plan.args[1].plan_type, ctx).await?;
    
    // 统计每个值的出现次数
    let mut value_counts: HashMap<String, u64> = HashMap::new();
    for ts in &series {
        for sample in &ts.samples {
            let value_str = sample.value.to_string();
            *value_counts.entry(value_str).or_insert(0) += 1;
        }
    }
    
    // 创建结果序列
    let mut result = Vec::new();
    for (value, count) in value_counts {
        let mut labels = Vec::new();
        labels.push(Label::new(label_name.clone(), value));
        let mut ts = TimeSeries::new(0, labels);
        ts.add_sample(Sample::new(ctx.start, count as f64));
        result.push(ts);
    }
    
    Ok(result)
}
```

### 2. @ 修饰符实现

**文件**: `storage/src/query/parser.rs`, `storage/src/query/executor.rs`

**解析器修改**:
```rust
// 在 parse_primary 或相关函数中添加 @ 支持
fn parse_at_modifier(&mut self) -> Result<Option<At>> {
    if self.consume(Token::At) {
        if self.consume(Token::Start) {
            Ok(Some(At::Start))
        } else if self.consume(Token::End) {
            Ok(Some(At::End))
        } else {
            let timestamp = self.parse_number()?;
            Ok(Some(At::Timestamp(timestamp as i64)))
        }
    } else {
        Ok(None)
    }
}
```

**执行器修改**:
```rust
async fn execute_expr(&self, expr: &Expr, ctx: &ExecutionContext) -> Result<Vec<TimeSeries>> {
    let mut new_ctx = ctx.clone();
    
    // 处理 @ 修饰符
    if let Some(at) = &expr.at {
        new_ctx.start = match at {
            At::Timestamp(ts) => *ts,
            At::Start => ctx.query_start,
            At::End => ctx.query_end,
        };
        new_ctx.end = new_ctx.start;
    }
    
    self.execute_expr_type(&expr.expr_type, &new_ctx).await
}
```

### 3. offset 修饰符完善

**检查当前实现**:
```rust
// 在 VectorSelector 中检查 offset 处理
pub struct VectorSelector {
    pub metric: String,
    pub matchers: Vec<LabelMatcher>,
    pub offset: Option<Duration>,
}
```

**完善执行器**:
```rust
async fn execute_vector_selector(&self, selector: &VectorSelector, ctx: &ExecutionContext) -> Result<Vec<TimeSeries>> {
    let mut new_ctx = ctx.clone();
    
    // 应用 offset
    if let Some(offset) = selector.offset {
        let offset_secs = offset.as_secs() as i64;
        new_ctx.start -= offset_secs;
        new_ctx.end -= offset_secs;
    }
    
    // 继续执行查询...
}
```

---

## 🧪 测试计划

### 测试用例清单

#### count_values 测试
- [ ] 基础 count_values 测试
- [ ] 空数据测试
- [ ] 大量不同值测试
- [ ] 与聚合组合测试

#### @ 修饰符测试
- [ ] @ timestamp 测试
- [ ] @ start() 测试
- [ ] @ end() 测试
- [ ] 与 offset 组合测试
- [ ] 与聚合组合测试

#### offset 修饰符测试
- [ ] 正 offset 测试
- [ ] 负 offset 测试
- [ ] 与聚合组合测试
- [ ] 与子查询组合测试

#### topk/bottomk 测试
- [ ] 基础 topk 测试
- [ ] 基础 bottomk 测试
- [ ] k=0 边界测试
- [ ] k>series count 测试
- [ ] 与聚合组合测试

#### quantile 测试
- [ ] 不同分位数测试
- [ ] 边界值测试（0, 1）
- [ ] 空数据测试
- [ ] 与聚合组合测试

#### 子查询测试
- [ ] 基础子查询测试
- [ ] 嵌套子查询测试
- [ ] 子查询 + 聚合测试
- [ ] 子查询 + offset 测试
- [ ] 性能测试

---

## 📈 预期成果

### 功能提升

| 指标 | 当前 | 目标 | 提升 |
|------|------|------|------|
| **PromQL 兼容性** | 70% | 90% | +20% |
| **聚合函数覆盖** | 80% | 100% | +20% |
| **修饰符支持** | 50% | 100% | +50% |
| **子查询支持** | 0% | 100% | +100% |

### 质量提升

- ✅ 所有新功能有完整测试覆盖
- ✅ 性能测试通过
- ✅ 文档更新完整
- ✅ 代码审查通过

---

## ⏱️ 时间规划

| 周次 | 任务 | 工作量 | 产出 |
|------|------|--------|------|
| **Week 1** | 聚合函数完善 | 5天 | count_values, @, offset |
| **Week 2** | 测试与优化 | 5天 | 测试覆盖, 性能优化 |
| **Week 3** | 子查询支持 | 5天 | 子查询实现与测试 |

**总计**: 15个工作日 (3周)

---

## 🎯 验收标准

1. **功能完整**
   - count_values 函数可用
   - @ 修饰符可用
   - offset 修饰符完善
   - 子查询可用

2. **测试通过**
   - 所有新功能测试通过
   - 边界条件测试通过
   - 性能测试达标

3. **文档完整**
   - API 文档更新
   - 使用示例更新
   - 性能基准更新

---

## 🚀 下一步行动

1. **立即开始**: count_values 函数实现
2. **本周目标**: 完成聚合函数完善
3. **下周目标**: 完成测试与优化
4. **第三周**: 完成子查询支持

**准备好了吗？让我们开始实现 count_values 函数！**
