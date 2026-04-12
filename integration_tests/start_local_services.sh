#!/bin/bash
# 启动本地 Prometheus 和 ChronoDB 服务用于测试

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
PROMETHEUS_DIR="/Users/zhb/workspace/prometheus"

echo "=========================================="
echo "启动本地测试服务"
echo "=========================================="

# 检查端口是否被占用
check_port() {
    local port=$1
    if lsof -i :$port > /dev/null 2>&1; then
        echo "警告: 端口 $port 已被占用"
        return 1
    fi
    return 0
}

# 启动 Prometheus
start_prometheus() {
    echo ""
    echo "启动 Prometheus..."
    
    if [ ! -d "$PROMETHEUS_DIR" ]; then
        echo "错误: Prometheus 目录不存在: $PROMETHEUS_DIR"
        return 1
    fi
    
    check_port 9090 || return 1
    
    cd "$PROMETHEUS_DIR"
    
    # 创建简单的配置文件
    cat > /tmp/prometheus_test.yml << 'EOF'
global:
  scrape_interval: 15s
  evaluation_interval: 15s

scrape_configs:
  - job_name: 'prometheus'
    static_configs:
      - targets: ['localhost:9090']
  
  - job_name: 'chronodb'
    static_configs:
      - targets: ['localhost:9091']
EOF
    
    # 启动 Prometheus
    if [ -f "./prometheus" ]; then
        ./prometheus --config.file=/tmp/prometheus_test.yml --storage.tsdb.path=/tmp/prometheus_data &
        PROMETHEUS_PID=$!
        echo "Prometheus 已启动 (PID: $PROMETHEUS_PID)"
    elif command -v prometheus &> /dev/null; then
        prometheus --config.file=/tmp/prometheus_test.yml --storage.tsdb.path=/tmp/prometheus_data &
        PROMETHEUS_PID=$!
        echo "Prometheus 已启动 (PID: $PROMETHEUS_PID)"
    else
        echo "错误: 未找到 Prometheus 可执行文件"
        return 1
    fi
    
    # 等待服务就绪
    echo "等待 Prometheus 就绪..."
    for i in {1..30}; do
        if curl -s http://localhost:9090/-/healthy > /dev/null 2>&1; then
            echo "Prometheus 服务已就绪"
            return 0
        fi
        sleep 1
    done
    
    echo "错误: Prometheus 启动超时"
    return 1
}

# 启动 ChronoDB
start_chronodb() {
    echo ""
    echo "启动 ChronoDB..."
    
    check_port 9091 || return 1
    
    cd "$PROJECT_DIR"
    
    # 检查是否有预构建的二进制文件
    if [ -f "./target/release/chronodb-server" ]; then
        ./target/release/chronodb-server --config.file=./config/chronodb.yaml &
        CHRONODB_PID=$!
        echo "ChronoDB 已启动 (PID: $CHRONODB_PID)"
    elif [ -f "./target/debug/chronodb-server" ]; then
        ./target/debug/chronodb-server --config.file=./config/chronodb.yaml &
        CHRONODB_PID=$!
        echo "ChronoDB 已启动 (PID: $CHRONODB_PID)"
    else
        echo "错误: 未找到 ChronoDB 可执行文件"
        echo "请先构建 ChronoDB: cargo build --release"
        return 1
    fi
    
    # 等待服务就绪
    echo "等待 ChronoDB 就绪..."
    for i in {1..30}; do
        if curl -s http://localhost:9091/-/healthy > /dev/null 2>&1; then
            echo "ChronoDB 服务已就绪"
            return 0
        fi
        sleep 1
    done
    
    echo "错误: ChronoDB 启动超时"
    return 1
}

# 停止服务
stop_services() {
    echo ""
    echo "停止服务..."
    
    if [ -n "$PROMETHEUS_PID" ]; then
        kill $PROMETHEUS_PID 2>/dev/null || true
        echo "Prometheus 已停止"
    fi
    
    if [ -n "$CHRONODB_PID" ]; then
        kill $CHRONODB_PID 2>/dev/null || true
        echo "ChronoDB 已停止"
    fi
}

# 注册清理函数
trap stop_services EXIT INT TERM

# 主函数
main() {
    # 解析参数
    START_PROMETHEUS=false
    START_CHRONODB=false
    
    while [[ $# -gt 0 ]]; do
        case $1 in
            --prometheus)
                START_PROMETHEUS=true
                shift
                ;;
            --chronodb)
                START_CHRONODB=true
                shift
                ;;
            --all)
                START_PROMETHEUS=true
                START_CHRONODB=true
                shift
                ;;
            --help)
                echo "用法: $0 [选项]"
                echo ""
                echo "选项:"
                echo "  --prometheus    只启动 Prometheus"
                echo "  --chronodb      只启动 ChronoDB"
                echo "  --all           启动所有服务 (默认)"
                echo "  --help          显示帮助"
                exit 0
                ;;
            *)
                echo "未知选项: $1"
                exit 1
                ;;
        esac
    done
    
    # 默认启动所有
    if [ "$START_PROMETHEUS" = false ] && [ "$START_CHRONODB" = false ]; then
        START_PROMETHEUS=true
        START_CHRONODB=true
    fi
    
    # 启动服务
    if [ "$START_PROMETHEUS" = true ]; then
        start_prometheus || exit 1
    fi
    
    if [ "$START_CHRONODB" = true ]; then
        start_chronodb || exit 1
    fi
    
    echo ""
    echo "=========================================="
    echo "所有服务已启动"
    echo "=========================================="
    echo "Prometheus: http://localhost:9090"
    echo "ChronoDB:   http://localhost:9091"
    echo ""
    echo "按 Ctrl+C 停止服务"
    
    # 保持运行
    wait
}

main "$@"
