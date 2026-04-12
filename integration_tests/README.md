# ChronoDB 集成测试框架

一个完整的集成测试框架，用于验证 ChronoDB 在分布式和单机模式下的功能、性能和可靠性。

## 功能特性

- ✅ **多模式测试**: 支持单机模式和分布式模式测试
- ✅ **多规模测试**: 支持 Small/Medium/Large 三种数据规模
- ✅ **PromQL 全覆盖**: 测试各种 PromQL 算子（聚合、范围函数、数学函数等）
- ✅ **对比测试**: 与 Prometheus 进行结果对比
- ✅ **故障注入**: 支持 FO（强制停止）、随机重启等故障场景
- ✅ **一键运行**: 简单的命令行接口，一键运行所有测试
- ✅ **详细报告**: 生成 JSON/HTML/Markdown 格式的测试报告

## 快速开始

### 环境要求

- Docker 20.10+ (Docker 模式)
- Python 3.8+
- 本地镜像: `prom/prometheus:latest` (Docker 模式)

### 安装依赖

```bash
cd integration_tests
pip install requests pyyaml
```

### 运行测试

#### 方式一: Docker 模式 (自动启动容器)

```bash
# 运行单机模式小规模测试
python run_tests.py --mode standalone --scale small

# 运行对比测试
python run_tests.py --compare --scale medium

# 运行带故障注入的测试
python run_tests.py --mode distributed --scale medium --enable-fault-injection

# 运行完整测试并保留容器
python run_tests.py --compare --scale large --keep-containers
```

#### 方式二: 本地服务模式 (使用已运行的服务)

```bash
# 启动本地服务
./start_local_services.sh --all

# 在另一个终端运行测试
python run_local_test.py --scale small

# 运行对比测试 (需要 Prometheus 在 9090, ChronoDB 在 9091)
python run_local_test.py --compare --scale medium

# 指定自定义地址
python run_local_test.py --prometheus-url http://localhost:9090 --chronodb-url http://localhost:9091 --compare
```

## 命令行参数

| 参数 | 说明 | 默认值 |
|------|------|--------|
| `--mode` | 测试模式: standalone/distributed | standalone |
| `--scale` | 数据规模: small/medium/large | small |
| `--compare` | 启用 Prometheus vs ChronoDB 对比测试 | False |
| `--test-promql` | 运行 PromQL 测试 | True |
| `--enable-fault-injection` | 启用故障注入测试 | False |
| `--fault-duration` | 故障注入测试持续时间(秒) | 300 |
| `--generate-report` | 生成测试报告 | True |
| `--keep-containers` | 测试完成后保留容器 | False |
| `--log-level` | 日志级别: DEBUG/INFO/WARNING/ERROR | INFO |

## 数据规模定义

| 规模 | 指标数 | 序列数/指标 | 样本数/序列 | 时间范围 |
|------|--------|-------------|-------------|----------|
| Small | 10 | 10 | 100 | 1小时 |
| Medium | 50 | 50 | 1,000 | 6小时 |
| Large | 100 | 100 | 10,000 | 24小时 |

## 测试覆盖

### 写入测试
- 单条数据写入
- 批量数据写入
- Gauge/Counter/Histogram 类型写入

### 查询测试
- 即时查询
- 范围查询
- 标签查询
- 序列查询

### PromQL 算子测试
- 聚合算子: sum, avg, min, max, count, stddev, quantile
- 范围函数: rate, irate, increase, delta, changes
- 数学函数: abs, ceil, floor, round, sqrt
- 二元运算符: +, -, *, /, %
- 集合运算符: and, or, unless

### 故障注入测试
- 容器强制停止 (kill)
- 容器重启 (restart)
- 容器暂停 (pause)
- 随机故障注入

## 项目结构

```
integration_tests/
├── core/                       # 核心模块
│   ├── config.py              # 配置管理
│   ├── logger.py              # 日志管理
│   └── base_test.py           # 测试基类
├── containers/                 # 容器管理
│   └── docker_manager.py      # Docker 容器管理
├── data_generators/            # 数据生成
│   └── metric_generator.py    # 指标数据生成器
├── query_tests/                # 查询测试
│   └── promql_tester.py       # PromQL 测试套件
├── fault_injection/            # 故障注入
│   └── fault_injector.py      # 故障注入器
├── comparators/                # 结果对比
│   └── result_comparator.py   # 结果对比器
├── reports/                    # 报告生成
│   └── report_generator.py    # 报告生成器
├── specs/                      # 测试规范
│   ├── spec.md                # 测试规范文档
│   ├── tasks.md               # 任务清单
│   └── checklist.md           # 验收清单
├── run_tests.py               # 主运行脚本
└── README.md                  # 本文档
```

## 测试报告

测试完成后会在 `integration_test_reports/` 目录下生成以下报告：

- `integration_test_report_*.json` - JSON 格式详细数据
- `integration_test_report_*.html` - HTML 格式可视化报告
- `integration_test_report_*.md` - Markdown 格式报告

## 配置

可以通过创建 `integration_test_config.yaml` 文件来自定义配置：

```yaml
container:
  prometheus_image: "prom/prometheus:latest"
  chronodb_image: "chronodb:latest"
  network_name: "chronodb-integration-test"

test:
  test_standalone: true
  test_distributed: true
  test_scales: ["small", "medium"]
  enable_fault_injection: true
  value_tolerance: 0.01
  timestamp_tolerance_ms: 1000

promql_operators:
  aggregations: ["sum", "avg", "min", "max", "count"]
  range_functions: ["rate", "irate", "increase", "delta"]
```

## 使用示例

### 示例 1: 基本测试

```bash
# 运行单机模式小规模测试
python run_tests.py --mode standalone --scale small
```

### 示例 2: 对比测试

```bash
# 运行 Prometheus vs ChronoDB 对比测试
python run_tests.py --compare --scale medium
```

### 示例 3: 故障注入测试

```bash
# 运行带故障注入的测试，持续 5 分钟
python run_tests.py --mode distributed --scale medium \
  --enable-fault-injection --fault-duration 300
```

### 示例 4: 完整测试

```bash
# 运行所有测试并保留容器用于调试
python run_tests.py --compare --scale large \
  --enable-fault-injection --keep-containers
```

## 注意事项

1. 测试前请确保 Docker 已安装并运行
2. 确保端口 9090-9092 未被占用
3. 测试过程中会创建 Docker 网络和容器
4. 默认情况下测试完成后会自动清理容器
5. 使用 `--keep-containers` 可以保留容器用于调试

## 故障排查

### 容器启动失败

检查 Docker 是否运行：
```bash
docker version
```

### 端口被占用

检查端口占用情况：
```bash
lsof -i :9090
lsof -i :9091
```

### 镜像不存在

ChronoDB 镜像会自动从 Dockerfile 构建，确保项目根目录有 Dockerfile。

## 贡献

欢迎提交 Issue 和 PR 来改进测试框架。

## 许可证

MIT License
