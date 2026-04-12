#!/bin/bash
# PGO (Profile-Guided Optimization) 优化脚本

set -e

echo "=== ChronoDB PGO 优化 ==="

# 步骤 1: 构建带有性能分析支持的版本
echo "[1/4] 构建 PGO 生成版本..."
RUSTFLAGS="-Cprofile-generate=/tmp/pgo-data" \
    cargo build --release --bin chronodb-server

# 步骤 2: 运行工作负载收集性能数据
echo "[2/4] 运行工作负载收集性能数据..."
./target/release/chronodb-server --config config/test.yaml &
SERVER_PID=$!

# 等待服务启动
sleep 5

# 运行测试工作负载
cd integration_tests
python3 run_local_test.py --scale medium --chronodb-url http://localhost:9093 2>&1 | tail -20

# 停止服务
kill $SERVER_PID 2>/dev/null || true
sleep 2

cd ..

# 步骤 3: 合并性能数据
echo "[3/4] 合并性能数据..."
llvm-profdata merge -o /tmp/pgo-data/merged.profdata /tmp/pgo-data

# 步骤 4: 使用性能数据重新构建
echo "[4/4] 使用 PGO 数据重新构建..."
RUSTFLAGS="-Cprofile-use=/tmp/pgo-data/merged.profdata" \
    cargo build --release --bin chronodb-server

echo "=== PGO 优化完成 ==="
echo "优化后的二进制文件: ./target/release/chronodb-server"
