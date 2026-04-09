# Phase 1: 测试和验证 - 实施计划

## 概述

本计划详细描述了ChronoDB项目Phase 1的测试和验证工作，包括单元测试完善、集成测试和性能基准测试。

---

## 代码库现状分析

### 现有测试文件
1. `storage/tests/integration_tests.rs` - 基础集成测试
   - ✅ 写入和读取测试
   - ✅ 按标签查询测试
   - ✅ 时间范围查询测试
   - ✅ 部分分布式测试

2. `storage/tests/storage_integration_test.rs` - 存储集成测试
3. `storage/tests/backup_integration_test.rs` - 备份集成测试
4. `storage/tests/fault_injection_test.rs` - 故障注入测试

### 缺少的测试
- ❌ 降采样系统的单元测试
- ❌ 分布式系统的完整单元测试
- ❌ 列式存储的单元测试
- ❌ 查询优化器的单元测试
- ❌ 完整的端到端测试
- ❌ 性能基准测试框架

---

## 实施计划

### 任务1: 为降采样系统添加单元测试

**目标**: 为降采样系统的核心模块添加完整的单元测试

**文件和模块**
1. `storage/src/downsample/processor.rs` - 降采样处理器
2. `storage/src/downsample/scheduler.rs` - 降采样调度器
3. `storage/src/downsample/worker.rs` - 降采样工作器
4. `storage/src/downsample/task.rs` - 降采样任务

**步骤**

#### 1.1 创建 `storage/src/downsample/tests.rs`
```rust
// 测试框架设置
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    
    // 辅助函数
    fn create_test_samples(count: usize) -> Vec<Sample> {
        (0..count)
            .map(|i| Sample::new(i as i64 * 1000, i as f64))
            .collect()
    }
}
```

#### 1.2 测试DownsampleProcessor
- [ ] 测试基础降采样功能
- [ ] 测试不同降采样级别（L0-L4）
- [ ] 测试不同聚合函数（min, max, avg, sum, count, last）
- [ ] 测试边界条件（空输入、单样本）
- [ ] 测试降采样点的数据结构

#### 1.3 测试DownsampleTask
- [ ] 测试任务创建
- [ ] 测试任务状态转换
- [ ] 测试任务超时
- [ ] 测试任务重试逻辑

#### 1.4 测试DownsampleScheduler
- [ ] 测试调度器初始化
- [ ] 测试任务提交
- [ ] 测试优先级队列（不同优先级的任务顺序）
- [ ] 测试任务取消
- [ ] 测试并发控制

#### 1.5 测试DownsampleWorker
- [ ] 测试工作器初始化
- [ ] 测试任务处理
- [ ] 测试降采样数据持久化（需要临时目录）
- [ ] 测试错误处理

---

### 任务2: 为分布式系统添加单元测试

**目标**: 为分布式系统的核心模块添加完整的单元测试

**文件和模块**
1. `storage/src/distributed/cluster.rs` - 集群管理器
2. `storage/src/distributed/replication.rs` - 复制管理器
3. `storage/src/distributed/query_coordinator.rs` - 查询协调器
4. `storage/src/distributed/shard.rs` - 分片管理器

**步骤**

#### 2.1 创建 `storage/src/distributed/tests.rs`
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
}
```

#### 2.2 测试ClusterManager
- [ ] 测试集群管理器初始化
- [ ] 测试节点注册
- [ ] 测试节点发现
- [ ] 测试心跳机制（模拟）
- [ ] 测试领导者选举（模拟）
- [ ] 测试节点状态管理

#### 2.3 测试ReplicationManager
- [ ] 测试复制管理器初始化
- [ ] 测试同步复制
- [ ] 测试异步复制
- [ ] 测试复制队列管理
- [ ] 测试最小副本数检查
- [ ] 测试复制失败重试
- [ ] 测试复制日志

#### 2.4 测试QueryCoordinator
- [ ] 测试查询协调器初始化
- [ ] 测试查询路由（分片分配）
- [ ] 测试一致性哈希
- [ ] 测试并行查询执行
- [ ] 测试结果合并
- [ ] 测试查询缓存
- [ ] 测试聚合查询（sum, avg, min, max, count）

#### 2.5 测试ShardManager
- [ ] 测试分片管理器初始化
- [ ] 测试系列到分片的映射
- [ ] 测试分片到节点的分配
- [ ] 测试虚拟节点
- [ ] 测试节点添加/移除

---

### 任务3: 为列式存储添加单元测试

**目标**: 为列式存储的核心模块添加完整的单元测试

**文件和模块**
1. `storage/src/columnstore/block.rs` - 块结构
2. `storage/src/columnstore/column.rs` - 列结构
3. `storage/src/columnstore/writer.rs` - 块写入器
4. `storage/src/columnstore/reader.rs` - 块读取器

**步骤**

#### 3.1 创建 `storage/src/columnstore/tests.rs`
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
}
```

#### 3.2 测试Block和BlockMeta
- [ ] 测试BlockMeta创建
- [ ] 测试BlockMeta编码/解码
- [ ] 测试Block创建
- [ ] 测试块元数据验证

#### 3.3 测试Column和ColumnBuilder
- [ ] 测试时间列构建（delta-of-delta编码）
- [ ] 测试值列构建（预测编码）
- [ ] 测试标签列构建（字典编码）
- [ ] 测试列编码/解码
- [ ] 测试压缩比验证
- [ ] 测试不同压缩级别

#### 3.4 测试BlockWriter
- [ ] 测试块写入器初始化
- [ ] 测试添加时间序列
- [ ] 测试添加降采样数据
- [ ] 测试块写入（需要临时目录）
- [ ] 测试元数据正确性

#### 3.5 测试BlockReader
- [ ] 测试块读取器初始化
- [ ] 测试读取时间序列
- [ ] 测试读取降采样数据
- [ ] 测试时间范围过滤
- [ ] 测试索引读取
- [ ] 测试布隆过滤器

---

### 任务4: 为查询引擎添加单元测试

**目标**: 为查询引擎的核心模块添加完整的单元测试

**文件和模块**
1. `storage/src/query/parser.rs` - PromQL解析器
2. `storage/src/query/planner.rs` - 查询计划器
3. `storage/src/query/executor.rs` - 查询执行器
4. `storage/src/query/optimizer.rs` - 查询优化器
5. `storage/src/query/cost_optimizer.rs` - 成本优化器

**步骤**

#### 4.1 创建 `storage/src/query/tests.rs`
```rust
#[cfg(test)]
mod tests {
    use super::*;
}
```

#### 4.2 测试PromQL解析器
- [ ] 测试基本查询解析
- [ ] 测试聚合查询解析
- [ ] 测试标签过滤解析
- [ ] 测试时间范围解析
- [ ] 测试复杂查询解析

#### 4.3 测试查询计划器
- [ ] 测试查询计划创建
- [ ] 测试计划验证
- [ ] 测试计划序列化

#### 4.4 测试查询执行器
- [ ] 测试基本查询执行
- [ ] 测试聚合查询执行
- [ ] 测试标签过滤
- [ ] 测试时间范围过滤

#### 4.5 测试查询优化器
- [ ] 测试逻辑优化（谓词下推）
- [ ] 测试列裁剪
- [ ] 测试公共子表达式消除

#### 4.6 测试成本优化器
- [ ] 测试成本估算
- [ ] 测试索引选择
- [ ] 测试计划选择

---

### 任务5: 创建端到端集成测试

**目标**: 创建完整的端到端测试，验证各模块之间的协作

**文件**
1. 扩展 `storage/tests/integration_tests.rs`
2. 创建新的 `storage/tests/downsample_integration_test.rs`
3. 创建新的 `storage/tests/distributed_integration_test.rs`

**步骤**

#### 5.1 降采样端到端测试
**文件**: `storage/tests/downsample_integration_test.rs`

- [ ] 测试写入原始数据
- [ ] 测试触发降采样任务
- [ ] 测试查询降采样数据
- [ ] 测试自动降采样选择
- [ ] 测试降采样数据持久化（重启验证）

#### 5.2 分布式集成测试
**文件**: `storage/tests/distributed_integration_test.rs`

- [ ] 测试多节点集群启动（模拟）
- [ ] 测试节点发现
- [ ] 测试数据复制
- [ ] 测试分布式查询
- [ ] 测试查询结果一致性

#### 5.3 数据持久化测试
**扩展**: `storage/tests/integration_tests.rs`

- [ ] 测试WAL写入和恢复
- [ ] 测试重启后数据恢复
- [ ] 测试块持久化和读取

#### 5.4 故障注入测试
**扩展**: `storage/tests/fault_injection_test.rs`

- [ ] 测试节点故障
- [ ] 测试网络分区
- [ ] 测试数据一致性
- [ ] 测试故障恢复

---

### 任务6: 创建性能基准测试框架

**目标**: 建立性能基准测试框架，建立性能基线

**文件**
1. 创建 `storage/benches/storage_benchmark.rs`
2. 创建 `storage/benches/query_benchmark.rs`
3. 创建 `storage/benches/downsample_benchmark.rs`

**步骤**

#### 6.1 存储性能基准测试
**文件**: `storage/benches/storage_benchmark.rs`

- [ ] 测试单节点写入吞吐量
- [ ] 测试批量写入性能
- [ ] 测试延迟（P50, P95, P99）
- [ ] 测试压缩比
- [ ] 测试内存占用
- [ ] 测试磁盘IO

#### 6.2 查询性能基准测试
**文件**: `storage/benches/query_benchmark.rs`

- [ ] 测试简单查询延迟
- [ ] 测试聚合查询延迟
- [ ] 测试范围查询延迟
- [ ] 测试标签过滤性能
- [ ] 测试并发查询性能
- [ ] 测试查询缓存效果

#### 6.3 降采样性能基准测试
**文件**: `storage/benches/downsample_benchmark.rs`

- [ ] 测试降采样任务执行时间
- [ ] 测试降采样查询性能（对比原始查询）
- [ ] 测试降采样数据存储效率
- [ ] 测试长时间段查询（对比降采样效果）

#### 6.4 与Prometheus对比测试
**文件**: `storage/benches/prometheus_comparison.rs`

- [ ] 相同数据集对比
- [ ] 相同查询对比
- [ ] 生成性能对比报告

---

## 实施顺序

### 第一阶段：核心模块单元测试（优先级：高）
1. ✅ 降采样系统单元测试（任务1）
2. ✅ 列式存储单元测试（任务3）

### 第二阶段：分布式系统单元测试（优先级：高）
3. ✅ 分布式系统单元测试（任务2）
4. ✅ 查询引擎单元测试（任务4）

### 第三阶段：集成测试（优先级：中）
5. ✅ 端到端集成测试（任务5）

### 第四阶段：性能基准测试（优先级：中）
6. ✅ 性能基准测试框架（任务6）

---

## 验收标准

### 单元测试
- [ ] 核心模块测试覆盖率达到80%以上
- [ ] 所有单元测试通过
- [ ] 测试包含边界条件和错误情况

### 集成测试
- [ ] 所有集成测试通过
- [ ] 端到端测试覆盖主要场景
- [ ] 故障场景下系统行为符合预期

### 性能测试
- [ ] 性能测试报告完整
- [ ] 性能瓶颈已识别
- [ ] 达到设计目标的80%以上

---

## 风险和缓解措施

### 风险1: 测试数据量大
- **缓解**: 使用临时目录，测试后自动清理
- **缓解**: 使用小规模测试数据进行单元测试

### 风险2: 分布式测试复杂
- **缓解**: 使用模拟的RPC客户端进行测试
- **缓解**: 先进行单节点测试，再进行多节点测试

### 风险3: 性能测试环境不一致
- **缓解**: 在相同的环境下运行所有测试
- **缓解**: 多次运行取平均值

---

## 时间估算

- 单元测试：3-5天
- 集成测试：2-3天
- 性能测试：2-3天
- 总计：7-11天

---

## 总结

本计划详细描述了Phase 1的测试和验证工作，包括单元测试完善、集成测试和性能基准测试。通过系统性的测试工作，可以确保ChronoDB的核心功能稳定可靠，为后续的性能优化和生产就绪打下坚实的基础。
