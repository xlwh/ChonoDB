# ChronoDB 集成测试框架

## 概述

本测试框架用于全面测试 ChronoDB 的功能、性能，并与 Prometheus 进行对比。支持一键部署测试环境、自动运行测试、自动清理环境。

## 功能特性

- ✅ 一键部署测试环境（Docker Compose）
- ✅ 自动部署 Prometheus 和 ChronoDB
- ✅ 测试所有查询算子（聚合、二元、比较、逻辑运算符等）
- ✅ 测试不同时间跨度查询（1分钟到6个月）
- ✅ ChronoDB vs Prometheus 功能对比
- ✅ ChronoDB vs Prometheus 性能对比
- ✅ ChronoDB vs Prometheus 存储成本对比
- ✅ 每个节点部署监控 Agent，实时采集资源使用数据
- ✅ 自动识别资源瓶颈，提供优化建议
- ✅ 测试完成后自动清理环境

## 快速开始

### 1. 部署测试环境

```bash
cd integration-tests
./scripts/deploy.sh
```

### 2. 运行测试

```bash
./scripts/run_tests.sh
```

### 3. 查看测试报告

测试完成后，报告生成在 `reports/test-report.html`

### 4. 清理环境

```bash
./scripts/cleanup.sh
```

## 目录结构

```
integration-tests/
├── docker/                      # Docker 配置文件
│   ├── docker-compose.test.yml  # 单节点测试环境
│   ├── docker-compose.distributed.yml  # 分布式测试环境
│   ├── prometheus/              # Prometheus 配置
│   ├── chronodb/                # ChronoDB 配置
│   ├── agent/                   # 监控 Agent 配置
│   └── grafana/                 # Grafana 配置
├── tests/                       # 测试用例
│   ├── test_basic_operations.py
│   ├── test_query_operators.py
│   ├── test_time_ranges.py
│   ├── test_distributed.py
│   ├── test_comparison.py
│   ├── test_performance.py
│   ├── test_storage_cost.py
│   └── test_resource_monitoring.py
├── utils/                       # 工具类
│   ├── docker_manager.py
│   ├── data_generator.py
│   ├── query_executor.py
│   ├── metrics_collector.py
│   ├── resource_analyzer.py
│   └── report_generator.py
├── scripts/                     # 脚本文件
│   ├── deploy.sh
│   ├── run_tests.sh
│   ├── cleanup.sh
│   └── generate_report.sh
├── reports/                     # 测试报告
├── requirements.txt             # Python 依赖
├── pytest.ini                   # pytest 配置
└── README.md                    # 本文件
```

## 测试环境

### 单节点测试环境

- ChronoDB: http://localhost:9090
- Prometheus: http://localhost:9092
- 监控 Prometheus: http://localhost:9093
- Grafana: http://localhost:3000 (admin/admin)

### 分布式测试环境

- ChronoDB Node 1: http://localhost:9091
- ChronoDB Node 2: http://localhost:9093
- ChronoDB Node 3: http://localhost:9095
- Prometheus: http://localhost:9090
- 监控 Prometheus: http://localhost:9097

## 监控说明

每个测试节点都部署了 prom-agent 监控采集器，自动采集以下指标：

- CPU 使用率
- 内存使用率
- 磁盘 I/O
- 网络流量
- 服务自身 metrics

监控数据发送到独立的 monitoring-prometheus，可通过 Grafana 查看实时监控面板。

## 测试报告

测试报告包含：

1. 执行摘要
2. 功能测试结果
3. 性能测试结果
4. ChronoDB vs Prometheus 对比
5. 资源使用分析
6. 优化建议

## 依赖要求

- Docker 20.10+
- Docker Compose 2.0+
- Python 3.8+
- pytest 7.0+

## 许可证

MIT License
