# 智能预聚合功能规格说明

## Why
ChronoDB 目前支持基本的记录规则（Recording Rules），但缺乏智能化的预聚合管理。用户需要手动配置预聚合规则，且查询时无法自动路由到预聚合数据。为了提升查询性能和用户体验，需要实现一个智能预聚合系统，支持自动根据查询频率创建和管理预聚合规则，并在查询时自动使用预聚合数据。

## What Changes
- 实现智能预聚合规则管理器，支持手动和自动创建规则
- 实现查询路由器，自动匹配和路由到预聚合数据
- 实现查询频率统计和分析系统
- 实现预聚合规则的自动创建和清理机制
- 实现预聚合数据的存储和索引
- 在 Web 管理界面中添加预聚合规则管理功能
- **BREAKING**: 需要修改查询引擎以支持预聚合数据路由

## Impact
- Affected specs: 查询引擎、规则管理、存储引擎、Web 管理界面
- Affected code:
  - storage/src/query/engine.rs（查询路由）
  - server/src/rules/recording.rs（预聚合规则管理）
  - server/src/rules/mod.rs（规则管理器）
  - storage/src/memstore/store.rs（预聚合数据存储）
  - server/src/api/admin.rs（管理 API）
  - server/web/src/pages/（前端界面）

## ADDED Requirements

### Requirement: 智能预聚合规则管理
系统应提供智能的预聚合规则管理功能，支持手动和自动创建规则。

#### Scenario: 用户手动创建预聚合规则
- **WHEN** 用户通过 Web 界面或 API 创建预聚合规则
- **THEN** 系统验证规则的 PromQL 表达式
- **AND** 创建预聚合任务并开始执行
- **AND** 存储预聚合数据

#### Scenario: 系统自动创建预聚合规则
- **WHEN** 某个查询在指定时间窗口内达到频率阈值
- **THEN** 系统自动分析查询模式
- **AND** 创建对应的预聚合规则
- **AND** 标记为自动创建的规则
- **AND** 开始执行预聚合任务

#### Scenario: 自动清理低频预聚合规则
- **WHEN** 自动创建的预聚合规则在指定时间内未达到查询频率阈值
- **THEN** 系统标记规则为待清理
- **AND** 删除预聚合数据
- **AND** 删除预聚合规则

### Requirement: 预聚合数据存储
系统应提供专门的预聚合数据存储机制。

#### Scenario: 存储预聚合数据
- **WHEN** 预聚合任务执行完成
- **THEN** 系统将结果存储到预聚合存储区
- **AND** 建立查询到预聚合数据的索引
- **AND** 记录预聚合数据的元数据

#### Scenario: 查询预聚合数据
- **WHEN** 查询匹配到预聚合数据
- **THEN** 系统直接返回预聚合数据
- **AND** 更新查询频率统计

### Requirement: 查询路由
系统应实现智能查询路由，自动匹配预聚合数据。

#### Scenario: 查询匹配预聚合数据
- **WHEN** 用户执行查询
- **THEN** 系统分析查询表达式
- **AND** 检查是否存在匹配的预聚合数据
- **AND** 如果存在，路由到预聚合数据
- **AND** 如果不存在，执行原始查询

#### Scenario: 预聚合数据优先级
- **WHEN** 存在多个匹配的预聚合规则
- **THEN** 系统选择最精确的预聚合数据
- **AND** 考虑时间范围和标签匹配度
- **AND** 返回最优结果

### Requirement: 查询频率统计
系统应实现查询频率统计和分析功能。

#### Scenario: 统计查询频率
- **WHEN** 用户执行查询
- **THEN** 系统记录查询表达式
- **AND** 更新查询频率计数器
- **AND** 记录查询时间戳

#### Scenario: 分析查询模式
- **WHEN** 系统定期分析查询模式
- **THEN** 识别高频查询
- **AND** 分析查询的优化潜力
- **AND** 生成预聚合建议

### Requirement: 预聚合任务调度
系统应实现预聚合任务的调度和执行。

#### Scenario: 调度预聚合任务
- **WHEN** 预聚合规则创建或更新
- **THEN** 系统创建对应的预聚合任务
- **AND** 根据配置的间隔调度任务
- **AND** 执行预聚合计算

#### Scenario: 预聚合任务失败处理
- **WHEN** 预聚合任务执行失败
- **THEN** 系统记录错误日志
- **AND** 标记任务状态为失败
- **AND** 在下次调度时重试

### Requirement: Web 管理界面支持
系统应在 Web 管理界面中提供预聚合规则管理功能。

#### Scenario: 查看预聚合规则
- **WHEN** 用户访问预聚合管理页面
- **THEN** 系统显示所有预聚合规则
- **AND** 区分手动和自动创建的规则
- **AND** 显示规则的查询频率统计

#### Scenario: 管理预聚合规则
- **WHEN** 用户创建、编辑或删除预聚合规则
- **THEN** 系统执行相应操作
- **AND** 更新预聚合任务
- **AND** 显示操作结果

#### Scenario: 查看预聚合建议
- **WHEN** 用户查看预聚合建议页面
- **THEN** 系统显示基于查询频率的建议
- **AND** 显示预期的性能提升
- **AND** 支持一键创建规则

### Requirement: 配置管理
系统应提供预聚合功能的配置管理。

#### Scenario: 配置自动创建规则
- **WHEN** 管理员配置自动创建规则参数
- **THEN** 系统保存配置
- **AND** 应用新的阈值和参数
- **AND** 更新自动创建逻辑

#### Scenario: 配置清理策略
- **WHEN** 管理员配置清理策略
- **THEN** 系统保存配置
- **AND** 应用新的清理规则
- **AND** 更新清理任务

## MODIFIED Requirements

### Requirement: 查询引擎
查询引擎需要支持预聚合数据路由。

#### Scenario: 查询执行流程
- **WHEN** 查询引擎接收查询请求
- **THEN** 首先检查预聚合数据索引
- **AND** 如果匹配到预聚合数据，使用预聚合数据
- **AND** 否则执行原始查询
- **AND** 记录查询频率统计

### Requirement: 规则管理器
规则管理器需要支持预聚合规则的智能管理。

#### Scenario: 规则管理
- **WHEN** 规则管理器加载规则
- **THEN** 区分普通记录规则和预聚合规则
- **AND** 为预聚合规则创建任务调度
- **AND** 启动自动创建和清理机制

## REMOVED Requirements
无移除的需求。

## 配置参数

### 自动创建配置
```yaml
pre_aggregation:
  auto_create:
    enabled: true
    # 查询频率阈值（次/小时）
    frequency_threshold: 20
    # 统计时间窗口（小时）
    time_window: 24
    # 最大自动创建规则数
    max_auto_rules: 100
    # 排除的查询模式（正则表达式）
    exclude_patterns:
      - "^up$"
      - "^ALERTS"
  
  auto_cleanup:
    enabled: true
    # 清理检查间隔（小时）
    check_interval: 6
    # 低频阈值（次/小时）
    low_frequency_threshold: 5
    # 清理前的观察期（小时）
    observation_period: 48
  
  storage:
    # 预聚合数据保留时间（天）
    retention_days: 30
    # 最大存储空间（GB）
    max_storage_gb: 100
    # 压缩算法
    compression: zstd
```

## 性能指标

### 预期性能提升
- 高频查询性能提升：50%-90%
- 存储空间增加：10%-30%（取决于预聚合规则数量）
- 查询路由延迟：< 5ms

### 监控指标
- `chronodb_preagg_rules_total` - 预聚合规则总数
- `chronodb_preagg_rules_auto_created` - 自动创建的规则数
- `chronodb_preagg_queries_routed` - 路由到预聚合数据的查询数
- `chronodb_preagg_query_latency_seconds` - 预聚合查询延迟
- `chronodb_preagg_storage_bytes` - 预聚合数据存储大小
- `chronodb_preagg_task_duration_seconds` - 预聚合任务执行时间
