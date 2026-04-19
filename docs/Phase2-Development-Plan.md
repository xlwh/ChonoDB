# Phase 2 开发计划与执行

**启动日期**: 2026-04-19  
**目标**: 功能完善和性能优化  

---

## 🎯 Phase 2 目标

1. **PromQL 兼容性** - 达到 90%+ 兼容 Prometheus
2. **告警功能完善** - 支持多通知渠道
3. **API 兼容性** - 100% 兼容 Prometheus API
4. **数据迁移工具** - 支持主流数据源
5. **性能优化** - 查询 10x，存储 10x
6. **测试完善** - 覆盖率 90%

---

## 📋 Week 1-3: PromQL 兼容性开发

### 当前状态分析

从代码审查发现：
- ✅ 基础表达式类型已定义（VectorSelector, MatrixSelector, Call, BinaryExpr, Aggregation）
- ✅ 二元操作符已支持 And, Or, Unless
- ✅ 聚合函数已定义（Sum, Avg, Min, Max, Count 等）
- ⚠️ 缺少 by/without clause 的完整实现
- ⚠️ 缺少标量函数（scalar, vector）的实现
- ⚠️ 缺少修饰符（@, offset）的处理

### 开发任务

#### Week 1: by/without clause 和逻辑操作符

**任务 1.1**: 完善 Aggregation 的 by/without 支持
```rust
// 当前结构
pub struct Aggregation {
    pub op: Function,
    pub expr: Box<Expr>,
    pub grouping: Vec<String>,
    pub without: bool,  // true = without, false = by
}
```

**任务 1.2**: 实现聚合操作符的完整执行
- [ ] sum by (label1, label2)
- [ ] avg without (label1)
- [ ] count by (job)
- [ ] max without (instance)

**任务 1.3**: 验证逻辑操作符
- [ ] vector1 and vector2
- [ ] vector1 or vector2
- [ ] vector1 unless vector2

#### Week 2: 标量函数和二元操作符

**任务 2.1**: 实现标量函数
- [ ] scalar(vector) - 将单元素向量转为标量
- [ ] vector(scalar) - 将标量转为向量

**任务 2.2**: 完善二元操作符
- [ ] 算术操作符: +, -, *, /, %, ^
- [ ] 比较操作符: ==, !=, >, <, >=, <=
- [ ] 集合操作符: and, or, unless

**任务 2.3**: 实现向量匹配修饰符
- [ ] on (label1, label2)
- [ ] ignoring (label1, label2)
- [ ] group_left
- [ ] group_right

#### Week 3: 修饰符和测试

**任务 3.1**: 实现时间修饰符
- [ ] @ timestamp
- [ ] offset duration

**任务 3.2**: 创建 PromQL 兼容性测试套件
- [ ] 基础查询测试
- [ ] 聚合查询测试
- [ ] 二元操作测试
- [ ] 函数调用测试

**任务 3.3**: 修复发现的 Bug

---

## 📋 Week 4-5: 告警功能开发

### 当前状态分析

- ✅ 告警规则结构已定义
- ✅ 指标收集器已实现
- ⚠️ 缺少通知渠道实现
- ⚠️ 缺少告警抑制和静默

### 开发任务

#### Week 4: 通知渠道

**任务 4.1**: 实现邮件通知
```rust
pub struct EmailNotifier {
    smtp_server: String,
    smtp_port: u16,
    username: String,
    password: String,
    from: String,
    to: Vec<String>,
}
```

**任务 4.2**: 实现 Slack 通知
```rust
pub struct SlackNotifier {
    webhook_url: String,
    channel: String,
    username: String,
}
```

**任务 4.3**: 实现 Webhook 通知
```rust
pub struct WebhookNotifier {
    url: String,
    headers: HashMap<String, String>,
    method: String,
}
```

**任务 4.4**: 实现 PagerDuty 通知

#### Week 5: 告警管理

**任务 5.1**: 实现告警抑制
- [ ] 定义抑制规则
- [ ] 实现抑制逻辑
- [ ] 抑制状态持久化

**任务 5.2**: 实现告警静默
- [ ] 静默规则管理
- [ ] 静默时间窗口
- [ ] 静默状态查询

**任务 5.3**: 实现告警分组和路由
- [ ] 按标签分组
- [ ] 路由规则配置
- [ ] 通知模板定制

---

## 📋 Week 6-7: API 兼容性开发

### 当前状态分析

- ✅ 基础查询 API 已实现
- ✅ 元数据 API 已实现
- ⚠️ 缺少 Admin API
- ⚠️ 缺少 Targets/Rules/Alerts API

### 开发任务

#### Week 6: Admin API

**任务 6.1**: 实现删除序列 API
```
POST /api/v1/admin/tsdb/delete_series
```

**任务 6.2**: 实现清理数据 API
```
POST /api/v1/admin/tsdb/clean_tombstones
```

**任务 6.3**: 实现快照 API
```
POST /api/v1/admin/tsdb/snapshot
```

#### Week 7: 管理 API

**任务 7.1**: 实现 Targets API
```
GET /api/v1/targets
```

**任务 7.2**: 实现 Rules API
```
GET /api/v1/rules
```

**任务 7.3**: 实现 Alerts API
```
GET /api/v1/alerts
```

---

## 📋 Week 8-9: 数据迁移工具

### 开发任务

#### Week 8: 数据导入

**任务 8.1**: Prometheus TSDB 导入
- [ ] 读取 Prometheus TSDB 格式
- [ ] 块数据转换
- [ ] 索引重建

**任务 8.2**: InfluxDB Line Protocol 导入
- [ ] 解析 Line Protocol
- [ ] 批量写入

#### Week 9: 数据导出

**任务 9.1**: CSV 导出
**任务 9.2**: JSON 导出
**任务 9.3**: Parquet 导出

---

## 📋 Week 10-12: 性能优化

### 开发任务

#### Week 10: 查询优化

**任务 10.1**: 向量化执行引擎优化
- [ ] SIMD 加速
- [ ] 批量处理

**任务 10.2**: 查询并行化
- [ ] 多线程查询
- [ ] 分片并行

#### Week 11: 存储优化

**任务 11.1**: 压缩算法优化
- [ ] 评估不同压缩算法
- [ ] 自适应压缩选择

**任务 11.2**: 列式存储优化
- [ ] 列式布局优化
- [ ] 预聚合

#### Week 12: 写入优化

**任务 12.1**: 批量写入优化
**任务 12.2**: WAL 优化
**任务 12.3**: 内存池实现

---

## 📋 Week 13-14: 测试完善

### 开发任务

#### Week 13: 单元测试和集成测试

**任务 13.1**: 补充单元测试
- [ ] 目标覆盖率: 90%

**任务 13.2**: 添加端到端测试
- [ ] 完整工作流测试
- [ ] 混沌测试

#### Week 14: 性能测试和安全测试

**任务 14.1**: 性能基准测试
**任务 14.2**: 压力测试
**任务 14.3**: 安全扫描

---

## 🎯 关键里程碑

| 里程碑 | 时间 | 目标 | 验收标准 |
|--------|------|------|----------|
| M1 | Week 3 | PromQL 90% 兼容 | 通过兼容性测试套件 |
| M2 | Week 5 | 告警系统完善 | 支持 5+ 通知渠道 |
| M3 | Week 7 | API 100% 兼容 | 通过 API 兼容性测试 |
| M4 | Week 9 | 数据迁移工具 | 支持 Prometheus/InfluxDB |
| M5 | Week 12 | 性能达标 | 查询 10x，存储 10x |
| M6 | Week 14 | 测试完善 | 覆盖率 90% |

---

## 🚀 立即开始开发

### 第一步：PromQL by clause 实现

现在开始实现第一个任务：完善 Aggregation 的 by/without 支持。

**文件**: `storage/src/query/executor.rs`

需要修改：
1. 解析器支持 by/without 语法
2. 执行器实现分组聚合逻辑
3. 添加单元测试

---

**开发启动时间**: 2026-04-19  
**预计完成时间**: 14 周后（2026-07-26）  
**负责人**: 开发团队
