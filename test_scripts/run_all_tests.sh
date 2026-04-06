#!/bin/bash

# 测试脚本主文件
# 用于准备环境并运行所有测试用例

set -e

echo "=== ChronoDB 与 Prometheus 接口功能对比测试 ==="
echo "开始准备测试环境..."

# 检查Python是否安装
if ! command -v python3 &> /dev/null; then
    echo "错误: Python 3 未安装"
    exit 1
fi

# 检查pip是否安装
if ! command -v pip3 &> /dev/null; then
    echo "错误: pip3 未安装"
    exit 1
fi

# 安装必要的Python依赖
echo "安装必要的Python依赖..."
pip3 install requests python-snappy prometheus_client --break-system-packages

# 检查Prometheus是否安装
if ! command -v prometheus &> /dev/null; then
    echo "警告: Prometheus 未安装，请确保 Prometheus 服务器正在运行"
else
    echo "Prometheus 已安装"
fi

# 检查Prometheus是否正在运行
echo "检查Prometheus服务器状态..."
if curl -s http://localhost:9090/api/v1/query -d "query=up" | grep -q "success"; then
    echo "✅ Prometheus服务器正在运行"
else
    echo "❌ Prometheus服务器未运行，请启动Prometheus服务器"
    exit 1
fi

# 构建ChronoDB服务器
echo "构建ChronoDB服务器..."
cargo build --bin chronodb-server

# 创建测试配置文件
echo "创建测试配置文件..."
cat > test_config.yaml << EOF
# ChronoDB 测试配置文件
listen_address: "0.0.0.0"
port: 9090
data_dir: "./data"

storage:
  mode: standalone
  backend: local
  local_path: "./data"
  max_disk_usage: "80%"

query:
  max_concurrent: 100
  timeout: 120
  max_samples: 50000000
  enable_vectorized: true
  enable_parallel: true
  enable_auto_downsampling: true
  downsample_policy: "auto"
  query_cache_size: "2GB"
  enable_query_cache: true
  query_cache_ttl: 300

memory:
  memstore_size: "1GB"
  wal_size: "512MB"
  query_cache_size: "1GB"
  max_memory_usage: "80%"

compression:
  time_column:
    algorithm: "zstd"
    level: 3
  value_column:
    algorithm: "zstd"
    level: 3
    use_prediction: true
  label_column:
    algorithm: "dictionary"
    level: 0

retention:
  hot: "24h"
  warm: "168h"
  cold: "720h"
  archive: "8760h"

rules:
  rule_files:
    - "./alert_rules.yaml"
  evaluation_interval: 60
  alert_send_interval: 60

targets:
  config_file: ""
  scrape_interval: 60
  scrape_timeout: 10

remote_write:
  enabled: true
  listen_address: "0.0.0.0:9092"
  max_concurrent: 100
  batch_size: 1000
  timeout: 30

log:
  level: "info"
  format: "text"
  output: ""
EOF

# 创建告警规则文件
echo "创建告警规则文件..."
cat > alert_rules.yaml << EOF
groups:
- name: test_alerts
  rules:
  - alert: HighCPUUsage
    expr: avg(rate(node_cpu_seconds_total{mode="idle"}[5m])) by (instance) < 0.7
    for: 5m
    labels:
      severity: warning
    annotations:
      summary: High CPU usage on {{ $labels.instance }}
      description: CPU usage is above 30% for 5 minutes

  - alert: HighMemoryUsage
    expr: (node_memory_MemTotal_bytes - node_memory_MemAvailable_bytes) / node_memory_MemTotal_bytes > 0.8
    for: 10m
    labels:
      severity: critical
    annotations:
      summary: High memory usage on {{ $labels.instance }}
      description: Memory usage is above 80% for 10 minutes
EOF

# 启动ChronoDB服务器
echo "启动ChronoDB服务器..."
./target/debug/chronodb-server --config test_config.yaml &
CHRONODB_PID=$!

# 等待ChronoDB服务器启动
echo "等待ChronoDB服务器启动..."
sleep 5

# 检查ChronoDB服务器是否正在运行
echo "检查ChronoDB服务器状态..."
if curl -s http://localhost:9090/api/v1/metadata | grep -q "success"; then
    echo "✅ ChronoDB服务器正在运行"
else
    echo "❌ ChronoDB服务器未运行，请检查日志"
    kill $CHRONODB_PID
    exit 1
fi

# 运行测试
echo "开始运行测试..."

# 运行HTTP API v1兼容性测试
echo "\n=== 运行 HTTP API v1 兼容性测试 ==="
python3 test_scripts/test_http_api.py

# 运行Remote Write/Read协议兼容性测试
echo "\n=== 运行 Remote Write/Read 协议兼容性测试 ==="
python3 test_scripts/test_remote_write_read.py

# 运行PromQL查询兼容性测试
echo "\n=== 运行 PromQL 查询兼容性测试 ==="
python3 test_scripts/test_promql.py

# 运行告警规则兼容性测试
echo "\n=== 运行 告警规则 兼容性测试 ==="
python3 test_scripts/test_alert_rules.py

# 运行数据模型兼容性测试
echo "\n=== 运行 数据模型 兼容性测试 ==="
python3 test_scripts/test_data_model.py

# 停止ChronoDB服务器
echo "\n停止ChronoDB服务器..."
kill $CHRONODB_PID
wait $CHRONODB_PID 2>/dev/null

echo "\n=== 测试完成 ==="
echo "测试结果已输出到控制台"
echo "请查看测试报告以获取详细结果"
