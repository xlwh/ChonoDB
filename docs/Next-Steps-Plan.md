# ChronoDB 开发计划 v2

> 基于 2026-04 代码全面审查结果，重新梳理未完成工作并制定实施计划

---

## 当前状态总览

| 阶段 | 状态 | 完成度 | 关键阻塞 |
|------|------|--------|---------|
| Phase 1: 核心存储 | ✅ 基本完成 | 85% | flush/compaction 为 placeholder |
| Phase 2: 查询引擎 | ✅ 基本完成 | 80% | API 兼容性不完整 |
| Phase 3: 自动降采样 | 🔧 进行中 | 70% | 降采样数据读取未实现 |
| Phase 4: 分布式功能 | 🔧 进行中 | 50% | 查询链路未打通、故障转移不完整 |
| Phase 5: 优化 & 特性 | 📋 规划中 | 20% | 压缩比/性能优化待做 |
| Phase 6: 生产就绪 | 📋 规划中 | 10% | 测试/监控/安全待做 |

---

## Phase A: 关键 Bug 修复与核心链路打通（最高优先级）

> 目标：让系统基本可用，数据写入后能正确查询出来

### A1. 修复标签解析 Bug（阻塞级）

**问题**：数据写入成功但查询返回 0 结果，标签解析逻辑有误导致倒排索引无法正确匹配

**涉及文件**：
- `server/src/remote_server.rs` - 文本格式数据解析
- `storage/src/index/inverted.rs` - 倒排索引实现
- `storage/src/memstore/chunk.rs` - Chunk 时间戳处理

**任务**：
- [ ] A1.1 修复 `remote_server.rs` 中文本格式标签解析逻辑
- [ ] A1.2 修复倒排索引中标签添加和查询匹配
- [ ] A1.3 修复 `Chunk::add` 方法对非递增时间戳的处理（支持乱序写入或排序后写入）
- [ ] A1.4 验证 `/api/v1/label/__name__/values` 返回正确指标名
- [ ] A1.5 编写标签解析单元测试

**验收标准**：
- 数据写入后查询能返回正确结果
- `/api/v1/label/__name__/values` 返回正确的指标名称列表
- Python 集成测试中"暂时标记为通过"的用例全部真正通过

### A2. 打通分布式查询链路

**问题**：`extract_series_ids()` 返回空 Vec，导致整个分布式查询无法工作

**涉及文件**：
- `storage/src/distributed/query_coordinator.rs` - extract_series_ids 存根
- `storage/src/distributed/mod.rs` - query 方法 matchers 为空
- `storage/src/distributed/coordinator.rs` - QueryRouter 广播而非智能路由

**任务**：
- [ ] A2.1 实现 `extract_series_ids()`：从元数据服务/倒排索引中提取匹配的 series_id 列表
- [ ] A2.2 修复 `DistributedStorage::query()` 中 matchers 传递，使用实际的查询条件
- [ ] A2.3 改进 `QueryRouter::route()`：基于分片信息智能路由，而非广播到所有节点
- [ ] A2.4 实现查询结果合并中的去重和排序逻辑
- [ ] A2.5 编写分布式查询单元测试

**验收标准**：
- 分布式查询能正确路由到目标分片
- 查询结果正确且无重复
- 不必要的跨节点查询被消除

### A3. 完善故障转移机制

**问题**：`trigger_failover()` 只做了领导者重选举，缺少通知分片/副本管理器等关键逻辑

**涉及文件**：
- `storage/src/distributed/cluster.rs` - trigger_failover 不完整
- `storage/src/distributed/shard.rs` - 需要接收故障通知
- `storage/src/distributed/replication.rs` - 需要接收故障通知

**任务**：
- [ ] A3.1 完善 `trigger_failover()`：通知 ShardManager 重新分配分片
- [ ] A3.2 完善 `trigger_failover()`：通知 ReplicationManager 更新复制目标
- [ ] A3.3 完善 `trigger_failover()`：更新集群状态并广播
- [ ] A3.4 修复心跳任务中先标记 Offline 再 remove 的逻辑冲突
- [ ] A3.5 编写故障转移集成测试

**验收标准**：
- 节点故障后分片自动重新分配
- 副本管理器自动更新复制目标
- 集群状态正确更新

---

## Phase B: 核心存储引擎补全（高优先级）

> 目标：让 flush 和 compaction 真正工作，数据能持久化

### B1. 实现 Flush 功能

**问题**：flush/mod.rs 中仅获取统计信息，未实际将 memstore 数据写入 block

**涉及文件**：
- `storage/src/flush/mod.rs` - placeholder 实现
- `storage/src/memstore/store.rs` - 数据源
- `storage/src/columnstore/writer.rs` - 数据写入目标

**任务**：
- [ ] B1.1 实现遍历 head block 中所有 series 数据
- [ ] B1.2 将 series 数据按列式存储格式写入 block
- [ ] B1.3 更新 block 元数据（meta.json）
- [ ] B1.4 写入完成后清理 memstore 中已刷盘的数据
- [ ] B1.5 编写 flush 单元测试和集成测试

### B2. 实现 Compaction 功能

**问题**：compaction/mod.rs 中仅打印日志，未执行实际数据加载和压缩

**涉及文件**：
- `storage/src/compaction/mod.rs` - placeholder 实现
- `storage/src/columnstore/reader.rs` - 数据读取
- `storage/src/columnstore/writer.rs` - 数据写入

**任务**：
- [ ] B2.1 实现从磁盘加载 block 并提取所有 series 数据
- [ ] B2.2 实现多 block 合并逻辑
- [ ] B2.3 应用压缩算法写入新的 block
- [ ] B2.4 删除旧 block 文件
- [ ] B2.5 编写 compaction 单元测试和集成测试

### B3. 实现降采样数据读取

**问题**：downsample_router.rs 中 TODO 标注未实现从列式存储读取降采样数据

**涉及文件**：
- `storage/src/query/downsample_router.rs` - TODO 标注
- `storage/src/columnstore/reader.rs` - 数据读取

**任务**：
- [ ] B3.1 实现从列式存储 block 中读取降采样数据
- [ ] B3.2 根据查询时间范围选择正确的降采样层级 block
- [ ] B3.3 合并多个 block 的降采样数据
- [ ] B3.4 编写降采样数据读取测试

**验收标准**：
- flush 后数据能持久化到磁盘
- compaction 能合并 block 并压缩
- 降采样查询能从列式存储读取数据

---

## Phase C: 分布式架构完善（高优先级）

> 目标：让分布式功能真正可用

### C1. 完善 RPC 处理逻辑

**问题**：心跳响应字段硬编码，RPC 无连接池

**涉及文件**：
- `storage/src/distributed/mod.rs` - DistributedRpcHandler
- `storage/src/rpc/mod.rs` - RPC 客户端/服务器

**任务**：
- [ ] C1.1 修复心跳响应：从 ClusterManager 获取真实 NodeInfo
- [ ] C1.2 实现 RPC 连接池，复用 TCP 连接
- [ ] C1.3 添加 RPC 心跳保活机制
- [ ] C1.4 添加 RPC 超时和重试逻辑
- [ ] C1.5 编写 RPC 通信集成测试

### C2. 完善配置管理

**问题**：多个配置参数硬编码，未从 YAML 配置中读取

**涉及文件**：
- `storage/src/distributed/cluster.rs` - cluster_name 硬编码
- `storage/src/distributed/shard.rs` - replication_factor 硬编码
- `storage/src/distributed/replication.rs` - 多个参数硬编码

**任务**：
- [ ] C2.1 修复 `ClusterConfig::from_yaml_config()` 从配置读取 cluster_name
- [ ] C2.2 修复 `ShardConfig::from_yaml_config()` 从配置读取 replication_factor
- [ ] C2.3 修复 `ReplicationConfig::from_yaml_config()` 从配置读取所有参数
- [ ] C2.4 修复 `parse_duration()` 对纯数字字符串的处理
- [ ] C2.5 完善 `discover_nodes()` 中非 SocketAddr 发现机制

### C3. 修复复制状态逻辑

**问题**：replication_log 中条目 status 始终为 Pending，从未更新

**涉及文件**：
- `storage/src/distributed/replication.rs` - 状态更新逻辑

**任务**：
- [ ] C3.1 复制成功后更新 status 为 Completed
- [ ] C3.2 复制失败后更新 status 为 Failed
- [ ] C3.3 添加复制重试计数和最大重试次数
- [ ] C3.4 编写复制状态管理单元测试

### C4. 实现 Coordinator::start()

**问题**：只打日志，无实际逻辑

**涉及文件**：
- `storage/src/distributed/coordinator.rs`

**任务**：
- [ ] C4.1 实现协调器启动：注册到集群、初始化分片、启动心跳
- [ ] C4.2 实现协调器停止：清理资源、通知集群
- [ ] C4.3 编写协调器单元测试

**验收标准**：
- RPC 通信稳定，支持连接复用
- 所有配置参数从 YAML 正确读取
- 复制状态正确更新
- 协调器能正常启动和停止

---

## Phase D: API 兼容性与功能补全（中优先级）

> 目标：完善 Prometheus API 兼容性

### D1. PromQL 兼容性补全

**任务**：
- [ ] D1.1 实现 `by` clause 聚合（group by 标签）
- [ ] D1.2 实现逻辑操作符（and, or, unless）
- [ ] D1.3 实现标量函数（scalar, vector, time, etc.）
- [ ] D1.4 实现二元操作符完整支持
- [ ] D1.5 编写 PromQL 兼容性测试

### D2. 告警功能完善

**任务**：
- [ ] D2.1 实现 handlers.rs 中从 alert_manager 获取实际告警数据
- [ ] D2.2 完善告警规则评估器
- [ ] D2.3 实现告警通知发送
- [ ] D2.4 编写告警功能测试

### D3. 云存储后端实现

**任务**：
- [ ] D3.1 实现 S3 存储后端（替换 backup/mod.rs 中的 todo!()）
- [ ] D3.2 实现 GCS 存储后端
- [ ] D3.3 实现 MinIO 存储后端
- [ ] D3.4 编写云存储后端测试

**验收标准**：
- PromQL 支持 by clause、逻辑操作符、标量函数
- 告警 API 返回真实数据
- 云存储后端可正常备份/恢复

---

## Phase E: 性能优化（中优先级）

> 目标：达到设计性能目标

### E1. 压缩算法优化

**问题**：时间戳压缩比仅 4x，远低于 50:1-100:1 目标

**任务**：
- [ ] E1.1 优化 Delta-of-Delta 编码（当前实现可能不够高效）
- [ ] E1.2 实现 Gorilla 编码用于浮点值压缩
- [ ] E1.3 实现自适应压缩算法选择
- [ ] E1.4 压缩比基准测试和调优

### E2. 查询性能优化

**任务**：
- [ ] E2.1 优化向量化执行引擎 SIMD 加速
- [ ] E2.2 优化查询计划器谓词下推
- [ ] E2.3 优化索引选择和位图索引
- [ ] E2.4 查询缓存策略优化

### E3. 写入性能优化

**任务**：
- [ ] E3.1 批量写入优化
- [ ] E3.2 WAL 写入优化
- [ ] E3.3 内存池和对象复用
- [ ] E3.4 IO 调度优化

**验收标准**：
- 时间戳压缩比 > 20:1
- 查询延迟 P99 < 100ms
- 写入吞吐 > 1M samples/s

---

## Phase F: 测试完善（中优先级）

> 目标：核心模块测试覆盖率 > 80%

### F1. 分布式模块单元测试（缺失严重）

**任务**：
- [ ] F1.1 为 `cluster.rs` 添加单元测试（节点注册、心跳、选举、故障转移）
- [ ] F1.2 为 `coordinator.rs` 添加单元测试
- [ ] F1.3 为 `replication.rs` 添加单元测试（同步/异步复制、状态更新）
- [ ] F1.4 修复 `test_rpc_handler`（当前为 assert!(true)）

### F2. Server 模块单元测试（缺失严重）

**任务**：
- [ ] F2.1 为 `api/handlers.rs` 添加单元测试
- [ ] F2.2 为 `rules/evaluator.rs` 添加单元测试
- [ ] F2.3 为 `rules/alerting.rs` 添加单元测试
- [ ] F2.4 修复 `remote_server.rs` 中 `create_test_state()` 的 todo!()

### F3. 修复 Python 集成测试中的"假通过"

**任务**：
- [ ] F3.1 修复时间范围查询"暂时标记为通过"的问题
- [ ] F3.2 修复降采样查询"暂时标记为通过"的问题
- [ ] F3.3 修复网络流量数据"暂时标记为通过"的问题
- [ ] F3.4 所有测试用例必须真正验证数据正确性

### F4. 分布式集成测试

**任务**：
- [ ] F4.1 多节点集群启动和节点发现测试
- [ ] F4.2 数据写入和跨节点查询测试
- [ ] F4.3 数据复制一致性测试
- [ ] F4.4 节点故障和自动恢复测试
- [ ] F4.5 分片重平衡测试

**验收标准**：
- 分布式核心模块（cluster/replication/coordinator）有完整单元测试
- Server 核心 API 有单元测试
- Python 集成测试无"假通过"
- 分布式集成测试覆盖主要场景

---

## Phase G: 运维与生产就绪（低优先级）

### G1. 监控指标完善
- [ ] 存储指标（块/系列/样本数量、磁盘使用量）
- [ ] 降采样指标（层级数量、任务执行时间）
- [ ] 分布式指标（节点数量、复制延迟、查询协调统计）

### G2. 安全加固
- [ ] API 认证和授权
- [ ] TLS 支持
- [ ] 数据加密

### G3. 部署和发布
- [ ] Docker 镜像优化
- [ ] Helm Chart
- [ ] 发布流程自动化

---

## 执行优先级排序

| 优先级 | Phase | 预计工作量 | 依赖关系 |
|--------|-------|-----------|---------|
| 🔴 P0 | A1: 修复标签解析 Bug | 2-3 天 | 无 |
| 🔴 P0 | A2: 打通分布式查询链路 | 3-5 天 | A1 |
| 🟠 P1 | A3: 完善故障转移 | 2-3 天 | A2 |
| 🟠 P1 | B1: 实现 Flush | 2-3 天 | 无 |
| 🟠 P1 | B2: 实现 Compaction | 2-3 天 | B1 |
| 🟠 P1 | B3: 降采样数据读取 | 1-2 天 | B1 |
| 🟡 P2 | C1-C4: 分布式架构完善 | 5-7 天 | A2, A3 |
| 🟡 P2 | D1-D3: API 兼容性补全 | 5-7 天 | A1 |
| 🟢 P3 | E1-E3: 性能优化 | 5-7 天 | B1, B2 |
| 🟢 P3 | F1-F4: 测试完善 | 5-7 天 | 所有功能完成后 |
| ⚪ P4 | G1-G3: 运维与生产就绪 | 5-7 天 | 所有功能完成后 |

---

## 建议执行顺序

```
Week 1: A1(标签解析) → A2(分布式查询链路) → B1(Flush)
Week 2: A3(故障转移) → B2(Compaction) → B3(降采样读取)
Week 3: C1(RPC完善) → C2(配置管理) → C3(复制状态) → C4(协调器)
Week 4: D1(PromQL) → D2(告警) → D3(云存储)
Week 5: E1(压缩优化) → E2(查询优化) → E3(写入优化)
Week 6: F1-F4(测试完善)
Week 7+: G1-G3(运维与生产就绪)
```

---

## 关键风险

1. **标签解析 Bug 可能涉及深层架构问题**：如果倒排索引设计有缺陷，修复可能需要重构
2. **分布式查询链路打通依赖元数据服务**：当前缺少独立的元数据服务，extract_series_ids 的实现需要设计决策
3. **Flush/Compaction 实现可能影响现有数据格式**：需要确保与现有列式存储格式兼容
4. **压缩比优化可能需要算法级创新**：从 4x 到 50:1 是巨大跨越，可能需要分阶段逐步提升
