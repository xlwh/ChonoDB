# ChronoDB 集成测试方案

## 1. 方案概述

本方案旨在构建一个完整的集成测试框架，用于全面测试 ChronoDB 的功能、性能，并与 Prometheus 进行对比。测试框架支持一键部署测试环境、自动运行测试、自动清理环境。

## 2. 测试框架架构

### 2.1 目录结构

```
integration-tests/
├── docker/
│   ├── docker-compose.test.yml          # 测试环境编排文件
│   ├── docker-compose.distributed.yml   # 分布式测试环境编排
│   ├── prometheus/
│   │   ├── prometheus.yml               # Prometheus配置
│   │   ├── prometheus-distributed.yml   # 分布式Prometheus配置
│   │   ├── monitoring-prometheus.yml    # 监控Prometheus配置
│   │   └── rules/                       # Prometheus规则文件
│   ├── chronodb/
│   │   ├── chronodb.yaml               # ChronoDB配置
│   │   ├── chronodb-distributed.yaml   # 分布式ChronoDB配置
│   │   └── rules/                      # ChronoDB规则文件
│   ├── agent/
│   │   ├── chronodb-agent.yaml         # ChronoDB节点Agent配置
│   │   ├── prometheus-agent.yaml       # Prometheus节点Agent配置
│   │   ├── chronodb-node-1-agent.yaml  # 分布式节点1 Agent配置
│   │   ├── chronodb-node-2-agent.yaml  # 分布式节点2 Agent配置
│   │   └── chronodb-node-3-agent.yaml  # 分布式节点3 Agent配置
│   └── grafana/
│       ├── dashboards/                 # 测试监控面板
│       └── provisioning/               # Grafana配置
│           ├── datasources/            # 数据源配置
│           └── dashboards/             # 面板配置
├── tests/
│   ├── conftest.py                     # pytest配置和fixtures
│   ├── test_basic_operations.py       # 基础操作测试
│   ├── test_query_operators.py        # 查询算子测试
│   ├── test_time_ranges.py            # 时间跨度测试
│   ├── test_distributed.py            # 分布式功能测试
│   ├── test_comparison.py             # ChronoDB vs Prometheus对比
│   ├── test_performance.py            # 性能测试
│   ├── test_storage_cost.py           # 存储成本测试
│   └── test_resource_monitoring.py    # 资源监控测试
├── utils/
│   ├── docker_manager.py              # Docker环境管理
│   ├── data_generator.py              # 测试数据生成器
│   ├── query_executor.py              # 查询执行器
│   ├── metrics_collector.py           # 指标收集器
│   ├── resource_analyzer.py           # 资源使用分析器
│   └── report_generator.py            # 测试报告生成器
├── scripts/
│   ├── deploy.sh                      # 一键部署脚本
│   ├── run_tests.sh                   # 运行测试脚本
│   ├── cleanup.sh                     # 清理环境脚本
│   └── generate_report.sh             # 生成测试报告
├── requirements.txt                   # Python依赖
├── pytest.ini                        # pytest配置
└── README.md                         # 测试框架使用说明
```

### 2.2 技术栈

- **测试框架**: pytest + pytest-asyncio
- **HTTP客户端**: requests + aiohttp
- **数据处理**: pandas + numpy
- **可视化**: matplotlib + plotly
- **Docker管理**: docker-compose + docker-py
- **监控**: Prometheus + Grafana
- **报告生成**: Jinja2 + HTML

## 3. Docker Compose 配置

### 3.1 测试环境配置 (docker-compose.test.yml)

```yaml
version: '3.8'

services:
  # ChronoDB 单节点
  chronodb:
    build:
      context: ../
      dockerfile: Dockerfile
    container_name: chronodb-test
    ports:
      - "9090:9090"
      - "9091:9091"
    volumes:
      - chronodb-data:/var/lib/chronodb
      - chronodb-logs:/var/log/chronodb
      - ./chronodb/chronodb.yaml:/etc/chronodb/chronodb.yaml:ro
    environment:
      - RUST_LOG=debug
      - CHRONODB_DATA_DIR=/var/lib/chronodb
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:9090/-/healthy"]
      interval: 5s
      timeout: 3s
      retries: 10
    networks:
      - test-network

  # ChronoDB 监控 Agent
  chronodb-agent:
    build:
      context: /Users/zhb/workspace/prom-agent
      dockerfile: Dockerfile
    container_name: chronodb-agent
    depends_on:
      - chronodb
    volumes:
      - /proc:/host/proc:ro
      - /sys:/host/sys:ro
      - ./agent/chronodb-agent.yaml:/etc/prom-agent/agent_config.yaml:ro
    environment:
      - NODE_NAME=chronodb
      - SERVICE_PORT=9090
    networks:
      - test-network

  # Prometheus (用于对比测试)
  prometheus:
    image: prom/prometheus:v2.45.0
    container_name: prometheus-test
    ports:
      - "9092:9090"
    volumes:
      - ./prometheus/prometheus.yml:/etc/prometheus/prometheus.yml:ro
      - prometheus-data:/prometheus
    command:
      - '--config.file=/etc/prometheus/prometheus.yml'
      - '--storage.tsdb.path=/prometheus'
      - '--storage.tsdb.retention.time=7d'
      - '--web.enable-lifecycle'
    healthcheck:
      test: ["CMD", "wget", "-q", "--spider", "http://localhost:9090/-/healthy"]
      interval: 5s
      timeout: 3s
      retries: 10
    networks:
      - test-network

  # Prometheus 监控 Agent
  prometheus-agent:
    build:
      context: /Users/zhb/workspace/prom-agent
      dockerfile: Dockerfile
    container_name: prometheus-agent
    depends_on:
      - prometheus
    volumes:
      - /proc:/host/proc:ro
      - /sys:/host/sys:ro
      - ./agent/prometheus-agent.yaml:/etc/prom-agent/agent_config.yaml:ro
    environment:
      - NODE_NAME=prometheus
      - SERVICE_PORT=9090
    networks:
      - test-network

  # 监控 Prometheus (接收所有 Agent 数据)
  monitoring-prometheus:
    image: prom/prometheus:v2.45.0
    container_name: monitoring-prometheus
    ports:
      - "9093:9090"
    volumes:
      - ./prometheus/monitoring-prometheus.yml:/etc/prometheus/prometheus.yml:ro
      - monitoring-prometheus-data:/prometheus
    command:
      - '--config.file=/etc/prometheus/prometheus.yml'
      - '--storage.tsdb.path=/prometheus'
      - '--storage.tsdb.retention.time=7d'
      - '--web.enable-remote-write-receiver'
      - '--web.enable-lifecycle'
    healthcheck:
      test: ["CMD", "wget", "-q", "--spider", "http://localhost:9090/-/healthy"]
      interval: 5s
      timeout: 3s
      retries: 10
    networks:
      - test-network

  # Grafana (测试监控)
  grafana:
    image: grafana/grafana:10.0.0
    container_name: grafana-test
    ports:
      - "3000:3000"
    environment:
      - GF_SECURITY_ADMIN_PASSWORD=admin
      - GF_INSTALL_PLUGINS=grafana-clock-panel
    volumes:
      - grafana-data:/var/lib/grafana
      - ./grafana/dashboards:/var/lib/grafana/dashboards
      - ./grafana/provisioning:/etc/grafana/provisioning
    networks:
      - test-network

volumes:
  chronodb-data:
  chronodb-logs:
  prometheus-data:
  monitoring-prometheus-data:
  grafana-data:

networks:
  test-network:
    driver: bridge
```

### 3.2 分布式测试环境 (docker-compose.distributed.yml)

```yaml
version: '3.8'

services:
  # Etcd 集群
  etcd-1:
    image: bitnami/etcd:3.5
    container_name: etcd-1
    environment:
      - ETCD_NAME=etcd-1
      - ETCD_INITIAL_CLUSTER=etcd-1=http://etcd-1:2380,etcd-2=http://etcd-2:2380,etcd-3=http://etcd-3:2380
      - ETCD_INITIAL_CLUSTER_STATE=new
      - ETCD_INITIAL_CLUSTER_TOKEN=etcd-cluster
      - ALLOW_NONE_AUTHENTICATION=yes
    ports:
      - "2379:2379"
    networks:
      - test-network

  etcd-2:
    image: bitnami/etcd:3.5
    container_name: etcd-2
    environment:
      - ETCD_NAME=etcd-2
      - ETCD_INITIAL_CLUSTER=etcd-1=http://etcd-1:2380,etcd-2=http://etcd-2:2380,etcd-3=http://etcd-3:2380
      - ETCD_INITIAL_CLUSTER_STATE=new
      - ETCD_INITIAL_CLUSTER_TOKEN=etcd-cluster
      - ALLOW_NONE_AUTHENTICATION=yes
    networks:
      - test-network

  etcd-3:
    image: bitnami/etcd:3.5
    container_name: etcd-3
    environment:
      - ETCD_NAME=etcd-3
      - ETCD_INITIAL_CLUSTER=etcd-1=http://etcd-1:2380,etcd-2=http://etcd-2:2380,etcd-3=http://etcd-3:2380
      - ETCD_INITIAL_CLUSTER_STATE=new
      - ETCD_INITIAL_CLUSTER_TOKEN=etcd-cluster
      - ALLOW_NONE_AUTHENTICATION=yes
    networks:
      - test-network

  # ChronoDB 节点 1
  chronodb-node-1:
    build:
      context: ../
      dockerfile: Dockerfile
    container_name: chronodb-node-1
    ports:
      - "9091:9090"
      - "9092:9091"
    environment:
      - RUST_LOG=debug
      - CHRONODB_NODE_ID=1
      - CHRONODB_ETCD_ENDPOINTS=etcd-1:2379,etcd-2:2379,etcd-3:2379
    volumes:
      - chronodb-data-1:/var/lib/chronodb
      - ./chronodb/chronodb-distributed.yaml:/etc/chronodb/chronodb.yaml:ro
    depends_on:
      - etcd-1
      - etcd-2
      - etcd-3
    networks:
      - test-network

  # ChronoDB 节点 1 监控 Agent
  chronodb-node-1-agent:
    build:
      context: /Users/zhb/workspace/prom-agent
      dockerfile: Dockerfile
    container_name: chronodb-node-1-agent
    depends_on:
      - chronodb-node-1
    volumes:
      - /proc:/host/proc:ro
      - /sys:/host/sys:ro
      - ./agent/chronodb-node-1-agent.yaml:/etc/prom-agent/agent_config.yaml:ro
    environment:
      - NODE_NAME=chronodb-node-1
      - SERVICE_PORT=9090
    networks:
      - test-network

  # ChronoDB 节点 2
  chronodb-node-2:
    build:
      context: ../
      dockerfile: Dockerfile
    container_name: chronodb-node-2
    ports:
      - "9093:9090"
      - "9094:9091"
    environment:
      - RUST_LOG=debug
      - CHRONODB_NODE_ID=2
      - CHRONODB_ETCD_ENDPOINTS=etcd-1:2379,etcd-2:2379,etcd-3:2379
    volumes:
      - chronodb-data-2:/var/lib/chronodb
      - ./chronodb/chronodb-distributed.yaml:/etc/chronodb/chronodb.yaml:ro
    depends_on:
      - etcd-1
      - etcd-2
      - etcd-3
    networks:
      - test-network

  # ChronoDB 节点 2 监控 Agent
  chronodb-node-2-agent:
    build:
      context: /Users/zhb/workspace/prom-agent
      dockerfile: Dockerfile
    container_name: chronodb-node-2-agent
    depends_on:
      - chronodb-node-2
    volumes:
      - /proc:/host/proc:ro
      - /sys:/host/sys:ro
      - ./agent/chronodb-node-2-agent.yaml:/etc/prom-agent/agent_config.yaml:ro
    environment:
      - NODE_NAME=chronodb-node-2
      - SERVICE_PORT=9090
    networks:
      - test-network

  # ChronoDB 节点 3
  chronodb-node-3:
    build:
      context: ../
      dockerfile: Dockerfile
    container_name: chronodb-node-3
    ports:
      - "9095:9090"
      - "9096:9091"
    environment:
      - RUST_LOG=debug
      - CHRONODB_NODE_ID=3
      - CHRONODB_ETCD_ENDPOINTS=etcd-1:2379,etcd-2:2379,etcd-3:2379
    volumes:
      - chronodb-data-3:/var/lib/chronodb
      - ./chronodb/chronodb-distributed.yaml:/etc/chronodb/chronodb.yaml:ro
    depends_on:
      - etcd-1
      - etcd-2
      - etcd-3
    networks:
      - test-network

  # ChronoDB 节点 3 监控 Agent
  chronodb-node-3-agent:
    build:
      context: /Users/zhb/workspace/prom-agent
      dockerfile: Dockerfile
    container_name: chronodb-node-3-agent
    depends_on:
      - chronodb-node-3
    volumes:
      - /proc:/host/proc:ro
      - /sys:/host/sys:ro
      - ./agent/chronodb-node-3-agent.yaml:/etc/prom-agent/agent_config.yaml:ro
    environment:
      - NODE_NAME=chronodb-node-3
      - SERVICE_PORT=9090
    networks:
      - test-network

  # Prometheus
  prometheus:
    image: prom/prometheus:v2.45.0
    container_name: prometheus-dist-test
    ports:
      - "9090:9090"
    volumes:
      - ./prometheus/prometheus-distributed.yml:/etc/prometheus/prometheus.yml:ro
      - prometheus-data:/prometheus
    networks:
      - test-network

  # Prometheus 监控 Agent
  prometheus-agent:
    build:
      context: /Users/zhb/workspace/prom-agent
      dockerfile: Dockerfile
    container_name: prometheus-dist-agent
    depends_on:
      - prometheus
    volumes:
      - /proc:/host/proc:ro
      - /sys:/host/sys:ro
      - ./agent/prometheus-agent.yaml:/etc/prom-agent/agent_config.yaml:ro
    environment:
      - NODE_NAME=prometheus
      - SERVICE_PORT=9090
    networks:
      - test-network

  # 监控 Prometheus (接收所有 Agent 数据)
  monitoring-prometheus:
    image: prom/prometheus:v2.45.0
    container_name: monitoring-prometheus-dist
    ports:
      - "9097:9090"
    volumes:
      - ./prometheus/monitoring-prometheus.yml:/etc/prometheus/prometheus.yml:ro
      - monitoring-prometheus-data:/prometheus
    command:
      - '--config.file=/etc/prometheus/prometheus.yml'
      - '--storage.tsdb.path=/prometheus'
      - '--storage.tsdb.retention.time=7d'
      - '--web.enable-remote-write-receiver'
      - '--web.enable-lifecycle'
    networks:
      - test-network

volumes:
  chronodb-data-1:
  chronodb-data-2:
  chronodb-data-3:
  prometheus-data:
  monitoring-prometheus-data:

networks:
  test-network:
    driver: bridge
```

### 3.3 Agent 配置文件示例

#### 3.3.1 ChronoDB Agent 配置 (agent/chronodb-agent.yaml)

```yaml
agent:
  log_level: info
  listen_address: 0.0.0.0:9090
  metrics_path: /metrics

system_collector:
  enabled: true
  collectors:
    cpu: true
    memory: true
    disk: true
    filesystem: true
    network: true
    load: true
  container_mode: auto

service_scrapers:
  - id: chronodb
    url: http://chronodb:9090/metrics
    interval_secs: 15
    timeout_secs: 5
    labels:
      service: chronodb
      node: chronodb
      environment: test

remote_write:
  endpoints:
    - name: monitoring
      endpoint: http://monitoring-prometheus:9090/api/v1/write
      priority: 1
      enabled: true
  queue_config:
    capacity: 10000
    max_shards: 5
    max_samples_per_send: 1000
    batch_send_deadline_secs: 5
    max_retries: 3
```

#### 3.3.2 Prometheus Agent 配置 (agent/prometheus-agent.yaml)

```yaml
agent:
  log_level: info
  listen_address: 0.0.0.0:9090
  metrics_path: /metrics

system_collector:
  enabled: true
  collectors:
    cpu: true
    memory: true
    disk: true
    filesystem: true
    network: true
    load: true
  container_mode: auto

service_scrapers:
  - id: prometheus
    url: http://prometheus:9090/metrics
    interval_secs: 15
    timeout_secs: 5
    labels:
      service: prometheus
      node: prometheus
      environment: test

remote_write:
  endpoints:
    - name: monitoring
      endpoint: http://monitoring-prometheus:9090/api/v1/write
      priority: 1
      enabled: true
  queue_config:
    capacity: 10000
    max_shards: 5
    max_samples_per_send: 1000
    batch_send_deadline_secs: 5
    max_retries: 3
```

#### 3.3.3 分布式节点 Agent 配置 (agent/chronodb-node-1-agent.yaml)

```yaml
agent:
  log_level: info
  listen_address: 0.0.0.0:9090
  metrics_path: /metrics

system_collector:
  enabled: true
  collectors:
    cpu: true
    memory: true
    disk: true
    filesystem: true
    network: true
    load: true
  container_mode: auto

service_scrapers:
  - id: chronodb-node-1
    url: http://chronodb-node-1:9090/metrics
    interval_secs: 15
    timeout_secs: 5
    labels:
      service: chronodb
      node: chronodb-node-1
      node_id: "1"
      environment: distributed-test

remote_write:
  endpoints:
    - name: monitoring
      endpoint: http://monitoring-prometheus:9090/api/v1/write
      priority: 1
      enabled: true
  queue_config:
    capacity: 10000
    max_shards: 5
    max_samples_per_send: 1000
    batch_send_deadline_secs: 5
    max_retries: 3
```

## 4. 测试用例设计

### 4.1 基础操作测试 (test_basic_operations.py)

**测试目标**: 验证基本的数据写入和查询功能

**测试用例**:
1. 单条数据写入和查询
2. 批量数据写入和查询
3. 数据写入性能测试（吞吐量、延迟）
4. 数据持久化验证（重启后数据完整性）
5. 并发写入测试
6. 错误数据处理测试

### 4.2 查询算子测试 (test_query_operators.py)

**测试目标**: 验证所有 PromQL 查询算子的正确性

**测试用例**:

#### 4.2.1 聚合算子
- `sum()`: 求和聚合
- `avg()`: 平均值聚合
- `min()`: 最小值聚合
- `max()`: 最大值聚合
- `count()`: 计数聚合
- `count_values()`: 值计数
- `group()`: 分组
- `stddev()`: 标准差
- `stdvar()`: 方差
- `topk()`: Top K
- `bottomk()`: Bottom K
- `quantile()`: 分位数

#### 4.2.2 二元运算符
- 加法 (`+`)
- 减法 (`-`)
- 乘法 (`*`)
- 除法 (`/`)
- 取模 (`%`)
- 幂运算 (`^`)

#### 4.2.3 比较运算符
- 等于 (`==`)
- 不等于 (`!=`)
- 大于 (`>`)
- 小于 (`<`)
- 大于等于 (`>=`)
- 小于等于 (`<=`)

#### 4.2.4 逻辑运算符
- `and`
- `or`
- `unless`

#### 4.2.5 向量匹配
- `on`: 指定匹配标签
- `ignoring`: 忽略匹配标签
- `group_left`: 多对一匹配
- `group_right`: 一对多匹配

#### 4.2.6 函数测试
- 时间函数: `time()`, `minute()`, `hour()`, `month()`, `year()`
- 数学函数: `abs()`, `ceil()`, `floor()`, `round()`, `sqrt()`, `ln()`, `log2()`, `log10()`
- 变化率函数: `rate()`, `irate()`, `increase()`, `delta()`
- 时间窗口函数: `avg_over_time()`, `min_over_time()`, `max_over_time()`, `sum_over_time()`, `count_over_time()`
- 标签操作函数: `label_replace()`, `label_join()`
- 其他函数: `vector()`, `scalar()`, `clamp()`, `clamp_max()`, `clamp_min()`

### 4.3 时间跨度测试 (test_time_ranges.py)

**测试目标**: 验证不同时间跨度查询的正确性和性能

**测试用例**:

#### 4.3.1 短时间跨度
- 1分钟查询
- 5分钟查询
- 15分钟查询
- 1小时查询

#### 4.3.2 中等时间跨度
- 6小时查询
- 12小时查询
- 1天查询
- 3天查询

#### 4.3.3 长时间跨度
- 1周查询
- 1个月查询
- 3个月查询
- 6个月查询

#### 4.3.4 降采样验证
- 自动降采样触发验证
- 不同降采样策略查询
- 降采样数据精度验证
- 降采样性能提升验证

### 4.4 分布式功能测试 (test_distributed.py)

**测试目标**: 验证分布式架构的功能和一致性

**测试用例**:
1. 多节点数据写入
2. 跨节点查询
3. 数据分片验证
4. 节点故障恢复测试
5. 数据一致性验证
6. 负载均衡测试
7. 联邦查询测试

### 4.5 ChronoDB vs Prometheus 对比测试 (test_comparison.py)

**测试目标**: 全面对比 ChronoDB 和 Prometheus 的功能、性能、存储成本

#### 4.5.1 功能对比

**测试方法**:
1. 向两个系统写入相同的测试数据
2. 执行相同的查询语句
3. 对比查询结果的正确性

**对比项**:
- 基本查询功能
- 聚合查询功能
- 时间窗口函数
- 复杂 PromQL 表达式
- 标签过滤功能
- 元数据查询功能

#### 4.5.2 性能对比

**测试方法**:
1. 使用相同的数据集
2. 执行相同的查询负载
3. 记录响应时间、吞吐量、资源使用

**对比指标**:
- 写入吞吐量 (points/second)
- 写入延迟 (P50, P95, P99)
- 查询响应时间 (P50, P95, P99)
- 查询吞吐量 (queries/second)
- 并发查询能力
- 内存使用
- CPU 使用率
- 磁盘 I/O

**测试场景**:
- 小规模数据 (100万数据点)
- 中等规模数据 (1000万数据点)
- 大规模数据 (1亿数据点)
- 超大规模数据 (10亿数据点)

#### 4.5.3 存储成本对比

**测试方法**:
1. 写入相同的数据量
2. 测量磁盘占用
3. 测量压缩率
4. 测量内存占用

**对比指标**:
- 原始数据大小
- 存储后数据大小
- 压缩率
- 索引大小
- 元数据大小
- 总存储成本
- 内存占用

### 4.6 性能测试 (test_performance.py)

**测试目标**: 全面评估 ChronoDB 的性能特性

**测试用例**:

#### 4.6.1 写入性能
- 单线程写入吞吐量
- 多线程写入吞吐量
- 批量写入性能
- Remote Write 性能
- 写入延迟分布

#### 4.6.2 查询性能
- 简单查询性能
- 复杂查询性能
- 聚合查询性能
- 时间范围查询性能
- 并发查询性能
- 查询缓存效果

#### 4.6.3 压力测试
- 高并发写入测试
- 高并发查询测试
- 混合负载测试
- 长时间稳定性测试

### 4.7 存储成本测试 (test_storage_cost.py)

**测试目标**: 评估 ChronoDB 的存储效率

**测试用例**:
1. 不同压缩算法效果对比
2. 不同数据类型的压缩率
3. 索引开销测量
4. 元数据开销测量
5. 降采样存储节省
6. 分层存储效果

### 4.8 资源使用监控测试 (test_resource_monitoring.py)

**测试目标**: 监控和分析测试过程中各节点的资源使用情况

**测试用例**:

#### 4.8.1 CPU 使用分析
- 各节点 CPU 使用率趋势
- CPU 使用峰值分析
- CPU 使用与负载的关系
- CPU 核心利用率分布

#### 4.8.2 内存使用分析
- 各节点内存使用趋势
- 内存使用峰值分析
- 内存泄漏检测
- 内存使用与数据量的关系

#### 4.8.3 磁盘 I/O 分析
- 磁盘读写速率
- IOPS 分析
- 磁盘使用量趋势
- 磁盘 I/O 与查询性能的关系

#### 4.8.4 网络流量分析
- 网络吞吐量
- 网络延迟
- 网络流量与数据写入的关系
- 分布式节点间网络流量

#### 4.8.5 资源使用对比
- ChronoDB vs Prometheus CPU 使用对比
- ChronoDB vs Prometheus 内存使用对比
- ChronoDB vs Prometheus 磁盘 I/O 对比
- ChronoDB vs Prometheus 网络流量对比

#### 4.8.6 性能瓶颈分析
- 识别资源瓶颈
- 分析性能与资源使用的关系
- 提供优化建议

## 5. 测试数据生成

### 5.1 数据生成器 (utils/data_generator.py)

```python
class DataGenerator:
    """测试数据生成器"""
    
    def generate_time_series(
        self,
        metric_name: str,
        labels: Dict[str, str],
        start_time: int,
        end_time: int,
        interval: int = 1000,
        value_pattern: str = "random"
    ) -> List[str]:
        """生成时间序列数据"""
        pass
    
    def generate_batch_data(
        self,
        num_metrics: int,
        num_series: int,
        num_points: int,
        time_range: Tuple[int, int]
    ) -> List[str]:
        """批量生成测试数据"""
        pass
    
    def generate_realistic_data(
        self,
        scenario: str = "web_server"
    ) -> List[str]:
        """生成真实场景数据"""
        pass
```

### 5.2 测试数据集

**小规模数据集**:
- 10个指标
- 100个时间序列
- 每个序列1000个数据点
- 总计: 100万数据点

**中等规模数据集**:
- 50个指标
- 1000个时间序列
- 每个序列10000个数据点
- 总计: 1000万数据点

**大规模数据集**:
- 100个指标
- 10000个时间序列
- 每个序列10000个数据点
- 总计: 1亿数据点

**超大规模数据集**:
- 500个指标
- 100000个时间序列
- 每个序列1000个数据点
- 总计: 10亿数据点

## 6. 自动化脚本

### 6.1 一键部署脚本 (scripts/deploy.sh)

```bash
#!/bin/bash
set -e

echo "=== ChronoDB 集成测试环境部署 ==="

# 检查依赖
command -v docker >/dev/null 2>&1 || { echo "需要安装 Docker"; exit 1; }
command -v docker-compose >/dev/null 2>&1 || { echo "需要安装 Docker Compose"; exit 1; }

# 设置环境变量
export TEST_ENV=true
export COMPOSE_PROJECT_NAME=chronodb-test

# 构建 ChronoDB 镜像
echo "构建 ChronoDB 镜像..."
docker-compose -f docker/docker-compose.test.yml build chronodb

# 构建 prom-agent 镜像
echo "构建 prom-agent 镜像..."
docker-compose -f docker/docker-compose.test.yml build chronodb-agent prometheus-agent

# 启动服务
echo "启动测试环境..."
docker-compose -f docker/docker-compose.test.yml up -d

# 等待服务就绪
echo "等待服务启动..."
sleep 10

# 健康检查
echo "检查服务健康状态..."
max_retries=30
retry=0

while [ $retry -lt $max_retries ]; do
    if curl -f http://localhost:9090/-/healthy >/dev/null 2>&1; then
        echo "✓ ChronoDB 服务就绪"
        break
    fi
    retry=$((retry + 1))
    sleep 2
done

if [ $retry -eq $max_retries ]; then
    echo "✗ ChronoDB 服务启动失败"
    exit 1
fi

retry=0
while [ $retry -lt $max_retries ]; do
    if curl -f http://localhost:9092/-/healthy >/dev/null 2>&1; then
        echo "✓ Prometheus 服务就绪"
        break
    fi
    retry=$((retry + 1))
    sleep 2
done

if [ $retry -eq $max_retries ]; then
    echo "✗ Prometheus 服务启动失败"
    exit 1
fi

retry=0
while [ $retry -lt $max_retries ]; do
    if curl -f http://localhost:9093/-/healthy >/dev/null 2>&1; then
        echo "✓ 监控 Prometheus 服务就绪"
        break
    fi
    retry=$((retry + 1))
    sleep 2
done

if [ $retry -eq $max_retries ]; then
    echo "✗ 监控 Prometheus 服务启动失败"
    exit 1
fi

# 等待 Agent 启动并开始采集
echo "等待监控 Agent 启动..."
sleep 5

# 检查 Agent 是否正常工作
echo "检查监控 Agent 状态..."
if curl -f http://localhost:9093/api/v1/query?query=up >/dev/null 2>&1; then
    echo "✓ 监控 Agent 工作正常"
else
    echo "⚠ 监控 Agent 可能未正常工作"
fi

echo "=== 环境部署完成 ==="
echo "ChronoDB: http://localhost:9090"
echo "Prometheus: http://localhost:9092"
echo "监控 Prometheus: http://localhost:9093"
echo "Grafana: http://localhost:3000 (admin/admin)"
echo ""
echo "监控数据将自动采集到监控 Prometheus，可通过 Grafana 查看"
```

### 6.2 运行测试脚本 (scripts/run_tests.sh)

```bash
#!/bin/bash
set -e

echo "=== 运行集成测试 ==="

# 安装 Python 依赖
echo "安装 Python 依赖..."
pip install -r requirements.txt

# 运行测试
echo "运行测试..."
pytest tests/ \
    --verbose \
    --tb=short \
    --html=reports/test-report.html \
    --self-contained-html \
    --junitxml=reports/junit.xml \
    --metrics=reports/metrics.json

# 生成报告
echo "生成测试报告..."
python scripts/generate_report.py

echo "=== 测试完成 ==="
echo "测试报告: reports/test-report.html"
```

### 6.3 清理环境脚本 (scripts/cleanup.sh)

```bash
#!/bin/bash

echo "=== 清理测试环境 ==="

# 停止服务
echo "停止服务..."
docker-compose -f docker/docker-compose.test.yml down

# 删除数据卷
echo "删除数据卷..."
docker-compose -f docker/docker-compose.test.yml down -v

# 删除镜像
echo "删除测试镜像..."
docker rmi chronodb-test_chronodb 2>/dev/null || true

# 清理临时文件
echo "清理临时文件..."
rm -rf reports/*.json
rm -rf reports/*.html

echo "=== 清理完成 ==="
```

## 7. 测试报告

### 7.1 报告内容

测试报告包含以下内容:

1. **执行摘要**
   - 测试环境信息
   - 测试执行时间
   - 总体通过率
   - 关键发现

2. **功能测试结果**
   - 各测试用例执行结果
   - 失败用例详情
   - 功能覆盖率

3. **性能测试结果**
   - 性能指标统计
   - 性能对比图表
   - 性能瓶颈分析

4. **对比测试结果**
   - ChronoDB vs Prometheus 功能对比
   - ChronoDB vs Prometheus 性能对比
   - ChronoDB vs Prometheus 存储成本对比

5. **资源使用分析**
   - 各节点 CPU 使用趋势图
   - 各节点内存使用趋势图
   - 磁盘 I/O 性能图表
   - 网络流量分析图表
   - ChronoDB vs Prometheus 资源使用对比
   - 资源瓶颈识别和优化建议

6. **建议和改进**
   - 发现的问题
   - 改进建议
   - 优化方向

### 7.2 报告生成器 (utils/report_generator.py)

```python
class ReportGenerator:
    """测试报告生成器"""
    
    def generate_html_report(
        self,
        test_results: Dict,
        output_path: str
    ):
        """生成 HTML 测试报告"""
        pass
    
    def generate_comparison_report(
        self,
        chronodb_results: Dict,
        prometheus_results: Dict,
        output_path: str
    ):
        """生成对比报告"""
        pass
    
    def generate_performance_charts(
        self,
        metrics: Dict,
        output_dir: str
    ):
        """生成性能图表"""
        pass
    
    def generate_resource_usage_charts(
        self,
        monitoring_data: Dict,
        output_dir: str
    ):
        """生成资源使用图表"""
        pass
    
    def analyze_resource_bottlenecks(
        self,
        resource_metrics: Dict
    ) -> Dict:
        """分析资源瓶颈"""
        pass
```

### 7.3 资源使用分析工具 (utils/resource_analyzer.py)

```python
class ResourceAnalyzer:
    """资源使用分析器"""
    
    def __init__(self, monitoring_prometheus_url: str):
        self.prometheus_url = monitoring_prometheus_url
    
    def query_cpu_usage(
        self,
        node: str,
        start_time: int,
        end_time: int
    ) -> List[Dict]:
        """查询 CPU 使用率"""
        query = f'cpu_usage_percent{{node="{node}"}}'
        return self._query_range(query, start_time, end_time)
    
    def query_memory_usage(
        self,
        node: str,
        start_time: int,
        end_time: int
    ) -> List[Dict]:
        """查询内存使用率"""
        query = f'memory_usage_percent{{node="{node}"}}'
        return self._query_range(query, start_time, end_time)
    
    def query_disk_io(
        self,
        node: str,
        start_time: int,
        end_time: int
    ) -> Dict:
        """查询磁盘 I/O"""
        read_query = f'disk_read_bytes_total{{node="{node}"}}'
        write_query = f'disk_written_bytes_total{{node="{node}"}}'
        return {
            'read': self._query_range(read_query, start_time, end_time),
            'write': self._query_range(write_query, start_time, end_time)
        }
    
    def query_network_traffic(
        self,
        node: str,
        start_time: int,
        end_time: int
    ) -> Dict:
        """查询网络流量"""
        receive_query = f'network_receive_bytes_total{{node="{node}"}}'
        transmit_query = f'network_transmit_bytes_total{{node="{node}"}}'
        return {
            'receive': self._query_range(receive_query, start_time, end_time),
            'transmit': self._query_range(transmit_query, start_time, end_time)
        }
    
    def compare_resource_usage(
        self,
        chronodb_node: str,
        prometheus_node: str,
        start_time: int,
        end_time: int
    ) -> Dict:
        """对比 ChronoDB 和 Prometheus 的资源使用"""
        return {
            'cpu': {
                'chronodb': self.query_cpu_usage(chronodb_node, start_time, end_time),
                'prometheus': self.query_cpu_usage(prometheus_node, start_time, end_time)
            },
            'memory': {
                'chronodb': self.query_memory_usage(chronodb_node, start_time, end_time),
                'prometheus': self.query_memory_usage(prometheus_node, start_time, end_time)
            },
            'disk_io': {
                'chronodb': self.query_disk_io(chronodb_node, start_time, end_time),
                'prometheus': self.query_disk_io(prometheus_node, start_time, end_time)
            },
            'network': {
                'chronodb': self.query_network_traffic(chronodb_node, start_time, end_time),
                'prometheus': self.query_network_traffic(prometheus_node, start_time, end_time)
            }
        }
    
    def identify_bottlenecks(
        self,
        resource_data: Dict,
        thresholds: Dict = None
    ) -> List[Dict]:
        """识别资源瓶颈"""
        if thresholds is None:
            thresholds = {
                'cpu_high': 80.0,
                'memory_high': 85.0,
                'disk_io_high': 100 * 1024 * 1024,
                'network_high': 50 * 1024 * 1024
            }
        
        bottlenecks = []
        
        for resource_type, data in resource_data.items():
            if resource_type == 'cpu':
                for node, values in data.items():
                    max_usage = max(v['value'] for v in values)
                    if max_usage > thresholds['cpu_high']:
                        bottlenecks.append({
                            'type': 'cpu',
                            'node': node,
                            'value': max_usage,
                            'threshold': thresholds['cpu_high'],
                            'severity': 'high' if max_usage > 90 else 'medium'
                        })
            
            elif resource_type == 'memory':
                for node, values in data.items():
                    max_usage = max(v['value'] for v in values)
                    if max_usage > thresholds['memory_high']:
                        bottlenecks.append({
                            'type': 'memory',
                            'node': node,
                            'value': max_usage,
                            'threshold': thresholds['memory_high'],
                            'severity': 'high' if max_usage > 95 else 'medium'
                        })
        
        return bottlenecks
    
    def _query_range(
        self,
        query: str,
        start_time: int,
        end_time: int
    ) -> List[Dict]:
        """执行范围查询"""
        url = f"{self.prometheus_url}/api/v1/query_range"
        params = {
            'query': query,
            'start': start_time,
            'end': end_time,
            'step': '15s'
        }
        response = requests.get(url, params=params)
        if response.status_code == 200:
            result = response.json()
            return result.get('data', {}).get('result', [])
        return []
```

## 8. 实施步骤

### 阶段一: 基础框架搭建 (1-2天)

1. 创建测试框架目录结构
2. 编写 Docker Compose 配置文件
3. 配置 prom-agent 监控采集
   - 创建 agent 配置文件
   - 配置监控 Prometheus
   - 配置 Grafana 数据源
4. 实现环境管理工具类
5. 实现数据生成器
6. 编写一键部署和清理脚本

### 阶段二: 基础测试实现 (2-3天)

1. 实现基础操作测试
2. 实现查询算子测试
3. 实现时间跨度测试
4. 编写测试 fixtures 和工具函数

### 阶段三: 对比测试实现 (2-3天)

1. 实现 ChronoDB vs Prometheus 功能对比
2. 实现性能对比测试
3. 实现存储成本对比测试
4. 实现分布式功能测试

### 阶段四: 性能测试实现 (2-3天)

1. 实现写入性能测试
2. 实现查询性能测试
3. 实现压力测试
4. 实现稳定性测试

### 阶段五: 资源监控分析实现 (1-2天)

1. 实现资源使用数据采集
2. 实现资源使用分析工具
3. 实现资源瓶颈识别
4. 实现资源使用对比分析
5. 生成资源使用图表和报告

### 阶段六: 报告和优化 (1-2天)

1. 实现测试报告生成器
2. 实现性能图表生成
3. 实现资源使用图表生成
4. 优化测试执行效率
5. 编写测试框架文档

## 9. 预期成果

1. **完整的测试框架**: 支持一键部署、自动测试、自动清理
2. **全面的测试覆盖**: 覆盖所有查询算子、时间跨度、分布式功能
3. **详细的对比报告**: ChronoDB vs Prometheus 的全面对比
4. **性能基准**: 建立性能基准数据，用于后续优化对比
5. **自动化流程**: CI/CD 集成，支持持续测试
6. **资源监控分析**: 
   - 实时监控各节点资源使用情况
   - 自动识别资源瓶颈
   - 提供资源优化建议
   - 生成资源使用趋势图表
   - ChronoDB vs Prometheus 资源使用对比分析

## 10. 技术要点

### 10.1 测试数据隔离

- 每个测试用例使用独立的指标名称
- 测试完成后自动清理测试数据
- 使用时间戳确保数据唯一性

### 10.2 测试可靠性

- 添加重试机制处理网络抖动
- 使用健康检查确保服务就绪
- 设置合理的超时时间
- 记录详细的测试日志

### 10.3 性能测试准确性

- 预热系统避免冷启动影响
- 多次运行取平均值
- 控制测试环境资源
- 监控系统资源使用

### 10.4 对比测试公平性

- 使用相同的数据集
- 使用相同的硬件环境
- 使用相同的测试方法
- 多次测试取平均值

### 10.5 资源监控准确性

- prom-agent 与被监控服务运行在同一容器网络
- 使用 container_mode: auto 自动识别容器环境
- 采集频率设置为 15 秒，平衡精度和性能
- 监控数据存储在独立的 monitoring-prometheus
- 测试完成后保留监控数据用于分析

### 10.6 监控数据分析

- 使用 Prometheus 查询 API 获取监控数据
- 自动识别资源使用峰值和异常
- 对比分析不同系统的资源使用模式
- 生成可视化图表辅助分析
- 提供优化建议基于监控数据

## 11. 风险和应对

### 11.1 测试环境不稳定

**风险**: Docker 环境可能存在网络、存储等问题

**应对**:
- 添加重试机制
- 记录详细日志
- 提供环境诊断工具

### 11.2 测试数据生成慢

**风险**: 大规模数据生成耗时较长

**应对**:
- 使用并行生成
- 缓存测试数据
- 提供数据预生成选项

### 11.3 测试执行时间长

**风险**: 完整测试套件执行时间过长

**应对**:
- 支持测试用例分组
- 支持并行执行
- 提供快速测试模式

## 12. 后续优化

1. **测试数据管理**: 支持测试数据版本化管理
2. **测试结果存储**: 建立测试结果数据库，支持历史对比
3. **可视化监控**: 实时监控测试执行状态
4. **智能测试**: 根据代码变更智能选择测试用例
5. **性能回归检测**: 自动检测性能回归问题
6. **资源监控增强**:
   - 添加更多监控指标（如 GC、线程池等）
   - 实现资源使用预测和预警
   - 支持自定义监控面板
   - 集成告警机制
7. **智能分析**:
   - 基于历史数据的异常检测
   - 自动生成优化建议
   - 性能趋势分析和预测
