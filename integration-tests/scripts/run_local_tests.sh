#!/bin/bash
set -e

echo "=== 运行 ChronoDB 集成测试 ==="

# 检查 Python 环境
command -v python3 >/dev/null 2>&1 || { echo "需要安装 Python 3"; exit 1; }

# 创建虚拟环境
echo "创建虚拟环境..."
python3 -m venv venv
source venv/bin/activate

# 安装依赖
echo "安装测试依赖..."
pip3 install -r requirements.txt

# 运行测试
echo "运行基础操作测试..."
python3 -m pytest tests/test_basic_operations.py -v

echo "运行查询算子测试..."
python3 -m pytest tests/test_query_operators.py -v

echo "运行资源监控测试..."
python3 -m pytest tests/test_resource_monitoring.py -v

# 生成测试报告
echo "生成测试报告..."
python3 utils/generate_report.py

echo "=== 测试完成 ==="
echo "测试报告已生成：reports/test_report.html"
echo ""
echo "查看报告：open reports/test_report.html"

# 退出虚拟环境
deactivate
