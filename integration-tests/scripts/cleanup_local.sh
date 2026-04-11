#!/bin/bash
set -e

echo "=== 清理 ChronoDB 测试环境 ==="

# 停止 ChronoDB 服务器
if [ -f "./test-data/server.pid" ]; then
    PID=$(cat ./test-data/server.pid)
    if ps -p $PID >/dev/null 2>&1; then
        echo "停止 ChronoDB 服务器..."
        kill $PID
        sleep 2
    fi
    rm ./test-data/server.pid
fi

# 清理测试数据
echo "清理测试数据..."
rm -rf ./test-data

# 清理测试报告
echo "清理测试报告..."
rm -rf ./reports

# 清理 Python 依赖
echo "清理 Python 依赖..."
rm -rf ./venv

# 检查是否有其他 ChronoDB 进程运行
CHRONODB_PIDS=$(ps aux | grep chronodb-server | grep -v grep | awk '{print $2}')
if [ ! -z "$CHRONODB_PIDS" ]; then
    echo "发现其他 ChronoDB 进程，正在停止..."
    for PID in $CHRONODB_PIDS; do
        kill $PID 2>/dev/null
    done
    sleep 2
fi

echo "=== 清理完成 ==="
