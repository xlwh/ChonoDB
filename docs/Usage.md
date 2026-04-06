# ChronoDB 使用文档

## 目录

1. [快速开始](#快速开始)
2. [配置文件](#配置文件)
3. [CLI 命令](#cli-命令)
4. [HTTP API](#http-api)
5. [数据导入导出](#数据导入导出)
6. [运维管理](#运维管理)
7. [性能调优](#性能调优)

## 快速开始

### 安装

```bash
# 从源码编译
git clone https://github.com/your-org/chronodb.git
cd chronodb
cargo build --release

# 安装到系统
sudo cp target/release/chronodb /usr/local/bin/
```

### 启动服务器

```bash
# 使用默认配置启动
chronodb

# 指定数据目录和监听地址
chronodb server --data-dir /var/lib/chronodb --listen 0.0.0.0:9090

# 使用配置文件
chronodb --config /etc/chronodb/config.yaml
```

### 写入数据

```bash
# 使用 HTTP API 写入数据
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
```

### 查询数据

```bash
# 即时查询
curl "http://localhost:9090/api/v1/query?query=cpu_usage&time=1609459200000"

# 范围查询
curl "http://localhost:9090/api/v1/query_range?query=cpu_usage&start=1609459200000&end=1609459260000&step=60"
```

## 配置文件

### 基本配置

创建 `/etc/chronodb/config.yaml`:

```yaml
# 监听地址
listen_address: "0.0.0.0:9090"

# 存储配置
storage:
  mode: standalone
  data_dir: /var/lib/chronodb
  backend: local

# 降采样配置
downsampling:
  enabled: true
  levels:
    - level: L0
      resolution: 10s
      retention: 168h
    - level: L1
      resolution: 1m
      retention: 720h
    - level: L2
      resolution: 5m
      retention: 2160h
    - level: L3
      resolution: 1h
      retention: 8760h
    - level: L4
      resolution: 1d
      retention: 87600h

# 内存配置
memory:
  memstore_size: 4GB
  wal_size: 1GB
  query_cache_size: 2GB

# 查询配置
query:
  max_concurrent: 100
  timeout: 2m
  max_samples: 50000000
  enable_vectorized: true
  enable_parallel: true
  enable_auto_downsampling: true
  downsample_policy: auto

# 数据保留策略
retention:
  hot: 24h
  warm: 168h
  cold: 720h
  archive: 8760h

# 日志配置
log:
  level: info
  format: json
  output: /var/log/chronodb/chronodb.log
```

### 配置热加载

ChronoDB 支持配置热加载，修改配置文件后发送 SIGHUP 信号:

```bash
kill -HUP $(pgrep chronodb)
```

## CLI 命令

### 服务器管理

```bash
# 启动服务器
chronodb server

# 后台运行
chronodb server --daemon

# 指定配置文件
chronodb server --config /etc/chronodb/config.yaml
```

### 数据检查

```bash
# 检查数据完整性
chronodb check

# 检查指定目录
chronodb check --data-dir /var/lib/chronodb
```

### 数据压缩

```bash
# 压缩数据
chronodb compact

# 查看压缩前后统计
chronodb compact --verbose
```

### 备份和恢复

```bash
# 创建备份
chronodb backup --backup-dir /backup/chronodb

# 恢复数据
chronodb restore --backup-dir /backup/chronodb/backup_1234567890

# 恢复到指定目录
chronodb restore --backup-dir /backup/chronodb/backup_1234567890 --data-dir /var/lib/chronodb
```

### 数据清理

```bash
# 清理 30 天前的数据
chronodb cleanup --retention-days 30

# 清理指定目录
chronodb cleanup --data-dir /var/lib/chronodb --retention-days 7
```

### 数据导入导出

```bash
# 导出数据
chronodb export --output data.json

# 导入数据
chronodb import --input data.json

# 导出为 Prometheus 格式
chronodb export --output data.prometheus --format prometheus
```

## HTTP API

ChronoDB 提供与 Prometheus 兼容的 HTTP API。

### 写入 API

#### Remote Write

```bash
POST /api/v1/write
Content-Type: application/x-protobuf
Content-Encoding: snappy

# 请求体为 Prometheus remote write 格式的 protobuf 数据
```

#### JSON 写入

```bash
POST /api/v1/write
Content-Type: application/json

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

### 查询 API

#### 即时查询

```bash
GET /api/v1/query?query=up&time=1609459200.000

# 响应
{
  "status": "success",
  "data": {
    "resultType": "vector",
    "result": [{
      "metric": {"__name__": "up", "job": "prometheus"},
      "value": [1609459200, "1"]
    }]
  }
}
```

#### 范围查询

```bash
GET /api/v1/query_range?query=up&start=1609459200&end=1609459260&step=60

# 响应
{
  "status": "success",
  "data": {
    "resultType": "matrix",
    "result": [{
      "metric": {"__name__": "up", "job": "prometheus"},
      "values": [
        [1609459200, "1"],
        [1609459260, "1"]
      ]
    }]
  }
}
```

### 元数据 API

#### 获取系列列表

```bash
GET /api/v1/series?match[]={job="prometheus"}&start=1609459200&end=1609459260
```

#### 获取标签列表

```bash
GET /api/v1/labels?start=1609459200&end=1609459260
```

#### 获取标签值

```bash
GET /api/v1/label/job/values?start=1609459200&end=1609459260
```

### 状态 API

#### 构建信息

```bash
GET /api/v1/status/buildinfo
```

#### 运行时信息

```bash
GET /api/v1/status/runtimeinfo
```

#### TSDB 状态

```bash
GET /api/v1/status/tsdb
```

### 健康检查

```bash
# 健康检查
GET /health

# 就绪检查
GET /ready

# 指标
GET /metrics
```

## 数据导入导出

### 从 Prometheus 迁移

```bash
# 1. 导出 Prometheus 数据
promtool tsdb dump /var/lib/prometheus > prometheus_data.txt

# 2. 转换为 ChronoDB 格式
chronodb convert --input prometheus_data.txt --output chronodb_data.json

# 3. 导入到 ChronoDB
chronodb import --input chronodb_data.json
```

### 导出为 CSV

```bash
# 导出查询结果为 CSV
chronodb export --query "cpu_usage" --format csv --output cpu_usage.csv
```

### 批量导入

```bash
# 使用批量导入工具
chronodb import --input data.json --batch-size 10000 --workers 4
```

## 运维管理

### 监控

ChronoDB 暴露 Prometheus 格式的指标:

```yaml
# prometheus.yml
scrape_configs:
  - job_name: 'chronodb'
    static_configs:
      - targets: ['localhost:9090']
```

关键指标:

- `chronodb_series_total` - 时间序列总数
- `chronodb_samples_total` - 样本总数
- `chronodb_storage_bytes` - 存储使用量
- `chronodb_writes_total` - 写入次数
- `chronodb_reads_total` - 读取次数

### 日志管理

```bash
# 查看日志
tail -f /var/log/chronodb/chronodb.log

# 日志轮转
logrotate /etc/logrotate.d/chronodb
```

### 备份策略

```bash
# 创建每日备份脚本
#!/bin/bash
BACKUP_DIR=/backup/chronodb/$(date +%Y%m%d)
chronodb backup --backup-dir $BACKUP_DIR

# 保留最近 7 天的备份
find /backup/chronodb -type d -mtime +7 -exec rm -rf {} \;
```

### 性能监控

```bash
# 查看性能统计
chronodb stats

# 查看查询性能
chronodb query-stats

# 查看存储使用
chronodb storage-stats
```

## 性能调优

### 内存调优

```yaml
# config.yaml
memory:
  # 根据数据量调整
  memstore_size: 8GB      # 大数据量: 8-16GB
  wal_size: 2GB
  query_cache_size: 4GB
```

### 查询优化

```yaml
# config.yaml
query:
  # 启用向量化执行
  enable_vectorized: true
  
  # 启用并行查询
  enable_parallel: true
  
  # 启用自动降采样
  enable_auto_downsampling: true
  downsample_policy: auto  # auto, conservative, aggressive
```

### 压缩配置

```yaml
# config.yaml
compression:
  time_column:
    algorithm: zstd
    level: 3
  value_column:
    algorithm: zstd
    level: 3
    use_prediction: true  # 启用预测编码
  label_column:
    algorithm: dictionary
```

### 系统调优

```bash
# 增加文件描述符限制
ulimit -n 65535

# 调整内核参数
sysctl -w vm.max_map_count=262144
sysctl -w net.core.somaxconn=65535
```

## 故障排除

### 常见问题

#### 启动失败

```bash
# 检查数据目录权限
ls -la /var/lib/chronodb

# 检查端口占用
netstat -tlnp | grep 9090

# 查看详细日志
chronodb server --log-level debug
```

#### 查询超时

```yaml
# 增加查询超时时间
query:
  timeout: 5m
  max_samples: 100000000
```

#### 内存不足

```yaml
# 减少内存使用
memory:
  memstore_size: 2GB
  query_cache_size: 1GB
```

### 获取帮助

```bash
# 查看帮助
chronodb --help

# 查看版本
chronodb --version

# 查看子命令帮助
chronodb server --help
```

## 最佳实践

1. **定期备份**: 建议每天备份，保留 7 天
2. **监控告警**: 设置磁盘空间、内存使用告警
3. **数据分层**: 根据访问频率配置数据分层策略
4. **查询优化**: 使用标签过滤减少查询范围
5. **降采样**: 对历史数据启用自动降采样

## 更多信息

- [API 文档](API.md)
- [架构设计](ChronoDB-Design.md)
- [GitHub 仓库](https://github.com/your-org/chronodb)
