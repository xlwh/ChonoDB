#!/bin/bash

echo "=== 清理测试环境 ==="

cd "$(dirname "$0")/.."

echo "停止服务..."
docker-compose -f docker/docker-compose.test.yml down

echo "删除数据卷..."
docker-compose -f docker/docker-compose.test.yml down -v

echo "删除测试镜像..."
docker rmi chronodb-test_chronodb 2>/dev/null || true
docker rmi chronodb-test_chronodb-agent 2>/dev/null || true
docker rmi chronodb-test_prometheus-agent 2>/dev/null || true

echo "清理临时文件..."
rm -rf reports/*.json
rm -rf reports/*.html

echo "=== 清理完成 ==="
