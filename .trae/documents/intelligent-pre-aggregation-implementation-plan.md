# 智能预聚合功能实施计划

## 概述

本计划详细描述了 ChronoDB 智能预聚合功能的实施步骤，包括数据模型设计、核心功能实现、API 开发、Web 界面集成等。

## 实施阶段

### 阶段 1: 数据模型和基础设施（第 1-2 天）

#### 1.1 设计数据模型
**文件**: `storage/src/model/pre_aggregation.rs`

创建以下数据结构：
```rust
// 预聚合规则
pub struct PreAggregationRule {
    pub id: String,
    pub name: String,
    pub expr: String,
    pub labels: HashMap<String, String>,
    pub is_auto_created: bool,
    pub created_at: i64,
    pub query_frequency: u64,
    pub last_query_time: i64,
    pub last_evaluation: i64,
    pub status: RuleStatus,
}

// 查询频率统计
pub struct QueryFrequencyStats {
    pub normalized_query: String,
    pub frequency: u64,
    pub last_query_time: i64,
    pub query_count_window: Vec<QueryRecord>,
}

// 预聚合数据索引
pub struct PreAggregationIndex {
    pub rule_id: String,
    pub query_pattern: String,
    pub label_matchers: Vec<LabelMatcher>,
    pub data_location: DataLocation,
}
```

#### 1.2 扩展存储引擎
**文件**: `storage/src/memstore/store.rs`

- 添加预聚合数据存储区域
- 实现预聚合数据的写入和读取接口
- 添加预聚合数据索引管理

### 阶段 2: 查询频率统计系统（第 3-4 天）

#### 2.1 实现查询频率统计模块
**文件**: `storage/src/query/frequency.rs`

功能：
- 查询表达式标准化（去除时间范围、常量等变量）
- 滑动窗口频率统计
- 查询模式分析
- 高频查询识别

关键算法：
```rust
// 查询标准化
fn normalize_query(query: &str) -> String {
    // 1. 移除时间范围参数
    // 2. 标准化标签匹配器顺序
    // 3. 简化常量表达式
    // 4. 生成标准化的查询签名
}

// 滑动窗口频率统计
fn update_frequency(query: &str, timestamp: i64) {
    // 1. 标准化查询
    // 2. 更新频率计数器
    // 3. 清理过期记录
    // 4. 检查是否达到阈值
}
```

#### 2.2 集成到查询引擎
**文件**: `storage/src/query/engine.rs`

- 在查询执行前后记录频率
- 提供频率查询接口
- 支持频率统计配置

### 阶段 3: 智能规则管理器（第 5-7 天）

#### 3.1 创建预聚合管理器
**文件**: `server/src/rules/pre_aggregation.rs`

功能：
- 规则 CRUD 操作
- 自动创建规则逻辑
- 自动清理规则逻辑
- 规则优先级管理
- 规则状态管理

关键逻辑：
```rust
impl PreAggregationManager {
    // 自动创建规则
    pub async fn auto_create_rules(&mut self) -> Result<Vec<PreAggregationRule>> {
        // 1. 获取高频查询列表
        // 2. 分析查询优化潜力
        // 3. 生成预聚合规则
        // 4. 验证规则有效性
        // 5. 创建规则并启动任务
    }
    
    // 自动清理规则
    pub async fn auto_cleanup_rules(&mut self) -> Result<Vec<String>> {
        // 1. 扫描所有自动创建的规则
        // 2. 检查查询频率
        // 3. 标记低频规则
        // 4. 删除规则和数据
    }
}
```

#### 3.2 扩展规则管理器
**文件**: `server/src/rules/mod.rs`

- 添加预聚合规则类型
- 集成预聚合管理器
- 统一规则管理接口

### 阶段 4: 查询路由器（第 8-9 天）

#### 4.1 实现查询路由器
**文件**: `storage/src/query/router.rs`

功能：
- 查询表达式匹配
- 预聚合数据查找
- 路由决策
- 性能优化

关键算法：
```rust
impl QueryRouter {
    // 路由查询
    pub async fn route(&self, query: &str, start: i64, end: i64) -> RouteDecision {
        // 1. 标准化查询
        // 2. 查找匹配的预聚合规则
        // 3. 检查预聚合数据可用性
        // 4. 选择最优路由
        // 5. 返回路由决策
    }
    
    // 匹配预聚合规则
    fn match_rules(&self, query: &str) -> Vec<&PreAggregationRule> {
        // 1. 精确匹配
        // 2. 部分匹配
        // 3. 标签匹配
        // 4. 按优先级排序
    }
}
```

#### 4.2 集成到查询引擎
**文件**: `storage/src/query/engine.rs`

- 在查询执行前调用路由器
- 根据路由决策执行查询
- 记录路由统计信息

### 阶段 5: 任务调度器（第 10-11 天）

#### 5.1 创建任务调度器
**文件**: `server/src/rules/scheduler.rs`

功能：
- 任务调度
- 任务执行
- 失败重试
- 并发控制
- 状态监控

关键实现：
```rust
impl PreAggregationScheduler {
    // 启动调度器
    pub async fn start(&mut self) {
        // 1. 加载所有活跃规则
        // 2. 创建定时任务
        // 3. 启动任务执行器
        // 4. 启动监控循环
    }
    
    // 执行预聚合任务
    async fn execute_task(&self, rule: &PreAggregationRule) -> Result<()> {
        // 1. 执行 PromQL 查询
        // 2. 处理结果
        // 3. 写入预聚合数据
        // 4. 更新索引
        // 5. 更新规则状态
    }
}
```

### 阶段 6: 配置管理（第 12 天）

#### 6.1 扩展配置结构
**文件**: `server/src/config.rs`

添加预聚合配置：
```rust
pub struct PreAggregationConfig {
    pub auto_create: AutoCreateConfig,
    pub auto_cleanup: AutoCleanupConfig,
    pub storage: PreAggregationStorageConfig,
}

pub struct AutoCreateConfig {
    pub enabled: bool,
    pub frequency_threshold: u64,
    pub time_window: u64,
    pub max_auto_rules: usize,
    pub exclude_patterns: Vec<String>,
}

pub struct AutoCleanupConfig {
    pub enabled: bool,
    pub check_interval: u64,
    pub low_frequency_threshold: u64,
    pub observation_period: u64,
}
```

#### 6.2 实现配置加载和验证
- 配置文件解析
- 配置验证
- 默认值设置
- 配置热更新

### 阶段 7: 管理 API（第 13-14 天）

#### 7.1 实现 API 端点
**文件**: `server/src/api/admin.rs`

端点列表：
```
GET    /api/admin/preagg/rules          - 获取规则列表
POST   /api/admin/preagg/rules          - 创建规则
GET    /api/admin/preagg/rules/:id      - 获取规则详情
PUT    /api/admin/preagg/rules/:id      - 更新规则
DELETE /api/admin/preagg/rules/:id      - 删除规则
GET    /api/admin/preagg/stats          - 获取统计信息
GET    /api/admin/preagg/suggestions    - 获取预聚合建议
POST   /api/admin/preagg/suggestions/:id/apply - 应用建议
```

#### 7.2 实现 API 处理函数
- 请求验证
- 业务逻辑调用
- 响应格式化
- 错误处理

### 阶段 8: Web 管理界面（第 15-17 天）

#### 8.1 创建前端页面
**文件**: `server/web/src/pages/PreAggregation/`

页面组件：
- `index.tsx` - 主页面
- `RulesList.tsx` - 规则列表
- `RuleEditor.tsx` - 规则编辑器
- `Suggestions.tsx` - 预聚合建议
- `Statistics.tsx` - 统计信息

#### 8.2 实现 API 客户端
**文件**: `server/web/src/api/preagg.ts`

- 封装所有预聚合 API 调用
- 类型定义
- 错误处理

#### 8.3 集成到导航
- 添加预聚合菜单项
- 配置路由

### 阶段 9: 监控指标（第 18 天）

#### 9.1 添加 Prometheus 指标
**文件**: `storage/src/metrics/mod.rs`

指标列表：
```rust
// 规则相关
pub static PREAGG_RULES_TOTAL: Lazy<Counter> = ...;
pub static PREAGG_RULES_AUTO_CREATED: Lazy<Counter> = ...;

// 查询相关
pub static PREAGG_QUERIES_ROUTED: Lazy<Counter> = ...;
pub static PREAGG_QUERY_LATENCY: Lazy<Histogram> = ...;

// 存储相关
pub static PREAGG_STORAGE_BYTES: Lazy<Gauge> = ...;

// 任务相关
pub static PREAGG_TASK_DURATION: Lazy<Histogram> = ...;
```

#### 9.2 集成到各模块
- 规则管理器中记录规则指标
- 查询路由器中记录查询指标
- 任务调度器中记录任务指标

### 阶段 10: 自动机制（第 19-20 天）

#### 10.1 实现后台任务
**文件**: `server/src/rules/auto_manager.rs`

功能：
- 定期分析查询频率
- 自动创建规则
- 自动清理规则
- 发送通知

关键实现：
```rust
impl AutoManager {
    // 启动自动管理
    pub async fn start(&mut self) {
        // 1. 启动频率分析任务
        // 2. 启动自动创建任务
        // 3. 启动自动清理任务
    }
    
    // 频率分析任务
    async fn analyze_frequency(&self) {
        // 1. 扫描查询频率统计
        // 2. 识别高频查询
        // 3. 生成预聚合建议
        // 4. 触发自动创建
    }
}
```

### 阶段 11: 集成测试（第 21-22 天）

#### 11.1 编写测试用例
**文件**: `storage/tests/pre_aggregation_test.rs`

测试场景：
- 手动创建规则完整流程
- 自动创建规则完整流程
- 查询路由功能
- 自动清理功能
- 配置管理功能
- 性能测试

#### 11.2 端到端测试
- 启动服务器
- 执行高频查询
- 验证自动创建
- 验证查询路由
- 验证自动清理

### 阶段 12: 性能优化（第 23-24 天）

#### 12.1 性能基准测试
- 查询路由延迟测试
- 预聚合任务执行时间测试
- 内存和 CPU 使用测试
- 并发性能测试

#### 12.2 性能优化
- 优化查询匹配算法
- 优化索引结构
- 优化内存使用
- 优化并发控制

### 阶段 13: 文档编写（第 25 天）

#### 13.1 编写用户文档
**文件**: `docs/pre-aggregation.md`

内容：
- 功能介绍
- 使用指南
- 配置说明
- 最佳实践
- 故障排除

#### 13.2 更新现有文档
- 更新 README.md
- 更新 Usage.md
- 更新 API.md

## 关键技术点

### 1. 查询标准化算法
```
输入: sum(rate(http_requests_total{job="api"}[5m])) by (status)
步骤:
1. 提取查询结构
2. 标准化标签顺序
3. 移除时间范围参数
4. 生成查询签名
输出: sum(rate(http_requests_total{job="api"}[5m])) by (status)
```

### 2. 查询匹配算法
```
输入: 用户查询 + 预聚合规则列表
步骤:
1. 标准化用户查询
2. 精确匹配规则
3. 部分匹配规则（标签子集）
4. 计算匹配度得分
5. 选择最优规则
输出: 最优预聚合规则或 None
```

### 3. 频率统计算法
```
数据结构: 滑动窗口计数器
窗口大小: 24 小时
更新频率: 每次查询
清理策略: 每小时清理过期记录
```

## 风险和缓解措施

### 风险 1: 预聚合数据占用过多存储
**缓解措施**:
- 设置最大存储限制
- 自动清理低频规则
- 数据压缩

### 风险 2: 自动创建规则过多影响性能
**缓解措施**:
- 设置最大规则数量限制
- 排除简单查询模式
- 限制并发任务数

### 风险 3: 查询路由延迟过高
**缓解措施**:
- 优化索引结构
- 缓存匹配结果
- 异步路由决策

## 验收标准

### 功能验收
- [ ] 手动创建规则功能正常
- [ ] 自动创建规则基于频率阈值触发
- [ ] 自动清理规则基于低频阈值触发
- [ ] 查询路由正确匹配预聚合数据
- [ ] Web 界面功能完整

### 性能验收
- [ ] 查询路由延迟 < 5ms
- [ ] 高频查询性能提升 50%-90%
- [ ] 存储空间增加 < 30%
- [ ] 系统稳定运行

### 质量验收
- [ ] 所有测试通过
- [ ] 代码覆盖率 > 80%
- [ ] 文档完整清晰
- [ ] 无严重 bug

## 时间估算

- **总工期**: 25 个工作日
- **核心功能**: 15 天
- **测试优化**: 7 天
- **文档编写**: 3 天

## 资源需求

- **开发人员**: 1-2 人
- **测试环境**: 1 套
- **文档编写**: 1 人

## 后续优化方向

1. **智能推荐**: 基于查询模式智能推荐预聚合规则
2. **成本优化**: 根据存储成本和查询收益动态调整规则
3. **分布式支持**: 在分布式环境下协调预聚合任务
4. **机器学习**: 使用 ML 预测查询模式，提前创建规则
