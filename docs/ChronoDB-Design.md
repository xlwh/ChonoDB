# ChronoDB 设计文档

## 1. 系统概述

### 1.1 项目背景
ChronoDB 是一个全新的时序数据库系统，设计目标是完全兼容 Prometheus 的协议和功能，同时实现：
- 查询性能提升 10 倍
- 存储成本降低 10 倍
- 支持单机本地存储和多机分布式部署
- 支持 DFS（分布式文件系统）和本地盘存储
- **智能自动降采样**：查询长时间段数据时自动使用降采样数据
- **智能预聚合**：自动识别高频查询并预先计算聚合结果
- **Web 管理界面**：提供可视化的数据查询、写入、监控和管理功能

### 1.2 核心目标
- 100% PromQL 兼容
- 100% Prometheus HTTP API 兼容
- 100% Prometheus Remote Write/Read 协议兼容
- 支持平滑迁移，无数据转换成本
- 智能降采样：自动选择最合适精度的数据

---

## 2. Prometheus 架构分析

### 2.1 Prometheus 核心组件
1. **TSDB（时序数据库）**：基于 LSM 树和 Gorilla 压缩的块存储
2. **PromQL 引擎**：查询语言执行引擎
3. **数据采集层**：Scrape Manager
4. **规则引擎**：Recording Rules 和 Alerting Rules
5. **API 服务层**：HTTP API 服务器

### 2.2 Prometheus 存储架构
- **Head Block**：内存中的实时数据块
- **WAL（Write-Ahead Log）**：预写日志保证数据持久性
- **Persistent Blocks**：持久化数据块，按时间分片
- **Compaction**：后台压缩合并机制

### 2.3 Prometheus 性能瓶颈
1. **查询性能**：
   - 单线程 PromQL 执行
   - 无查询优化器
   - 全表扫描式索引
   - 查询长时间段数据时需要处理海量原始样本
2. **存储性能**：
   - Gorilla 压缩比有限（约 1.37 字节/样本）
   - 无列式存储
   - 无数据分层
   - 无自动降采样机制

---

## 3. ChronoDB 系统架构

### 3.1 整体架构图

```
┌─────────────────────────────────────────────────────────────────┐
│                        API 兼容层                               │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐        │
│  │ PromQL 引擎  │  │ HTTP API     │  │ Remote Write │        │
│  │ (优化版)     │  │ (v1 兼容)    │  │ / Read       │        │
│  └──────────────┘  └──────────────┘  └──────────────┘        │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│              查询优化器 & 降采样路由引擎                        │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐        │
│  │ 逻辑优化器   │  │ 降采样选择器 │  │ 向量化执行   │        │
│  │ 谓词下推     │  │ 精度自适应   │  │ SIMD 批量    │        │
│  └──────────────┘  └──────────────┘  └──────────────┘        │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐        │
│  │ 预聚合路由器 │  │ 频率统计器   │  │ 查询标准化   │        │
│  │ 智能匹配     │  │ 滑动窗口     │  │ 表达式优化   │        │
│  └──────────────┘  └──────────────┘  └──────────────┘        │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                          存储引擎层                              │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │                    内存存储 (MemStore)                    │  │
│  │  ┌──────────┐  ┌──────────┐  ┌──────────┐            │  │
│  │  │ Delta    │  │ 索引缓存 │  │ Bloom    │            │  │
│  │  │ Encoding │  │          │  │ Filter   │            │  │
│  │  └──────────┘  └──────────┘  └──────────┘            │  │
│  └──────────────────────────────────────────────────────────┘  │
                              │
                              ▼
│  ┌──────────────────────────────────────────────────────────┐  │
│  │                自动降采样引擎 (Downsampler)               │  │
│  │  ┌──────────┐  ┌──────────┐  ┌──────────┐            │  │
│  │  │ 10s 精度 │  │ 1min 精度│  │ 5min 精度│            │  │
│  │  │ (raw)    │  │          │  │          │            │  │
│  │  └──────────┘  └──────────┘  └──────────┘            │  │
│  │  ┌──────────┐  ┌──────────┐  ┌──────────┐            │  │
│  │  │ 1h 精度  │  │ 1d 精度  │  │ 聚合函数  │            │  │
│  │  │          │  │          │  │ (min/max/│            │  │
│  │  └──────────┘  └──────────┘  │ avg/...) │            │  │
│  │                              └──────────┘            │  │
│  └──────────────────────────────────────────────────────────┘  │
                              │
                              ▼
│  ┌──────────────────────────────────────────────────────────┐  │
│  │                  列式存储 (ColumnStore)                   │  │
│  │  ┌──────────┐  ┌──────────┐  ┌──────────┐            │  │
│  │  │ 时间列   │  │ 值列     │  │ 标签列   │            │  │
│  │  │ (ZSTD)   │  │ (ZSTD+   │  │ (字典    │            │  │
│  │  │          │  │ 预测)    │  │ 压缩)    │            │  │
│  │  └──────────┘  └──────────┘  └──────────┘            │  │
│  └──────────────────────────────────────────────────────────┘  │
                              │
                              ▼
│  ┌──────────────────────────────────────────────────────────┐  │
│  │                  分布式协调层                               │  │
│  │  ┌──────────┐  ┌──────────┐  ┌──────────┐            │  │
│  │  │ 元数据   │  │ 数据分片 │  │ 副本管理 │            │  │
│  │  │ 管理     │  │ 路由     │  │          │            │  │
│  │  └──────────┘  └──────────┘  └──────────┘            │  │
│  └──────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                        存储抽象层                                │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐        │
│  │ 本地文件系统 │  │ DFS (HDFS)   │  │ 对象存储     │        │
│  │ (SSD/NVMe)   │  │ S3/GCS       │  │ (MinIO)      │        │
│  └──────────────┘  └──────────────┘  └──────────────┘        │
└─────────────────────────────────────────────────────────────────┘
```

### 3.2 核心模块设计

#### 3.2.1 API 兼容层
**职责**：完全兼容 Prometheus 的所有对外接口，并提供 Web 管理界面

**组件**：
1. **PromQL 引擎（兼容层）**：
   - 复用 Prometheus 的 parser 和 lexer
   - 重写执行引擎，支持向量化和并行化
   - 100% 语法兼容

2. **HTTP API v1**：
   - `/api/v1/query` - 即时查询
   - `/api/v1/query_range` - 范围查询
   - `/api/v1/series` - 系列元数据
   - `/api/v1/labels` - 标签名查询
   - `/api/v1/label/<name>/values` - 标签值查询
   - `/api/v1/targets` - 目标信息
   - `/api/v1/rules` - 规则信息
   - `/api/v1/alerts` - 告警信息

3. **Remote Write/Read**：
   - 完全兼容 Prometheus Remote Write 协议
   - 支持 snappy 压缩
   - 支持批量写入

4. **Web 管理界面**（新增）：
   - **数据查询**：PromQL 编辑器、即时/范围查询、结果可视化
   - **数据写入**：单条/批量写入、JSON 格式支持
   - **统计监控**：存储、查询、内存统计实时展示
   - **集群管理**：节点状态、分片分布可视化（分布式模式）
   - **告警管理**：规则配置、当前告警查看
   - **配置管理**：在线查看和修改系统配置
   - **预聚合管理**：规则管理、统计信息、预聚合建议
   
   **技术栈**：
   - 前端：React 18 + TypeScript + Vite
   - UI 组件：Ant Design 5.x
   - 图表：ECharts 5.x
   - 状态管理：Zustand + React Query
   - 嵌入方式：rust-embed（前端资源嵌入二进制文件）
   
   **访问方式**：
   ```
   http://localhost:9090/ui
   ```
   
   **界面特性**：
   - 响应式设计：支持桌面、平板和移动设备
   - 实时更新：统计数据自动刷新
   - 数据可视化：丰富的图表展示
   - 用户友好：直观的操作界面

#### 3.2.2 查询优化器 & 降采样路由引擎
**职责**：将 PromQL 转换为高效的物理执行计划，并自动选择最合适的降采样精度和预聚合数据

**组件**：
1. **逻辑优化器**：
   - 谓词下推
   - 列裁剪
   - 公共子表达式消除
   - 重排序优化

2. **降采样选择器**（核心创新）：
   - 根据查询时间范围自动选择精度
   - 基于查询函数智能选择（rate vs sum vs avg）
   - 支持用户覆盖（通过 hint 参数）

3. **向量化执行引擎**：
   - 基于 SIMD 的向量化处理
   - 批量处理替代逐行处理
   - 查询结果流水线

4. **预聚合路由器**（新增）：
   - 查询标准化：自动标准化查询表达式
   - 智能匹配：匹配预聚合规则和数据
   - 路由决策：选择使用预聚合数据或原始数据
   - 性能优化：路由延迟 < 5ms

5. **频率统计器**（新增）：
   - 滑动窗口算法：24 小时窗口实时统计
   - 高频识别：自动识别高频查询（> 20 次/小时）
   - 模式分析：分析查询模式和优化潜力

#### 3.2.3 智能预聚合引擎（新增）
**设计目标**：自动识别高频查询并预先计算聚合结果，查询性能提升 50%-90%

**核心功能**：

1. **查询频率统计**：
   - 滑动窗口算法：实时统计查询频率
   - 查询标准化：识别相似查询模式
   - 高频识别：自动识别高频查询（阈值可配置）

2. **自动规则管理**：
   - 自动创建：基于查询频率自动创建预聚合规则
   - 自动清理：清理低频使用的预聚合规则
   - 智能优化：根据查询模式动态调整策略

3. **查询路由优化**：
   - 智能路由：自动将查询路由到预聚合数据
   - 性能提升：高频查询性能提升 50%-90%
   - 透明切换：对用户完全透明

4. **分布式协调**：
   - 任务协调：分布式环境下的任务自动协调
   - 负载均衡：智能分配预聚合任务到各节点
   - 故障转移：节点故障时自动重新分配任务

**预聚合数据结构**：
```
PreAggregationRule
├── id: String                    # 规则 ID
├── name: String                  # 规则名称
├── expr: String                  # PromQL 表达式
├── labels: HashMap<String, String> # 附加标签
├── is_auto_created: bool         # 是否自动创建
├── query_frequency: u64          # 查询频率
├── status: RuleStatus            # 规则状态
└── evaluation_interval: u64      # 评估间隔

PreAggregatedData
├── rule_id: String               # 规则 ID
├── timestamp: i64                # 时间戳
├── value: f64                    # 聚合值
└── labels: Vec<(String, String)> # 标签
```

**配置示例**：
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

**性能提升**：
| 查询类型 | 原始性能 | 预聚合后 | 提升比例 |
|---------|---------|---------|---------|
| 简单聚合 | 100ms | 10ms | 90% |
| 复杂聚合 | 500ms | 50ms | 90% |
| 多维度聚合 | 1000ms | 100ms | 90% |
| 范围查询 | 2000ms | 400ms | 80% |

#### 3.2.3 自动降采样引擎 (Downsampler)
**设计目标**：自动生成和管理多精度数据，查询时自动选择最优精度

**降采样层级设计**：

| 层级 | 精度 | 保留时间 | 聚合函数 | 适用场景 |
|------|------|----------|----------|----------|
| L0 | 原始 (10s) | 7天 | - | 实时监控、精确查询 |
| L1 | 1分钟 | 30天 | min, max, avg, sum, count, last | 短期趋势分析 |
| L2 | 5分钟 | 90天 | min, max, avg, sum, count, last | 中期趋势分析 |
| L3 | 1小时 | 1年 | min, max, avg, sum, count, last | 长期趋势分析 |
| L4 | 1天 | 永久 | min, max, avg, sum, count, last | 年度对比、容量规划 |

**降采样策略**：
```
时间范围 → 自动选择精度
────────────────────────────────
< 1h    → L0 (原始数据)
1h-24h  → L1 (1min)
24h-7d  → L2 (5min)
7d-30d  → L3 (1h)
>30d    → L4 (1d)
```

**智能降采样选择算法**：
```go
func selectDownsampleLevel(queryRange time.Duration, funcType string) Level {
    // 1. 基础规则：根据时间范围选择
    baseLevel := selectByTimeRange(queryRange)
    
    // 2. 根据函数类型调整
    switch funcType {
    case "rate", "irate", "delta":
        // rate类函数需要较高精度，降低一级
        return max(baseLevel-1, L0)
    case "sum", "avg", "min", "max":
        // 聚合函数可以使用较低精度
        return baseLevel
    case "quantile":
        // 分位数函数需要原始精度
        return L0
    default:
        return baseLevel
    }
}
```

**降采样存储格式**：
```
Downsampled Series
├── series_id: uint64
├── level: Level (L0-L4)
├── resolution: Duration (10s, 1m, 5m, 1h, 1d)
├── min_value: float64
├── max_value: float64
├── avg_value: float64
├── sum_value: float64
├── count: uint64
├── last_value: float64
├── first_timestamp: int64
└── last_timestamp: int64
```

**降采样执行流程**：
```
原始数据 (L0)
    │
    ▼
┌─────────────────────┐
│  后台降采样任务     │
│  (异步执行)         │
└─────────────────────┘
    │
    ├─> 生成 L1 (1min)
    ├─> 生成 L2 (5min)
    ├─> 生成 L3 (1h)
    └─> 生成 L4 (1d)

查询时：
┌─────────────────────┐
│  查询时间范围分析   │
└─────────────────────┘
    │
    ▼
┌─────────────────────┐
│  选择最优精度层级   │
└─────────────────────┘
    │
    ▼
┌─────────────────────┐
│  从对应层级读取数据 │
└─────────────────────┘
    │
    ▼
┌─────────────────────┐
│  返回结果（透明）  │
└─────────────────────┘
```

#### 3.2.4 存储引擎层
**核心创新**：列式存储 + 多级压缩 + 智能索引 + 自动降采样

##### 3.2.4.1 内存存储 (MemStore)
**设计目标**：低延迟写入，高吞吐查询

**数据结构**：
```
MemStore
├── TimeSeriesMap (map[uint64]*TimeSeries)
├── LabelIndex (倒排索引)
├── BloomFilter (布隆过滤器)
└── WAL (Write-Ahead Log)
```

**编码方式**：
- **时间戳**：Delta-of-delta 编码（类似 Gorilla）
- **值**：Delta 编码 + 位压缩
- **标签**：字典编码 + 前缀压缩

##### 3.2.4.2 列式存储 (ColumnStore)
**设计目标**：高压缩比，快速扫描

**文件格式**：
```
ChronoDB Block Format
├── meta.json (元数据)
├── time.col (时间列)
├── value.col (值列)
├── labels.col (标签列)
├── downsample_L1.col (1min 降采样数据)
├── downsample_L2.col (5min 降采样数据)
├── downsample_L3.col (1h 降采样数据)
├── downsample_L4.col (1d 降采样数据)
├── index.idx (索引文件)
└── bloom.bf (布隆过滤器)
```

**压缩算法**：

| 列类型 | 压缩算法 | 预期压缩比 |
|--------|----------|------------|
| 时间戳 | ZSTD + Delta-of-delta | 50:1 - 100:1 |
| 浮点值 | ZSTD + 预测编码 | 20:1 - 50:1 |
| 标签名 | 字典编码 + 前缀树 | 100:1 - 500:1 |
| 标签值 | 字典编码 | 50:1 - 200:1 |
| 降采样数据 | ZSTD + 增量编码 | 100:1 - 200:1 |

**创新点**：
1. **自适应压缩**：根据数据特性自动选择最佳压缩算法
2. **预测编码**：利用时间序列的连续性进行值预测
3. **列式存储**：只读取需要的列，减少 IO
4. **降采样数据同块存储**：降采样数据与原始数据存储在同一块中，便于管理

##### 3.2.4.3 索引设计
**多级索引策略**：

1. **Level 0 - 布隆过滤器**：
   - 快速判断系列是否存在
   - 内存占用小

2. **Level 1 - 倒排索引**：
   - LabelName → LabelValue → SeriesID 映射
   - 支持高效的标签匹配

3. **Level 2 - 位图索引**：
   - 高频标签使用位图
   - 支持快速的交、并、差运算

4. **Level 3 - 范围索引**：
   - 时间范围索引
   - 值范围索引（可选）
   - 降采样层级索引

### 3.3 规则引擎

#### 3.3.1 规则引擎架构
**职责**：管理告警规则、记录规则和预聚合规则

**组件**：
1. **告警规则管理器**：
   - 加载和解析告警规则
   - 定期评估告警条件
   - 触发告警通知
   - 告警状态管理（pending, firing, resolved）

2. **记录规则管理器**：
   - 加载和解析记录规则
   - 定期执行预计算
   - 存储计算结果
   - 支持规则依赖

3. **预聚合规则管理器**（新增）：
   - 自动创建预聚合规则
   - 自动清理低频规则
   - 规则状态管理
   - 频率统计和分析
   - 手动规则管理

4. **规则评估器**：
   - 并行评估多个规则
   - 支持规则依赖
   - 失败重试机制
   - 性能监控

#### 3.3.2 预聚合规则管理流程

**自动创建流程**：
```
1. 查询执行 → 频率统计器记录
2. 频率统计 → 滑动窗口算法统计
3. 达到阈值 → 触发自动创建
4. 创建规则 → 启动预聚合任务
5. 定期执行 → 存储预聚合数据
```

**自动清理流程**：
```
1. 定期检查 → 扫描所有自动创建的规则
2. 频率分析 → 计算查询频率
3. 低于阈值 → 标记为待清理
4. 观察期结束 → 删除规则和数据
```

**分布式协调**：
```
1. 规则创建 → 协调器分配任务到节点
2. 节点调度器 → 检查任务是否分配给本节点
3. 执行任务 → 更新状态为 Running
4. 定期心跳 → 保持任务活跃状态
5. 任务完成 → 更新状态为 Completed
6. 节点故障 → 自动重新分配任务
```

#### 3.3.3 规则配置示例

**告警规则**：
```yaml
groups:
- name: system_alerts
  rules:
  - alert: HighCPUUsage
    expr: cpu_usage > 80
    for: 5m
    labels:
      severity: warning
    annotations:
      summary: "High CPU usage detected"
```

**记录规则**：
```yaml
groups:
- name: recording_rules
  rules:
  - record: job:http_requests:rate5m
    expr: sum(rate(http_requests_total[5m])) by (job)
```

**预聚合规则**：
```yaml
# 手动创建的预聚合规则
- name: dashboard_http_requests
  expr: sum(rate(http_requests_total[5m])) by (job, status)
  labels:
    dashboard: "main"
    priority: "high"
```

### 3.4 分布式架构

#### 3.4.1 部署模式

**模式一：单机模式**
```
ChronoDB (单节点)
├── 本地存储 (SSD/NVMe)
└── 完整功能
```

**模式二：分布式模式**
```
┌─────────────────────────────────────────────┐
│              查询协调器 (Query Coordinator)  │
│  - 查询路由                                  │
│  - 降采样精度选择                            │
│  - 结果聚合                                  │
│  - 负载均衡                                  │
└─────────────────────────────────────────────┘
                    │
        ┌───────────┼───────────┐
        ▼           ▼           ▼
┌─────────────┐ ┌─────────────┐ ┌─────────────┐
│  DataNode 1 │ │  DataNode 2 │ │  DataNode N │
│  - 存储分片 │ │  - 存储分片 │ │  - 存储分片 │
│  - 本地降采样│ │  - 本地降采样│ │  - 本地降采样│
│  - 本地计算 │ │  - 本地计算 │ │  - 本地计算 │
└─────────────┘ └─────────────┘ └─────────────┘
        │
        ▼
┌─────────────────────────────────────────────┐
│           元数据存储 (MetaStore)            │
│  - 集群拓扑                                 │
│  - 分片信息                                 │
│  - 副本位置                                 │
│  - 降采样配置                               │
└─────────────────────────────────────────────┘
```

#### 3.4.2 数据分片策略
**分片键**：SeriesID 的哈希值

**分片算法**：
- 一致性哈希
- 虚拟节点（避免数据倾斜）
- 支持动态扩容

#### 3.4.3 副本管理
**副本策略**：
- 默认 3 副本
- 跨机架/可用区分布
- Raft 共识协议保证一致性

---

## 4. 核心技术创新

### 4.1 查询优化：10 倍性能提升

#### 4.1.1 向量化执行
**传统方式**：
```go
// 逐行处理
for i := 0; i < len(samples); i++ {
    result[i] = process(samples[i])
}
```

**向量化方式**：
```go
// SIMD 批量处理
processBatch(samples, result)
```

**性能提升**：3-5 倍

#### 4.1.2 查询并行化
- **系列级并行**：不同系列并行处理
- **时间分片并行**：同一时间范围分片并行
- **操作符并行**：不同操作符流水线并行

**性能提升**：2-3 倍

#### 4.1.3 智能索引选择
- 基于成本的优化器
- 统计信息驱动
- 自适应索引

**性能提升**：2-3 倍

#### 4.1.4 自动降采样（新增）
**查询 30 天数据对比**：
- **Prometheus**：需要处理 30 * 24 * 60 * 6 = 259,200 个样本/系列
- **ChronoDB**：使用 L4 (1d) 精度，只需 30 个样本/系列

**性能提升**：8,000+ 倍（对于长时间段查询）

### 4.2 存储优化：10 倍成本降低

#### 4.2.1 列式存储 vs 行式存储

**Prometheus（行式）**：
```
[ts1, val1][ts2, val2][ts3, val3]...
```
- 每次查询读取整行
- 压缩效果有限

**ChronoDB（列式）**：
```
时间列: [ts1, ts2, ts3, ...]
值列:   [val1, val2, val3, ...]
标签列: [labels1, labels2, ...]
```
- 只读取需要的列
- 同类数据压缩效果好

#### 4.2.2 高级压缩算法

**ZSTD 压缩**：
- 比 LZ4 压缩率高 30%
- 解压速度快

**预测编码**：
```
previous = value[0]
for i := 1; i < len(values); i++ {
    predicted = predict(previous)
    delta = values[i] - predicted
    encode(delta)  // delta 通常很小
    previous = values[i]
}
```

**字典编码**：
- 标签名和标签值使用字典
- 重复数据只存储一次

#### 4.2.3 数据分层

| 层级 | 数据年龄 | 存储介质 | 压缩率 | 访问速度 |
|------|----------|----------|--------|----------|
| Hot  | < 24h    | NVMe SSD | 10:1   | 极快     |
| Warm | 24h-7d   | SSD      | 30:1   | 快       |
| Cold | 7d-30d   | HDD      | 50:1   | 中       |
| Archive | >30d  | 对象存储 | 100:1  | 慢       |

#### 4.2.4 降采样存储节省（新增）
**存储对比（1亿样本）**：
- **原始数据**：约 137MB（1.37 字节/样本）
- **L1 (1min)**：约 2.3MB（节省 98%）
- **L2 (5min)**：约 460KB（节省 99.7%）
- **L3 (1h)**：约 38KB（节省 99.97%）
- **L4 (1d)**：约 1.6KB（节省 99.998%）

**整体存储节省**：通过多精度降采样，历史数据存储成本降低 10-100 倍！

---

## 5. 数据模型 & 兼容设计

### 5.1 数据模型
**完全兼容 Prometheus 数据模型**：
```
metric {
    __name__: string,
    label1: value1,
    label2: value2,
    ...
}
[timestamp, value]
```

### 5.2 协议兼容

#### 5.2.1 PromQL 兼容
- 100% 语法兼容
- 100% 函数支持
- 100% 操作符支持
- 相同的语义和行为
- **新增 hint 参数**：`@downsample=auto|raw|1m|5m|1h|1d` 强制指定精度

#### 5.2.2 HTTP API 兼容
- 相同的端点
- 相同的请求参数
- 相同的响应格式
- 相同的错误码
- **新增响应头**：`X-ChronoDB-Downsample-Level: L2` 指示使用的降采样精度

#### 5.2.3 Remote Write/Read 兼容
- 完全兼容 protobuf 定义
- 支持相同的压缩方式
- 支持相同的批量大小

### 5.3 迁移方案

#### 5.3.1 双写迁移
```
Phase 1: 双写
┌─────────────┐
│  Prometheus  │────┐
└─────────────┘    │
                   ├─> ChronoDB
┌─────────────┐    │
│  新数据     │────┘
└─────────────┘

Phase 2: 数据回填
Prometheus 历史数据 ──> ChronoDB (同时生成降采样数据)

Phase 3: 切换读流量
查询流量 ──> ChronoDB (自动降采样生效)

Phase 4: 下线 Prometheus
```

#### 5.3.2 工具支持
- `chronodb-migrate`：数据迁移工具
- `chronodb-verify`：数据校验工具
- `chronodb-bench`：性能对比工具
- `chronodb-downsample`：手动降采样工具

---

## 6. 配置 & 部署

### 6.1 配置文件示例

```yaml
# chronodb.yaml

# 监听地址
listen_address: ":9090"

# 存储配置
storage:
  # 存储模式: standalone | distributed
  mode: standalone
  
  # 数据目录
  data_dir: "/var/lib/chronodb"
  
  # 存储后端: local | hdfs | s3
  backend: local
  
  # 本地存储配置
  local:
    path: "/var/lib/chronodb/data"
    max_disk_usage: "80%"
  
  # 分布式存储配置 (可选)
  distributed:
    metastore_endpoints: ["metastore1:2379", "metastore2:2379"]
    replication_factor: 3
    shard_count: 128

# 降采样配置（新增）
downsampling:
  # 是否启用自动降采样
  enabled: true
  
  # 降采样层级配置
  levels:
    - level: L0
      resolution: "10s"
      retention: "168h"  # 7天
    - level: L1
      resolution: "1m"
      retention: "720h"  # 30天
      functions: ["min", "max", "avg", "sum", "count", "last"]
    - level: L2
      resolution: "5m"
      retention: "2160h"  # 90天
      functions: ["min", "max", "avg", "sum", "count", "last"]
    - level: L3
      resolution: "1h"
      retention: "8760h"  # 1年
      functions: ["min", "max", "avg", "sum", "count", "last"]
    - level: L4
      resolution: "1d"
      retention: "87600h"  # 10年
      functions: ["min", "max", "avg", "sum", "count", "last"]
  
  # 降采样任务配置
  task:
    # 降采样任务执行间隔
    interval: "15m"
    # 并发数
    concurrency: 4
    # 超时时间
    timeout: "1h"

# 内存配置
memory:
  # MemStore 大小
  memstore_size: "4GB"
  # WAL 大小
  wal_size: "1GB"
  # 查询缓存大小
  query_cache_size: "2GB"

# 压缩配置
compression:
  # 时间列压缩
  time_column:
    algorithm: "zstd"
    level: 3
  # 值列压缩
  value_column:
    algorithm: "zstd"
    level: 3
    use_prediction: true
  # 标签列压缩
  label_column:
    algorithm: "dictionary"

# 查询配置
query:
  # 最大并发查询数
  max_concurrent: 100
  # 查询超时
  timeout: "2m"
  # 最大样本数
  max_samples: 50000000
  # 启用向量化执行
  enable_vectorized: true
  # 启用查询并行化
  enable_parallel: true
  # 启用自动降采样
  enable_auto_downsampling: true
  # 降采样精度选择策略: auto | conservative | aggressive
  downsample_policy: "auto"

# 数据保留策略
retention:
  # 热数据保留时间
  hot: "24h"
  # 温数据保留时间
  warm: "168h"  # 7d
  # 冷数据保留时间
  cold: "720h"   # 30d
  # 归档数据保留时间
  archive: "8760h" # 1y

# 日志配置
log:
  level: "info"
  format: "json"
  output: "/var/log/chronodb/chronodb.log"
```

### 6.2 部署方式

#### 6.2.1 Docker 部署
```dockerfile
FROM chronodb/chronodb:v1.0.0

COPY chronodb.yaml /etc/chronodb/

EXPOSE 9090

VOLUME ["/var/lib/chronodb"]

CMD ["chronodb", "--config.file=/etc/chronodb/chronodb.yaml"]
```

#### 6.2.2 Kubernetes 部署
```yaml
apiVersion: apps/v1
kind: StatefulSet
metadata:
  name: chronodb
spec:
  serviceName: chronodb
  replicas: 3
  selector:
    matchLabels:
      app: chronodb
  template:
    metadata:
      labels:
        app: chronodb
    spec:
      containers:
      - name: chronodb
        image: chronodb/chronodb:v1.0.0
        args:
        - --config.file=/etc/chronodb/chronodb.yaml
        ports:
        - containerPort: 9090
          name: http
        volumeMounts:
        - name: data
          mountPath: /var/lib/chronodb
        - name: config
          mountPath: /etc/chronodb
      volumes:
      - name: config
        configMap:
          name: chronodb-config
  volumeClaimTemplates:
  - metadata:
      name: data
    spec:
      accessModes: ["ReadWriteOnce"]
      resources:
        requests:
          storage: 100Gi
```

---

## 7. 性能指标 & 基准测试

### 7.1 设计目标

| 指标 | Prometheus | ChronoDB | 提升 |
|------|------------|----------|------|
| 查询延迟 (P99) | 1000ms | < 100ms | 10x |
| 查询延迟 (30d 范围) | 30000ms | < 300ms | 100x+ |
| 查询吞吐 (QPS) | 100 | 1000+ | 10x+ |
| 压缩比 | 1.37 字节/样本 | < 0.14 字节/样本 | 10x |
| 存储成本（含降采样）| 基准 | 0.05x | 20x+ |
| 写入吞吐 | 100k samples/s | 1M+ samples/s | 10x+ |

### 7.2 基准测试场景

#### 7.2.1 查询性能测试
- **简单查询**：`metric_name`
- **过滤查询**：`metric_name{label="value"}`
- **聚合查询**：`sum(metric_name) by (label)`
- **范围查询**：`rate(metric_name[5m])`
- **长范围查询（新增）**：`sum(metric_name[30d]) by (label)`
- **复杂查询**：多步操作符组合

#### 7.2.2 存储性能测试
- **数据集规模**：
  - 100k 系列
  - 10M 系列
  - 100M 系列
- **数据保留**：1d, 7d, 30d, 1y
- **写入负载**：恒定写入，突发写入
- **降采样 overhead**：测试降采样对写入性能的影响

---

## 8. 监控 & 运维

### 8.1 自监控指标
ChronoDB 暴露 Prometheus 格式的监控指标：

```
# 存储指标
chronodb_storage_blocks_total
chronodb_storage_series_total
chronodb_storage_samples_total
chronodb_storage_disk_usage_bytes

# 降采样指标（新增）
chronodb_downsample_levels_total
chronodb_downsample_samples_total
chronodb_downsample_task_duration_seconds
chronodb_downsample_task_runs_total
chronodb_query_downsample_level_count{level="L0|L1|L2|L3|L4"}

# 查询指标
chronodb_query_duration_seconds
chronodb_query_total
chronodb_query_samples_total

# 写入指标
chronodb_write_samples_total
chronodb_write_duration_seconds

# 内存指标
chronodb_memory_usage_bytes
chronodb_memstore_series_total
```

### 8.2 运维工具
- **`chronodb-tool`**：管理工具
  - `check`：数据完整性检查
  - `compact`：手动触发压缩
  - `backup`：数据备份
  - `restore`：数据恢复
  - `analyze`：统计分析
  - `downsample`：手动触发降采样
  - `downsample-status`：查看降采样状态
  - `downsample-config`：查看/修改降采样配置

---

## 9. 技术栈

### 9.1 编程语言
- **Rust**：存储引擎核心、降采样引擎（性能关键路径）
- **Go**：API 层、分布式协调（生态兼容）

### 9.2 核心依赖
- **ZSTD**：压缩算法
- **SIMD**：向量化执行
- **RocksDB**：可选的 KV 存储引擎
- **etcd**：元数据存储（分布式模式）
- **Arrow**：列式内存格式（可选）

---

## 10. 开发路线图

### Phase 1: 核心存储 (Month 1-2)
- [ ] 内存存储引擎
- [ ] 列式存储格式
- [ ] 基础压缩算法
- [ ] WAL 实现

### Phase 2: 查询引擎 (Month 3-4)
- [ ] PromQL parser 集成
- [ ] 向量化执行引擎
- [ ] 基础查询优化器
- [ ] HTTP API v1

### Phase 3: 自动降采样 (Month 5)（新增）
- [ ] 降采样数据结构设计
- [ ] 后台降采样任务
- [ ] 降采样选择器
- [ ] 查询时自动路由

### Phase 4: 分布式 (Month 6-7)
- [ ] 数据分片
- [ ] 副本管理
- [ ] 查询协调
- [ ] 元数据服务

### Phase 5: 优化 & 特性 (Month 8-9)
- [ ] 高级查询优化
- [ ] 数据分层
- [ ] 自适应压缩
- [ ] 迁移工具

### Phase 6: 生产就绪 (Month 10)
- [ ] 性能调优
- [ ] 监控完善
- [ ] 文档完善
- [ ] GA 发布

---

## 11. 总结

ChronoDB 通过以下创新实现 10x 性能提升和 10x+ 成本降低：

1. **列式存储**：替代行式存储，大幅提升压缩比
2. **高级压缩**：ZSTD + 预测编码 + 字典编码
3. **向量化执行**：SIMD 批量处理
4. **查询并行化**：多维度并行
5. **智能索引**：多级索引 + 基于成本的优化器
6. **数据分层**：热/温/冷/归档分层存储
7. **分布式架构**：支持水平扩展
8. **自动降采样**：
   - 5级精度（10s, 1m, 5m, 1h, 1d）
   - 查询时自动选择最优精度
   - 长时间段查询性能提升 100x+
   - 历史数据存储成本再降 10-100 倍
9. **智能预聚合**（新增）：
   - 自动识别高频查询（> 20 次/小时）
   - 自动创建和管理预聚合规则
   - 查询性能提升 50%-90%
   - 分布式任务协调和故障转移
   - 存储空间增加仅 10%-30%
10. **Web 管理界面**（新增）：
    - 可视化数据查询和写入
    - 实时统计监控和图表展示
    - 集群管理和配置管理
    - 告警规则管理和预聚合管理
    - 响应式设计，支持多设备

同时，ChronoDB 保持 100% Prometheus 兼容，确保平滑迁移。

---

## 附录：降采样使用示例

### 示例 1：自动降采样（默认）
```promql
# 查询过去 30 天的数据，自动使用 L4 (1d) 精度
sum(rate(http_requests_total[5m])) by (job)
```

### 示例 2：强制使用原始精度
```promql
# 即使查询 30 天，也强制使用原始数据
sum(rate(http_requests_total[5m] @downsample=raw)) by (job)
```

### 示例 3：强制指定精度
```promql
# 强制使用 1 小时精度
sum(rate(http_requests_total[5m] @downsample=1h)) by (job)
```

### 响应示例
```
HTTP/1.1 200 OK
X-ChronoDB-Downsample-Level: L2
Content-Type: application/json

{
  "status": "success",
  "data": {
    "resultType": "matrix",
    "result": [...]
  }
}
```
