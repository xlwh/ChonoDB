#!/bin/bash

set -e

echo "========================================="
echo "ChronoDB 性能测试"
echo "========================================="
echo ""

echo "1. 编译项目..."
cargo build --release

echo ""
echo "2. 运行性能基准测试..."
cargo bench --no-run

echo ""
echo "3. 运行性能测试..."
cargo test --release --test write_performance_test -- --nocapture
cargo test --release --test query_performance_test -- --nocapture

echo ""
echo "4. 运行压力测试..."
echo "   - 写入性能测试..."
cargo test --release --test write_performance_test test_write_throughput -- --nocapture

echo "   - 查询性能测试..."
cargo test --release --test query_performance_test test_query_latency -- --nocapture

echo ""
echo "========================================="
echo "性能测试完成！"
echo "========================================="
