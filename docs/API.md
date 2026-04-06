# ChronoDB API 文档

## 概述

ChronoDB 提供与 Prometheus 兼容的 HTTP API，同时扩展了额外的功能接口。

## 基础信息

- **Base URL**: `http://localhost:9090`
- **Content-Type**: `application/json`
- **认证**: 当前版本不支持认证（生产环境建议通过反向代理添加）

## API 端点

### 写入接口

#### Remote Write

接收 Prometheus remote write 格式的数据。

```http
POST /api/v1/write
Content-Type: application/x-protobuf
Content-Encoding: snappy
X-Prometheus-Remote-Write-Version: 0.1.0
```

**请求体**: Prometheus remote write 格式的 protobuf 数据（Snappy 压缩）

**响应**:
```json
{
  "status": "success"
}
```

#### JSON 写入

通过 JSON 格式写入数据。

```http
POST /api/v1/write
Content-Type: application/json
```

**请求体**:
```json
{
  "timeseries": [{
    "labels": [
      {"name": "__name__", "value": "metric_name"},
      {"name": "label1", "value": "value1"}
    ],
    "samples": [
      {"timestamp": 1609459200000, "value": 123.45}
    ]
  }]
}
```

**参数说明**:
| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| timeseries | array | 是 | 时间序列数组 |
| timeseries[].labels | array | 是 | 标签数组 |
| timeseries[].labels[].name | string | 是 | 标签名 |
| timeseries[].labels[].value | string | 是 | 标签值 |
| timeseries[].samples | array | 是 | 样本数组 |
| timeseries[].samples[].timestamp | integer | 是 | 时间戳（毫秒） |
| timeseries[].samples[].value | number | 是 | 样本值 |

### 查询接口

#### 即时查询

在指定时间点执行即时查询。

```http
GET /api/v1/query?query=up&time=1609459200.000
```

**参数**:
| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| query | string | 是 | PromQL 查询表达式 |
| time | float | 否 | 查询时间戳（秒），默认当前时间 |
| timeout | string | 否 | 查询超时时间 |

**响应**:
```json
{
  "status": "success",
  "data": {
    "resultType": "vector",
    "result": [{
      "metric": {
        "__name__": "up",
        "job": "prometheus",
        "instance": "localhost:9090"
      },
      "value": [1609459200, "1"]
    }]
  }
}
```

#### 范围查询

在指定时间范围内执行查询。

```http
GET /api/v1/query_range?query=up&start=1609459200&end=1609459260&step=60
```

**参数**:
| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| query | string | 是 | PromQL 查询表达式 |
| start | float | 是 | 开始时间戳（秒） |
| end | float | 是 | 结束时间戳（秒） |
| step | float | 是 | 查询步长（秒） |
| timeout | string | 否 | 查询超时时间 |

**响应**:
```json
{
  "status": "success",
  "data": {
    "resultType": "matrix",
    "result": [{
      "metric": {
        "__name__": "up",
        "job": "prometheus"
      },
      "values": [
        [1609459200, "1"],
        [1609459260, "1"],
        [1609459320, "1"]
      ]
    }]
  }
}
```

### 元数据接口

#### 获取系列列表

```http
GET /api/v1/series?match[]={job="prometheus"}&start=1609459200&end=1609459260
```

**参数**:
| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| match[] | string | 是 | 系列选择器（可多次指定） |
| start | float | 否 | 开始时间戳 |
| end | float | 否 | 结束时间戳 |

**响应**:
```json
{
  "status": "success",
  "data": [
    {
      "__name__": "up",
      "job": "prometheus",
      "instance": "localhost:9090"
    }
  ]
}
```

#### 获取标签列表

```http
GET /api/v1/labels?start=1609459200&end=1609459260
```

**参数**:
| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| start | float | 否 | 开始时间戳 |
| end | float | 否 | 结束时间戳 |

**响应**:
```json
{
  "status": "success",
  "data": ["__name__", "job", "instance"]
}
```

#### 获取标签值

```http
GET /api/v1/label/job/values?start=1609459200&end=1609459260
```

**参数**:
| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| label_name | string | 是 | 标签名（URL 路径参数） |
| start | float | 否 | 开始时间戳 |
| end | float | 否 | 结束时间戳 |

**响应**:
```json
{
  "status": "success",
  "data": ["prometheus", "node-exporter"]
}
```

### 状态接口

#### 构建信息

```http
GET /api/v1/status/buildinfo
```

**响应**:
```json
{
  "status": "success",
  "data": {
    "version": "1.0.0",
    "revision": "abc123",
    "branch": "main",
    "buildUser": "chronodb",
    "buildDate": "2024-01-01",
    "goVersion": "n/a"
  }
}
```

#### 运行时信息

```http
GET /api/v1/status/runtimeinfo
```

**响应**:
```json
{
  "status": "success",
  "data": {
    "startTime": "2024-01-01T00:00:00Z",
    "uptime": "72h30m",
    "cwd": "/var/lib/chronodb",
    "reloadConfigSuccess": true,
    "lastConfigTime": "2024-01-01T00:00:00Z",
    "corruptionCount": 0,
    "goroutineCount": 42,
    "storage": {
      "series_count": 10000,
      "sample_count": 1000000,
      "disk_usage": 1073741824
    }
  }
}
```

#### TSDB 状态

```http
GET /api/v1/status/tsdb
```

**响应**:
```json
{
  "status": "success",
  "data": {
    "headStats": {
      "numSeries": 10000,
      "numLabelPairs": 50000,
      "chunkCount": 50000,
      "minTime": 1609459200000,
      "maxTime": 1609459260000
    },
    "seriesCountByMetricName": [
      {"name": "up", "value": 100}
    ],
    "labelValueCountByLabelName": [
      {"name": "job", "value": 10}
    ]
  }
}
```

### 健康检查接口

#### 健康检查

```http
GET /health
```

**响应**:
- `200 OK`: 服务健康
- `503 Service Unavailable`: 服务不健康

#### 就绪检查

```http
GET /ready
```

**响应**:
- `200 OK`: 服务就绪
- `503 Service Unavailable`: 服务未就绪

#### 指标

```http
GET /metrics
```

**响应**: Prometheus 格式的指标数据

```
# HELP chronodb_series_total Total number of time series
# TYPE chronodb_series_total gauge
chronodb_series_total 10000

# HELP chronodb_samples_total Total number of samples
# TYPE chronodb_samples_total gauge
chronodb_samples_total 1000000

# HELP chronodb_storage_bytes Total storage size in bytes
# TYPE chronodb_storage_bytes gauge
chronodb_storage_bytes 1073741824

# HELP chronodb_writes_total Total number of writes
# TYPE chronodb_writes_total counter
chronodb_writes_total 50000

# HELP chronodb_reads_total Total number of reads
# TYPE chronodb_reads_total counter
chronodb_reads_total 100000
```

## 错误处理

### 错误响应格式

```json
{
  "status": "error",
  "errorType": "bad_data",
  "error": "invalid query expression"
}
```

### 错误类型

| 错误类型 | 说明 | HTTP 状态码 |
|----------|------|-------------|
| bad_data | 请求数据无效 | 400 |
| timeout | 查询超时 | 422 |
| canceled | 查询被取消 | 422 |
| execution | 查询执行错误 | 422 |
| internal | 内部服务器错误 | 500 |
| unavailable | 服务不可用 | 503 |

## 查询语言

ChronoDB 支持 PromQL（Prometheus Query Language）进行数据查询。

### 基本查询

```promql
# 查询指标
http_requests_total

# 带标签过滤
http_requests_total{job="prometheus", instance="localhost:9090"}

# 范围查询
http_requests_total[5m]

# 偏移查询
http_requests_total offset 1h
```

### 聚合操作

```promql
# 求和
sum(http_requests_total)

# 按标签分组求和
sum by (job) (http_requests_total)

# 平均值
avg(http_requests_total)

# 最大值
max(http_requests_total)

# 最小值
min(http_requests_total)

# 计数
count(http_requests_total)
```

### 函数

```promql
# 计算增长率
rate(http_requests_total[5m])

# 计算瞬时增长率
irate(http_requests_total[5m])

# 计算增量
increase(http_requests_total[1h])

# 计算差值
delta(http_requests_total[1h])

# 分位数
histogram_quantile(0.95, rate(http_request_duration_seconds_bucket[5m]))
```

### 二元操作

```promql
# 算术运算
http_requests_total / 100

# 比较运算
http_requests_total > 100

# 逻辑运算
http_requests_total and up

# 向量匹配
http_requests_total / on(instance) up
```

## 示例

### 写入示例

```bash
# 写入单个样本
curl -X POST http://localhost:9090/api/v1/write \
  -H "Content-Type: application/json" \
  -d '{
    "timeseries": [{
      "labels": [
        {"name": "__name__", "value": "cpu_usage"},
        {"name": "host", "value": "server1"}
      ],
      "samples": [
        {"timestamp": 1609459200000, "value": 45.5}
      ]
    }]
  }'

# 批量写入
curl -X POST http://localhost:9090/api/v1/write \
  -H "Content-Type: application/json" \
  -d '{
    "timeseries": [
      {
        "labels": [
          {"name": "__name__", "value": "cpu_usage"},
          {"name": "host", "value": "server1"}
        ],
        "samples": [
          {"timestamp": 1609459200000, "value": 45.5},
          {"timestamp": 1609459260000, "value": 46.0}
        ]
      },
      {
        "labels": [
          {"name": "__name__", "value": "memory_usage"},
          {"name": "host", "value": "server1"}
        ],
        "samples": [
          {"timestamp": 1609459200000, "value": 80.0}
        ]
      }
    ]
  }'
```

### 查询示例

```bash
# 即时查询
curl "http://localhost:9090/api/v1/query?query=cpu_usage"

# 范围查询
curl "http://localhost:9090/api/v1/query_range?query=cpu_usage&start=1609459200&end=1609459260&step=60"

# 带标签过滤的查询
curl "http://localhost:9090/api/v1/query?query=cpu_usage{host=\"server1\"}"

# 聚合查询
curl "http://localhost:9090/api/v1/query?query=sum(cpu_usage)by(host)"

# 函数查询
curl "http://localhost:9090/api/v1/query?query=rate(cpu_usage[5m])"
```

## 限制

### 查询限制

- 最大查询时间范围：1 年
- 最大返回样本数：50,000,000
- 查询超时：2 分钟（可配置）
- 最大并发查询数：100（可配置）

### 写入限制

- 单次写入最大系列数：10,000
- 单次写入最大样本数：100,000
- 标签名最大长度：512 字节
- 标签值最大长度：2048 字节
- 标签数量限制：32 个/系列

## SDK

### Go SDK

```go
import "github.com/your-org/chronodb-go-sdk"

client := chronodb.NewClient("http://localhost:9090")

// 写入数据
err := client.Write(ctx, &chronodb.TimeSeries{
    Labels: []chronodb.Label{
        {Name: "__name__", Value: "cpu_usage"},
        {Name: "host", Value: "server1"},
    },
    Samples: []chronodb.Sample{
        {Timestamp: time.Now().UnixMilli(), Value: 45.5},
    },
})

// 查询数据
result, err := client.Query(ctx, "cpu_usage", time.Now())
```

### Python SDK

```python
from chronodb import Client

client = Client("http://localhost:9090")

# 写入数据
client.write({
    "labels": [
        {"name": "__name__", "value": "cpu_usage"},
        {"name": "host", "value": "server1"},
    ],
    "samples": [
        {"timestamp": int(time.time() * 1000), "value": 45.5},
    ],
})

# 查询数据
result = client.query("cpu_usage", time.time())
```

## 变更日志

### v1.0.0 (2024-01-01)

- 初始版本发布
- 支持 Prometheus HTTP API v1
- 支持 Remote Write 协议
- 支持 PromQL 查询语言
- 支持自动降采样
- 支持数据分层存储
