#!/bin/bash

set -e

echo "Starting regression tests..."
echo "================================"

# 运行存储模块的测试
echo "Running storage module tests..."
echo "--------------------------------"
cargo test --package chronodb-storage

# 运行服务器模块的测试
echo "\nRunning server module tests..."
echo "--------------------------------"
cargo test --package chronodb-server

# 运行 CLI 模块的测试
echo "\nRunning CLI module tests..."
echo "--------------------------------"
cargo test --package chronodb-cli

# 运行集成测试
echo "\nRunning integration tests..."
echo "--------------------------------"
cargo test --package chronodb-storage --test integration_tests
cargo test --package chronodb-storage --test storage_integration_test
cargo test --package chronodb-storage --test backup_integration_test
cargo test --package chronodb-storage --test fault_injection_test

# 运行基准测试
echo "\nRunning benchmark tests..."
echo "--------------------------------"
cargo bench --package chronodb-storage

echo "\n================================"
echo "Regression tests completed successfully!"
