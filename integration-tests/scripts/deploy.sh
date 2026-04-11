#!/bin/bash
set -e

echo "=== ChronoDB 集成测试环境部署 ==="

cd "$(dirname "$0")/.."

command -v docker >/dev/null 2>&1 || { echo "需要安装 Docker"; exit 1; }
command -v docker-compose >/dev/null 2>&1 || { echo "需要安装 Docker Compose"; exit 1; }

export TEST_ENV=true
export COMPOSE_PROJECT_NAME=chronodb-test

echo "构建 ChronoDB 镜像..."
docker-compose -f docker/docker-compose.test.yml build chronodb

echo "构建 prom-agent 镜像..."
docker-compose -f docker/docker-compose.test.yml build chronodb-agent prometheus-agent

echo "启动测试环境..."
docker-compose -f docker/docker-compose.test.yml up -d

echo "等待服务启动..."
sleep 10

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

echo "等待监控 Agent 启动..."
sleep 5

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
