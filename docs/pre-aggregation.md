# 智能预聚合功能用户指南

## 概述

ChronoDB 的智能预聚合功能是一个强大的性能优化特性，通过自动识别高频查询并预先计算聚合结果，显著提升查询性能。该功能支持手动管理和自动管理两种模式，可在单机和分布式环境下使用。

## 核心特性

### 1. 智能频率统计
- **自动识别高频查询**: 基于滑动窗口算法统计查询频率
- **查询标准化**: 自动标准化查询表达式，识别相似查询
- **实时更新**: 实时跟踪查询频率变化

### 2. 自动规则管理
- **自动创建规则**: 当查询频率达到阈值时自动创建预聚合规则
- **自动清理规则**: 自动清理低频使用的预聚合规则
- **智能优化**: 根据查询模式动态调整预聚合策略

### 3. 查询路由优化
- **智能路由**: 自动将查询路由到预聚合数据
- **性能提升**: 高频查询性能提升 50%-90%
- **透明切换**: 对用户完全透明，无需修改查询

### 4. 分布式支持
- **任务协调**: 分布式环境下的任务自动协调
- **负载均衡**: 智能分配预聚合任务到各节点
- **故障转移**: 节点故障时自动重新分配任务

## 快速开始

### 1. 启用预聚合功能

在配置文件中添加预聚合配置：

```yaml
pre_aggregation:
  auto_create:
    enabled: true
    frequency_threshold: 20      # 查询频率阈值（次/小时）
    time_window: 24              # 统计时间窗口（小时）
    max_auto_rules: 100          # 最大自动创建规则数
    
  auto_cleanup:
    enabled: true
    check_interval: 6            # 清理检查间隔（小时）
    low_frequency_threshold: 5   # 低频阈值（次/小时）
    observation_period: 48       # 清理观察期（小时）
    
  storage:
    retention_days: 30           # 数据保留时间（天）
    max_storage_gb: 100          # 最大存储空间（GB）
    compression: zstd            # 压缩算法
```

### 2. 手动创建预聚合规则

通过 API 创建预聚合规则：

```bash
curl -X POST http://localhost:9090/api/admin/preagg/rules \
  -H "Content-Type: application/json" \
  -d '{
    "name": "http_requests_rate_5m",
    "expr": "sum(rate(http_requests_total[5m])) by (status)",
    "labels": {
      "aggregation": "rate",
      "interval": "5m"
    }
  }'
```

### 3. 查看预聚合规则

```bash
# 获取所有预聚合规则
curl http://localhost:9090/api/admin/preagg/rules

# 获取预聚合统计信息
curl http://localhost:9090/api/admin/preagg/stats

# 获取预聚合建议
curl http://localhost:9090/api/admin/preagg/suggestions
```

## 使用场景

### 场景 1: 高频查询优化

**问题**: 某个查询每分钟执行数十次，响应时间较长

**解决方案**: 
1. 系统自动识别高频查询
2. 自动创建预聚合规则
3. 后续查询自动使用预聚合数据
4. 性能提升 50%-90%

**示例**:
```promql
# 原始查询（执行时间: 500ms）
sum(rate(http_requests_total{job="api"}[5m])) by (status)

# 使用预聚合后（执行时间: 50ms）
# 系统自动路由到预聚合数据
```

### 场景 2: 仪表盘查询优化

**问题**: Grafana 仪表盘包含多个面板，每个面板都查询相同的数据

**解决方案**:
1. 为仪表盘查询创建预聚合规则
2. 所有面板共享预聚合数据
3. 仪表盘加载速度显著提升

**配置示例**:
```yaml
# 为仪表盘创建专用预聚合规则
- name: dashboard_http_requests
  expr: sum(rate(http_requests_total[5m])) by (job, status)
  labels:
    dashboard: "main"
```

### 场景 3: 定期报告生成

**问题**: 每小时生成报告，需要执行大量聚合查询

**解决方案**:
1. 为报告查询创建预聚合规则
2. 报告生成时直接使用预聚合数据
3. 报告生成时间大幅缩短

## 配置详解

### 自动创建配置

| 参数 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `enabled` | bool | true | 是否启用自动创建 |
| `frequency_threshold` | int | 20 | 查询频率阈值（次/小时） |
| `time_window` | int | 24 | 统计时间窗口（小时） |
| `max_auto_rules` | int | 100 | 最大自动创建规则数 |
| `exclude_patterns` | []string | ["^up$", "^ALERTS"] | 排除的查询模式 |

### 自动清理配置

| 参数 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `enabled` | bool | true | 是否启用自动清理 |
| `check_interval` | int | 6 | 清理检查间隔（小时） |
| `low_frequency_threshold` | int | 5 | 低频阈值（次/小时） |
| `observation_period` | int | 48 | 清理观察期（小时） |

### 存储配置

| 参数 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `retention_days` | int | 30 | 数据保留时间（天） |
| `max_storage_gb` | int | 100 | 最大存储空间（GB） |
| `compression` | string | "zstd" | 压缩算法 |

## API 参考

### 创建预聚合规则

**请求**:
```http
POST /api/admin/preagg/rules
Content-Type: application/json

{
  "name": "rule_name",
  "expr": "promql_expression",
  "labels": {
    "key": "value"
  }
}
```

**响应**:
```json
{
  "status": "success",
  "data": {
    "success": true,
    "message": "Pre-aggregation rule created successfully",
    "rule_id": "preagg-uuid"
  }
}
```

### 获取预聚合规则列表

**请求**:
```http
GET /api/admin/preagg/rules
```

**响应**:
```json
{
  "status": "success",
  "data": {
    "total": 5,
    "auto_created": 3,
    "rules": [
      {
        "id": "preagg-uuid",
        "name": "rule_name",
        "expr": "promql_expression",
        "is_auto_created": false,
        "status": "active",
        "query_frequency": 150,
        "created_at": 1234567890000
      }
    ]
  }
}
```

### 获取预聚合统计信息

**请求**:
```http
GET /api/admin/preagg/stats
```

**响应**:
```json
{
  "status": "success",
  "data": {
    "total_rules": 5,
    "active_rules": 4,
    "auto_created_rules": 3,
    "total_data_points": 10000,
    "storage_bytes": 1048576
  }
}
```

### 获取预聚合建议

**请求**:
```http
GET /api/admin/preagg/suggestions
```

**响应**:
```json
{
  "status": "success",
  "data": {
    "suggestions": [
      {
        "query": "sum(rate(http_requests_total[5m]))",
        "frequency": 150,
        "frequency_per_hour": 25.0,
        "potential_benefit": "High"
      }
    ]
  }
}
```

### 删除预聚合规则

**请求**:
```http
DELETE /api/admin/preagg/rules/{rule_id}
```

**响应**:
```json
{
  "status": "success",
  "data": {
    "success": true,
    "message": "Rule deleted successfully"
  }
}
```

## 分布式部署

### 单机模式

在单机模式下，所有预聚合任务都在本地执行：

```rust
let scheduler = PreAggregationScheduler::new(query_engine);
scheduler.start().await;
```

### 分布式模式

在分布式模式下，任务会自动分配到各个节点：

```rust
let coordinator = Arc::new(DistributedPreAggregationCoordinator::new(
    node_id,
    DistributedPreAggregationConfig::default(),
));

let scheduler = PreAggregationScheduler::with_distributed(
    query_engine,
    config,
    coordinator,
    node_id,
);

scheduler.start().await;
```

### 分布式配置

```yaml
distributed:
  enabled: true
  node_id: "node-1"
  cluster_name: "chronodb-cluster"
  
  coordination:
    heartbeat_interval_ms: 10000    # 心跳间隔
    task_timeout_ms: 300000         # 任务超时
    max_retries: 3                  # 最大重试次数
    enable_auto_failover: true      # 启用自动故障转移
```

## 最佳实践

### 1. 合理设置频率阈值

- **高频场景**: 设置较低的阈值（如 10 次/小时）
- **低频场景**: 设置较高的阈值（如 50 次/小时）
- **混合场景**: 使用默认值（20 次/小时）

### 2. 监控预聚合效果

定期检查预聚合统计信息：

```bash
# 查看预聚合统计
curl http://localhost:9090/api/admin/preagg/stats

# 查看预聚合建议
curl http://localhost:9090/api/admin/preagg/suggestions
```

### 3. 手动管理关键查询

对于关键业务查询，建议手动创建预聚合规则：

```yaml
- name: critical_api_latency
  expr: histogram_quantile(0.99, rate(api_latency_bucket[5m]))
  labels:
    priority: "critical"
```

### 4. 定期清理无用规则

启用自动清理功能，定期清理低频规则：

```yaml
auto_cleanup:
  enabled: true
  check_interval: 6
  low_frequency_threshold: 5
  observation_period: 48
```

### 5. 控制存储空间

设置合理的存储限制：

```yaml
storage:
  retention_days: 30
  max_storage_gb: 100
  compression: zstd
```

## 性能指标

### 预期性能提升

| 查询类型 | 原始性能 | 预聚合后 | 提升比例 |
|---------|---------|---------|---------|
| 简单聚合 | 100ms | 10ms | 90% |
| 复杂聚合 | 500ms | 50ms | 90% |
| 多维度聚合 | 1000ms | 100ms | 90% |
| 范围查询 | 2000ms | 400ms | 80% |

### 监控指标

系统提供以下 Prometheus 指标：

```promql
# 预聚合规则总数
chronodb_preagg_rules_total

# 自动创建的规则数
chronodb_preagg_rules_auto_created

# 路由到预聚合数据的查询数
chronodb_preagg_queries_routed

# 预聚合查询延迟
chronodb_preagg_query_latency_seconds

# 预聚合数据存储大小
chronodb_preagg_storage_bytes

# 预聚合任务执行时间
chronodb_preagg_task_duration_seconds
```

## 故障排除

### 问题 1: 预聚合规则未自动创建

**可能原因**:
- 查询频率未达到阈值
- 自动创建功能未启用
- 已达到最大规则数限制

**解决方案**:
```bash
# 检查查询频率
curl http://localhost:9090/api/admin/preagg/suggestions

# 检查配置
curl http://localhost:9090/api/admin/config

# 手动创建规则
curl -X POST http://localhost:9090/api/admin/preagg/rules -d '...'
```

### 问题 2: 查询未使用预聚合数据

**可能原因**:
- 查询表达式不匹配
- 预聚合数据不可用
- 路由功能未启用

**解决方案**:
```bash
# 检查预聚合规则
curl http://localhost:9090/api/admin/preagg/rules

# 检查预聚合数据
curl http://localhost:9090/api/admin/preagg/stats

# 检查查询标准化
# 确保查询表达式与规则匹配
```

### 问题 3: 分布式任务执行失败

**可能原因**:
- 节点间通信失败
- 任务超时
- 节点故障

**解决方案**:
```bash
# 检查节点状态
curl http://localhost:9090/api/admin/cluster/nodes

# 检查任务状态
curl http://localhost:9090/api/admin/preagg/stats

# 查看日志
tail -f /var/log/chronodb/chronodb.log
```

### 问题 4: 存储空间不足

**可能原因**:
- 预聚合数据过多
- 保留时间过长
- 未启用压缩

**解决方案**:
```yaml
# 调整存储配置
storage:
  retention_days: 15        # 减少保留时间
  max_storage_gb: 50        # 限制存储空间
  compression: zstd         # 启用压缩

# 启用自动清理
auto_cleanup:
  enabled: true
  low_frequency_threshold: 10  # 提高低频阈值
```

## 常见问题

**Q: 预聚合功能会影响写入性能吗？**

A: 预聚合功能主要影响查询性能，对写入性能影响很小。预聚合任务在后台异步执行，不会阻塞数据写入。

**Q: 如何判断哪些查询适合预聚合？**

A: 适合预聚合的查询通常具有以下特征：
- 执行频率高（如每分钟多次）
- 计算复杂（如多维度聚合）
- 数据量大（如扫描大量时间序列）
- 结果相对稳定（如历史数据统计）

**Q: 预聚合数据会占用多少存储空间？**

A: 存储空间取决于预聚合规则数量和数据保留时间。通常情况下，预聚合数据会增加 10%-30% 的存储空间。

**Q: 可以禁用自动创建功能吗？**

A: 可以。在配置文件中设置 `auto_create.enabled: false` 即可禁用自动创建功能，只使用手动管理。

**Q: 预聚合功能支持哪些 PromQL 函数？**

A: 预聚合功能支持所有标准的 PromQL 聚合函数，包括：
- `sum`, `avg`, `min`, `max`, `count`
- `rate`, `irate`, `increase`
- `histogram_quantile`
- 以及其他标准聚合函数

## 相关资源

- [ChronoDB 使用文档](Usage.md)
- [API 文档](API.md)
- [配置指南](Configuration.md)
- [性能调优](Performance.md)

## 更新日志

### v0.1.0 (2024-01-15)
- 初始版本发布
- 支持自动创建和清理预聚合规则
- 支持查询路由优化
- 支持分布式任务协调
