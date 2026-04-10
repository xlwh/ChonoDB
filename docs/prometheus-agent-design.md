# Prometheus数据上报Agent设计方案

## 项目概述

本项目旨在开发一个高性能的Prometheus数据上报Agent，能够像node exporter一样采集容器或物理机的监控数据，同时支持配置多个服务的metrics endpoint定期抓取，并以remote write协议主动上报到指定的Prometheus remote write endpoint。

## 架构设计

### 整体架构图

```
┌─────────────────────────────────────────────┐
│            Prometheus Agent                  │
├─────────────────────────────────────────────┤
│  ┌─────────────┐   ┌─────────────────────┐  │
│  │   Config    │   │    Health Check     │  │
│  │   Manager   │   │     Service         │  │
│  └─────────────┘   └─────────────────────┘  │
├─────────────────────────────────────────────┤
│  ┌─────────────┐   ┌─────────────────────┐  │
│  │   System    │   │   Service Metrics   │  │
│  │ Collectors  │   │    Scrapers         │  │
│  │  (CPU/RAM)  │   │   (HTTP fetch)      │  │
│  └─────────────┘   └─────────────────────┘  │
├─────────────────────────────────────────────┤
│  ┌─────────────────────────────────────────┐ │
│  │        Remote Write Client              │ │
│  │    ┌─────────┐    ┌──────────────┐     │ │
│  │    │  Batcher│    │Retry Policy  │     │ │
│  │    └─────────┘    └──────────────┘     │ │
│  └─────────────────────────────────────────┘ │
└─────────────────────────────────────────────┘
        │                       │
        ↓                       ↓
   /proc, /sys           Service Endpoints
        │                       │
        ↓                       ↓
   Prometheus Remote Write API
```

## 核心模块设计

### 1. 系统监控采集器 (System Collectors)

参考node exporter的插件式架构，采用类似的collector模式：

```go
// Collector接口定义
type Collector interface {
    Name() string
    Update(ch chan<- prometheus.Metric) error
    Enabled() bool
    SetEnabled(enabled bool)
}

// 系统信息采集器集合管理
type SystemCollectorManager struct {
    collectors map[string]Collector
    logger     *zap.Logger
}
```

**支持的采集器类型：**
- `CPUCollector`: 收集CPU使用率、时间片分布、频率等
- `MemoryCollector`: 收集内存使用量、缓存、交换区状态等
- `DiskCollector`: 收集磁盘IO统计、分区使用情况等
- `FilesystemCollector`: 收集挂载点使用情况、inode统计等
- `NetCollector`: 收集网络接口流量、包统计等
- `LoadCollector`: 收集系统负载信息
- `ProcessCollector`: 收集进程和线程统计

**容器和物理机支持：**
- 自动检测运行环境（物理机vs容器）
- 容器环境下自动检测正确的procfs/sysfs挂载点
- 支持容器级别的资源限制数据采集

### 2. 服务Metrics抓取器 (Service Metrics Scrapers)

负责定期抓取配置的服务endpoint提供的metrics数据：

```go
type ServiceScraper struct {
    ID           string
    URL          string
    Interval     time.Duration
    Timeout      time.Duration
    AuthConfig   *AuthConfig
    Labels       map[string]string
    Client       *http.Client
}

type ScraperManager struct {
    scrapers map[string]*ServiceScraper
    ticker   *time.Ticker
    metrics  *prometheus.Registry
}
```

**功能特性：**
- 支持多个target并发抓取
- 可配置的抓取频率（全局默认+单独target配置）
- 支持HTTP Basic Auth、Bearer Token、客户端证书认证
- 支持为每个target添加自定义标签
- 抓取超时控制和错误处理
- 智能退避算法处理失败重试

### 3. Remote Write客户端 (Remote Write Client)

负责将收集的metrics数据按照remote write协议上报：

```go
type RemoteWriteConfig struct {
    Endpoint    string
    QueueConfig QueueConfig
    AuthConfig  *AuthConfig
    TLSConfig   *TLSConfig
}

type QueueConfig struct {
    Capacity           int
    MaxShards          int
    MaxSamplesPerSend  int
    BatchSendDeadline  time.Duration
    MaxRetries         int
    MinBackoff         time.Duration
    MaxBackoff         time.Duration
}

type RemoteWriteClient struct {
    config      *RemoteWriteConfig
    metricStore *MetricStore
    httpClient  *http.Client
    mu          sync.RWMutex
}
```

**上报策略：**
- 本地数据缓冲和批处理
- 分片并发发送提高吞吐量
- 指数退避重试策略
- 网络故障时的本地数据持久化（可选）
- 采样率控制和数据压缩

### 4. 配置管理系统 (Configuration Management)

采用YAML配置文件格式：

```yaml
# agent_config.yaml
agent:
  log_level: info  # debug, info, warn, error
  listen_address: 0.0.0.0:9090  # 可选的本地监控端口
  metrics_path: /metrics          # 可选的本地metrics路径

# 系统监控配置
system_collector:
  enabled: true
  collectors:
    cpu: true
    memory: true
    disk: true
    network: true
    load: true
  container_mode: auto  # auto, force, disable

# 服务监控配置
service_scrapers:
  - id: webapp1
    url: http://localhost:8080/metrics
    interval: 30s
    timeout: 5s
    labels:
      service: webapp1
      environment: production
    auth:
      type: bearer
      token: your-token-here
  - id: db1
    url: http://db.example.com:9100/metrics
    interval: 60s
    timeout: 10s

# Remote Write配置
remote_write:
  endpoint: http://prometheus.example.com:9090/api/v1/write
  auth:
    type: basic
    username: your-username
    password: your-password
  queue_config:
    capacity: 10000
    max_shards: 5
    max_samples_per_send: 1000
    batch_send_deadline: 5s
    max_retries: 3
    min_backoff: 3s
    max_backoff: 10s
```

**配置特性：**
- 支持配置文件热重载
- 环境变量覆盖配置
- 敏感配置项的加密存储（可选）
- 配置验证和合理性检查

## 技术栈选择

### 核心依赖
- **语言**: Go 1.21+（高性能、并发能力强、丰富的Prometheus生态）
- **Prometheus SDK**: prometheus/client_golang（官方规范实现）
- **HTTP客户端**: net/http标准库，支持各种认证和超时控制
- **配置解析**: YAML格式，使用gopkg.in/yaml.v2
- **日志**: uber/zap（高性能结构化日志）
- **容器检测**: 使用cgroup探测和挂载点分析
- **HTTP服务器**: net/http（可选的本地监控服务端）

### 构建和部署
- **构建工具**: go build + go modules
- **容器化**: Docker多阶段构建
- **部署**: Kubernetes DaemonSet（容器环境）或systemd服务（物理机）
- **监控**: 内置健康检查和metrics endpoint自检

## 数据流设计

### 采集流程
1. 采集系统初始化：注册系统采集器和启动服务抓取器
2. 定期采集：每个collector独立定时执行Update()方法
3. 数据聚合：所有采集器的数据统一收集到共享的metrics缓冲
4. 服务抓取：配置的HTTP endpoint定期抓取并和系统数据merge
5. 数据上报：remote write客户端批量处理和上报

### 错误处理策略
- **采集器隔离**：单个collector失败不影响整个系统
- **网络容错**：远程上报失败时本地缓冲，避免数据丢失
- **资源控制**：限制内存使用，设置数据保留期限
- **降级策略**：在资源紧张时优先丢弃历史数据而非实时数据

## 性能优化

### 内存管理
- 流式处理大数据量，避免内存峰值
- 对象池化重用连接和缓冲区
- 垃圾回收优化（Go内存管理）

### 并发控制
- 合理设置goroutine池大小
- 使用工作池模式处理上报任务
- 控制最大并发数防止资源耗尽

### 网络优化
- HTTP连接复用和keep-alive
- 数据压缩减小网络传输量
- 批量处理减少网络请求次数

## 监控和运维

### 内置监控指标
- agent_uptime_seconds: Agent运行时间
- agent_collector_duration_seconds: 各采集器执行时间
- agent_scraper_duration_seconds: 各服务抓取器执行时间
- agent_remote_write_requests_total: 远程写入请求计数
- agent_remote_write_errors_total: 远程写入错误计数
- agent_memory_bytes: 内存使用量
- agent_goroutines: 当前goroutine数量

### 健康检查
- HTTP健康检查endpoint: `/health`
- 可读性检查: `/readyz`
- 运行时性能分析: `/debug/pprof`（可选）

### 日志和调试
- 结构化日志，支持不同级别和输出格式
- debug模式下显示协议层面的详细信息
- 性能profiling支持
- 内存和goroutine泄露检测

## 扩展性设计

### 插件式采集器
- 通过接口规范支持动态扩展新的系统信息源
- 支持第三方collector通过配置文件加载

### 多协议支持
- 支持prometheus text exposition格式作为输入
- 支持json格式数据结构（可选）
- 支持OpenTelemetry协议（未来扩展）

### 云原生适配
- Kubernetes自定义资源定义（CRD）支持
- Operator模式管理（未来扩展）
- Service discovery集成Kubernetes和Consul

通过此设计方案，我们将构建一个高性能、高可靠性的Prometheus数据上报Agent，能够很好的满足现代云原生环境下的监控需求。