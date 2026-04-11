#!/bin/bash
set -e

echo "=== ChronoDB 本地集成测试环境部署 ==="

# 检查二进制文件是否存在
if [ ! -f "../target/release/chronodb-server" ]; then
    echo "错误：chronodb-server 二进制文件不存在，请先构建项目"
    exit 1
fi

if [ ! -f "../target/release/chronodb" ]; then
    echo "错误：chronodb 二进制文件不存在，请先构建项目"
    exit 1
fi

# 清理旧的测试数据
rm -rf ./test-data
mkdir -p ./test-data/chronodb ./test-data/logs

# 复制配置文件
cp -r ../config ./test-data/

# 启动 ChronoDB 服务器
echo "启动 ChronoDB 服务器..."
../target/release/chronodb-server --config ./test-data/config/chronodb.yaml &
SERVER_PID=$!

echo "等待服务器启动..."
sleep 5

# 检查服务器状态
echo "检查服务器状态..."
if ! curl -f http://localhost:9090/-/healthy >/dev/null 2>&1; then
    echo "错误：ChronoDB 服务器启动失败"
    kill $SERVER_PID 2>/dev/null
    exit 1
fi

echo "✓ ChronoDB 服务器已启动"
echo "ChronoDB: http://localhost:9090"
echo ""
echo "=== 环境部署完成 ==="
echo "服务器 PID: $SERVER_PID"
echo "运行测试：./run_local_tests.sh"
echo "停止服务器：kill $SERVER_PID"
