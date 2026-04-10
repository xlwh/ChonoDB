# ChronoDB Makefile
# 支持编译测试、性能测试、Python集成测试和跨平台打包

# 默认目标
.DEFAULT_GOAL := help

# 变量定义
CARGO := cargo
PYTHON := python3
TEST_SCRIPTS_DIR := test_scripts
PERF_TEST_DIR := perf-test

# 构建目标
.PHONY: help build test perf-test integration-test clean fmt lint cross-build

# 帮助信息
help:
	@echo "ChronoDB Makefile 目标："
	@echo "  make build          - 构建项目"
	@echo "  make test           - 运行Rust测试"
	@echo "  make perf-test      - 运行性能测试"
	@echo "  make integration-test - 运行Python集成测试"
	@echo "  make clean          - 清理构建产物"
	@echo "  make fmt            - 格式化代码"
	@echo "  make lint           - 运行代码检查"
	@echo "  make cross-build    - 构建跨平台可执行文件"

# 构建项目
build:
	@echo "构建 ChronoDB..."
	@$(CARGO) build --release

# 运行Rust测试
test:
	@echo "运行 Rust 测试..."
	@$(CARGO) test

# 运行性能测试
perf-test:
	@echo "运行性能测试..."
	@cd $(PERF_TEST_DIR) && $(CARGO) run --release

# 运行Python集成测试
integration-test:
	@echo "运行 Python 集成测试..."
	@$(PYTHON) $(TEST_SCRIPTS_DIR)/integration_test.py

# 清理构建产物
clean:
	@echo "清理构建产物..."
	@$(CARGO) clean
	@rm -rf target/

# 格式化代码
fmt:
	@echo "格式化代码..."
	@$(CARGO) fmt

# 运行代码检查
lint:
	@echo "运行代码检查..."
	@$(CARGO) clippy

# 跨平台构建
cross-build:
	@echo "构建跨平台可执行文件..."
	@echo "构建 Linux x86_64..."
	@$(CARGO) build --release --target x86_64-unknown-linux-gnu
	@echo "构建 Windows x86_64..."
	@$(CARGO) build --release --target x86_64-pc-windows-gnu
	@echo "构建 macOS x86_64..."
	@$(CARGO) build --release --target x86_64-apple-darwin
	@echo "构建完成！可执行文件位于 target/{target}/release/ 目录"
