# ChronoDB API 文档

## 概述

ChronoDB 提供了兼容 Prometheus 的 HTTP API，支持数据写入、查询和管理功能。

**基础 URL**: `http://localhost:9090`

**API 版本**: v1

---

## 数据写入 API

### Remote Write

写入时序数据到 ChronoDB。

**端点**: `POST /api/v1/write`

**请求头**:
- `Content-Type`: `application/x-protobuf`
- `Content-Encoding`: `snappy` (可选)
- `X-Prometheus-Remote-Write-Version`: `0.1.0`

**请求体**: Prometheus Remote Write protobuf 格式

**示例**:
```bash
curl -X POST http://localhost:9090/api/v1/write \
  -H "Content-Type: application/x-protobuf" \
  -H "X-Prometheus-Remote-Write-Version: 0.1.0" \
  --data-binary @write_request.bin
```

**响应**:
- `204 No Content`: 写入成功
- `400 Bad Request`: 请求格式错误
- `500 Internal Server Error`: 服务器内部错误

---

## 查询 API

### 即时查询

执行即时 PromQL 查询。

**端点**: `GET /api/v1/query`

**参数**:
- `query` (必需): PromQL 查询表达式
- `time` (可选): 查询时间戳（RFC3339 或 Unix 时间戳）
- `timeout` (可选): 查询超时时间

**示例**:
```bash
curl 'http://localhost:9090/api/v1/query?query=up'
curl 'http://localhost:9090/api/v1/query?query=rate(http_requests_total[5m])&time=2024-01-01T00:00:00Z'
```

**响应**:
```json
{
  "status": "success",
  "data": {
    "resultType": "vector",
    "result": [
      {
        "metric": {
          "__name__": "up",
          "job": "prometheus"
        },
        "value": [1704067200, "1"]
      }
    ]
  }
}
```

### 范围查询

执行范围 PromQL 查询。

**端点**: `GET /api/v1/query_range`

**参数**:
- `query` (必需): PromQL 查询表达式
- `start` (必需): 开始时间（RFC3339 或 Unix 时间戳）
- `end` (必需): 结束时间（RFC3339 或 Unix 时间戳）
- `step` (必需): 查询步长（持续时间格式或秒数）
- `timeout` (可选): 查询超时时间

**示例**:
```bash
curl 'http://localhost:9090/api/v1/query_range?query=up&start=2024-01-01T00:00:00Z&end=2024-01-01T01:00:00Z&step=15s'
```

**响应**:
```json
{
  "status": "success",
  "data": {
    "resultType": "matrix",
    "result": [
      {
        "metric": {
          "__name__": "up",
          "job": "prometheus"
        },
        "values": [
          [1704067200, "1"],
          [1704067215, "1"],
          [1704067230, "1"]
        ]
      }
    ]
  }
}
```

---

## 元数据 API

### 查询标签名称

获取所有标签名称。

**端点**: `GET /api/v1/labels`

**参数**:
- `start` (可选): 开始时间
- `end` (可选): 结束时间
- `match[]` (可选): 标签选择器

**示例**:
```bash
curl 'http://localhost:9090/api/v1/labels'
```

**响应**:
```json
{
  "status": "success",
  "data": [
    "__name__",
    "job",
    "instance"
  ]
}
```

### 查询标签值

获取指定标签的所有值。

**端点**: `GET /api/v1/label/<label_name>/values`

**参数**:
- `start` (可选): 开始时间
- `end` (可选): 结束时间
- `match[]` (可选): 标签选择器

**示例**:
```bash
curl 'http://localhost:9090/api/v1/label/job/values'
```

**响应**:
```json
{
  "status": "success",
  "data": [
    "prometheus",
    "node_exporter",
    "grafana"
  ]
}
```

### 查询序列

获取匹配标签选择器的序列列表。

**端点**: `GET /api/v1/series`

**参数**:
- `match[]` (必需): 标签选择器（可重复）
- `start` (可选): 开始时间
- `end` (可选): 结束时间

**示例**:
```bash
curl 'http://localhost:9090/api/v1/series?match[]=up'
```

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

---

## 元数据查询 API

### 查询指标元数据

获取指标的元数据信息。

**端点**: `GET /api/v1/metadata`

**参数**:
- `metric` (可选): 指标名称
- `limit` (可选): 返回结果数量限制

**示例**:
```bash
curl 'http://localhost:9090/api/v1/metadata?metric=up'
```

**响应**:
```json
{
  "status": "success",
  "data": {
    "up": [
      {
        "type": "gauge",
        "help": "1 if the instance is up, 0 otherwise",
        "unit": ""
      }
    ]
  }
}
```

---

## 健康检查 API

### 健康检查

检查服务健康状态。

**端点**: `GET /-/healthy`

**示例**:
```bash
curl http://localhost:9090/-/healthy
```

**响应**:
```
OK
```

### 就绪检查

检查服务是否就绪。

**端点**: `GET /-/ready`

**示例**:
```bash
curl http://localhost:9090/-/ready
```

**响应**:
```
OK
```

---

## 监控 API

### Prometheus 指标

获取 Prometheus 格式的监控指标。

**端点**: `GET /metrics`

**示例**:
```bash
curl http://localhost:9090/metrics
```

**响应**:
```
# HELP chronodb_series_total Total number of time series
# TYPE chronodb_series_total gauge
chronodb_series_total 12345

# HELP chronodb_samples_total Total number of samples
# TYPE chronodb_samples_total gauge
chronodb_samples_total 1234567

# HELP chronodb_query_latency_ms Average query latency in milliseconds
# TYPE chronodb_query_latency_ms gauge
chronodb_query_latency_ms 15.5
```

---

## 管理 API

### 重载配置

重新加载配置文件。

**端点**: `POST /-/reload`

**示例**:
```bash
curl -X POST http://localhost:9090/-/reload
```

### 退出服务

优雅关闭服务。

**端点**: `POST /-/quit`

**示例**:
```bash
curl -X POST http://localhost:9090/-/quit
```

---

## 备份 API

### 创建备份

创建数据备份。

**端点**: `POST /api/v1/admin/backup`

**参数**:
- `type`: 备份类型（full | incremental）
- `destination`: 备份目标路径

**示例**:
```bash
curl -X POST 'http://localhost:9090/api/v1/admin/backup?type=full&destination=/backups/backup-2024-01-01'
```

**响应**:
```json
{
  "status": "success",
  "data": {
    "backup_id": "backup_20240101_123456",
    "files_count": 100,
    "total_size": 1073741824
  }
}
```

### 恢复备份

从备份恢复数据。

**端点**: `POST /api/v1/admin/restore`

**参数**:
- `backup_id`: 备份 ID

**示例**:
```bash
curl -X POST 'http://localhost:9090/api/v1/admin/restore?backup_id=backup_20240101_123456'
```

---

## 降采样 API

### 查询降采样数据

查询降采样后的数据。

**端点**: `GET /api/v1/downsample/query`

**参数**:
- `query`: PromQL 查询表达式
- `resolution`: 降采样分辨率（如 5m, 1h, 1d）
- `start`: 开始时间
- `end`: 结束时间

**示例**:
```bash
curl 'http://localhost:9090/api/v1/downsample/query?query=rate(http_requests_total[5m])&resolution=1h&start=2024-01-01T00:00:00Z&end=2024-01-02T00:00:00Z'
```

---

## 自然语言查询 API

### 自然语言查询

使用自然语言查询数据。

**端点**: `POST /api/v1/nlp/query`

**请求体**:
```json
{
  "query": "Show me the average CPU usage for the last hour",
  "time_range": "1h"
}
```

**示例**:
```bash
curl -X POST http://localhost:9090/api/v1/nlp/query \
  -H "Content-Type: application/json" \
  -d '{"query": "Show me the average CPU usage for the last hour", "time_range": "1h"}'
```

**响应**:
```json
{
  "status": "success",
  "data": {
    "promql": "avg(cpu_usage) [1h]",
    "result": {
      "resultType": "vector",
      "result": [...]
    }
  }
}
```

---

## 错误响应

所有 API 在发生错误时返回统一的错误格式：

```json
{
  "status": "error",
  "errorType": "bad_data",
  "error": "invalid parameter 'query': parse error"
}
```

**错误类型**:
- `bad_data`: 请求参数错误
- `execution`: 查询执行错误
- `internal`: 内部服务器错误
- `unavailable`: 服务不可用

---

## 速率限制

API 请求受以下速率限制：

- **写入 API**: 10,000 请求/秒
- **查询 API**: 1,000 请求/秒
- **管理 API**: 10 请求/秒

超过限制将返回 `429 Too Many Requests` 错误。

---

## 认证

部分 API 需要认证（如果启用了认证）：

**请求头**:
- `Authorization`: `Bearer <token>`

**示例**:
```bash
curl -H "Authorization: Bearer my-token" http://localhost:9090/api/v1/query?query=up
```

---

## 版本信息

**API 版本**: v1  
**ChronoDB 版本**: 0.1.0  
**Prometheus 兼容版本**: 2.45.0

---

## 支持的 PromQL 功能

### 聚合操作符
- `sum`, `avg`, `min`, `max`, `count`
- `stddev`, `stdvar`, `topk`, `bottomk`
- `group`, `quantile`

### 函数
- **数学函数**: `abs`, `ceil`, `floor`, `round`, `sqrt`
- **时间函数**: `rate`, `irate`, `increase`, `delta`
- **标签函数**: `label_replace`, `label_join`
- **类型转换**: `vector`, `scalar`

### 操作符
- **算术操作符**: `+`, `-`, `*`, `/`, `%`, `^`
- **比较操作符**: `==`, `!=`, `>`, `<`, `>=`, `<=`
- **逻辑操作符**: `and`, `or`, `unless`

---

## 客户端库

推荐使用以下客户端库：

- **Go**: `github.com/prometheus/client_golang`
- **Java**: `io.prometheus:simpleclient`
- **Python**: `prometheus_client`
- **Rust**: `prometheus`

---

## 最佳实践

1. **批量写入**: 使用批量写入减少网络开销
2. **合理设置时间范围**: 避免查询过大的时间范围
3. **使用标签过滤**: 通过标签选择器减少查询数据量
4. **监控查询性能**: 关注慢查询日志
5. **定期备份**: 设置自动备份策略

---

## 联系支持

- **文档**: https://chronodb.io/docs
- **GitHub**: https://github.com/chronodb/chronodb
- **社区**: https://community.chronodb.io
