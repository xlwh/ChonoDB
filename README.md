# ChronoDB

## 项目介绍

ChronoDB 是一个高性能、低成本的时序数据库系统，设计目标是完全兼容 Prometheus 的协议和功能，同时提供更优的性能和存储效率。

### 核心优势

- **高性能查询**：比 Prometheus 查询性能提升 30%+，特别是在基本查询和标签过滤场景
- **低成本存储**：通过智能存储策略和压缩算法，降低存储成本
- **完全兼容 Prometheus**：支持 Prometheus 的所有核心功能和 API
- **灵活部署**：支持单机本地存储和多机分布式部署
- **存储多样性**：支持本地盘存储和分布式文件系统 (DFS)
- **智能降采样**：查询长时间段数据时自动使用降采样数据，提高查询效率

## 软件架构

ChronoDB 采用分层架构设计，主要包括以下组件：

1. **API 层**：兼容 Prometheus HTTP API v1，支持查询、写入和元数据操作
2. **查询引擎**：处理 PromQL 查询，支持聚合、过滤和时间函数
3. **存储引擎**：高效存储和检索时间序列数据
4. **索引系统**：快速定位和过滤时间序列
5. **降采样系统**：自动管理不同精度的数据

## 快速开始

### 安装

1. **克隆仓库**
   ```bash
   git clone https://gitee.com/hongbin1/chonodb.git
   cd chonodb
   ```

2. **编译项目**
   ```bash
   cargo build --release
   ```

3. **运行服务器**
   ```bash
   cargo run --bin chronodb-server
   ```

   默认情况下，服务器会在 `0.0.0.0:9090` 上启动。

### 基本使用

#### 1. 写入数据

使用 Prometheus 文本格式写入数据：

```bash
curl -X POST http://localhost:9090/api/v1/write \
  -H "Content-Type: text/plain" \
  -d 'cpu_usage_percent{job="frontend", instance="server1", region="us-east-1", environment="production"} 45.6 1620000000000'
```

#### 2. 查询数据

使用 PromQL 查询数据：

```bash
# 基本查询
curl "http://localhost:9090/api/v1/query?query=cpu_usage_percent"

# 聚合查询
curl "http://localhost:9090/api/v1/query?query=sum(cpu_usage_percent)"

# 标签过滤
curl "http://localhost:9090/api/v1/query?query=cpu_usage_percent{job="frontend"}"
```

#### 3. Web 管理界面

ChronoDB 提供了功能完整的 Web 管理界面，支持可视化操作：

```bash
# 启动服务器后，访问 Web 界面
open http://localhost:9090/ui
```

**Web 界面功能：**
- 📊 **数据查询**：PromQL 查询编辑器，支持即时查询和范围查询，结果可视化展示
- ✏️ **数据写入**：单条和批量数据写入，支持 JSON 格式
- 📈 **统计监控**：存储、查询性能、内存使用等实时统计
- 🌐 **集群管理**：节点状态监控、分片分布可视化（分布式模式）
- 🔔 **告警管理**：告警规则配置和当前告警查看
- ⚙️ **配置管理**：在线查看和修改系统配置

详细使用说明请参考 [Web 管理界面文档](docs/Usage.md#web-管理界面)。

## 功能特性

### 已实现功能

- ✅ Prometheus HTTP API v1 兼容
- ✅ 基本查询和标签过滤
- ✅ 聚合函数（sum, avg, min, max）
- ✅ 时间函数
- ✅ 文本格式数据写入
- ✅ 内存存储引擎
- ✅ 倒排索引系统
- ✅ **智能降采样系统**：
  - 5级降采样（L0:10s, L1:1min, L2:5min, L3:1h, L4:1d）
  - 自动降采样选择器
  - 降采样任务调度器
  - 降采样数据持久化存储
- ✅ **列式存储**：
  - 时间列、值列、标签列分离存储
  - 多种压缩算法（Delta、Delta-of-Delta、ZSTD、预测编码、字典编码）
  - 块存储格式
- ✅ **分布式架构**：
  - 集群管理器（节点发现、心跳机制）
  - 数据复制（同步/异步模式）
  - 分布式查询协调器
  - 一致性哈希分片
  - 查询缓存
- ✅ **查询优化器**：
  - 成本优化器
  - 向量化执行引擎
  - 查询计划器
- ✅ **数据分层**：
  - 热/温/冷/归档数据分层
  - 自动迁移策略
- ✅ **完整的工具链**：
  - 备份和恢复
  - 数据压缩
  - 数据迁移
  - 故障注入测试
- ✅ **WAL和持久化**：
  - Write-Ahead Log
  - 数据刷盘
  - 块压缩
- ✅ **Web 管理界面**：
  - 数据查询和可视化
  - 数据写入界面
  - 系统统计监控
  - 集群管理
  - 告警管理
  - 配置管理

## 性能测试

### 与 Prometheus 对比测试

| 系统 | 平均查询时间 (秒) | 相对性能 |
|------|-----------------|----------|
| ChronoDB | 0.0070 | 1.32x faster |
| Prometheus | 0.0093 | 基准 |

详细的测试报告请参考 [docs/Test-Report.md](docs/Test-Report.md)。

## 测试数据生成

ChronoDB 提供了多种测试数据生成脚本，用于生成不同规模的测试数据：

```bash
# 生成大规模测试数据（80,000 条数据）
python3 test_scripts/generate_large_test_data.py

# 生成基本测试数据
python3 test_scripts/generate_test_data.py
```

## 参与贡献

1. **Fork 本仓库**
2. **新建 Feat_xxx 分支**
3. **提交代码**
4. **新建 Pull Request**

## 文档

- [API 文档](docs/API.md)
- [设计文档](docs/ChronoDB-Design.md)
- [使用指南](docs/Usage.md)
- [测试报告](docs/Test-Report.md)
- [问题跟踪](docs/Issues.md)
- [路线图](docs/ChronoDB-Roadmap.md)

## 技术栈

- **开发语言**：Rust
- **存储引擎**：自研内存存储 + 持久化存储
- **索引系统**：倒排索引
- **API**：兼容 Prometheus HTTP API v1
- **查询语言**：PromQL

## 许可证

ChronoDB 使用 MIT 许可证。详见 [LICENSE](LICENSE) 文件。

---

*ChronoDB - 高性能时序数据库*
