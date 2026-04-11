#!/bin/bash
set -e

echo "=== 运行集成测试 ==="

cd "$(dirname "$0")/.."

if [ ! -d "venv" ]; then
    echo "创建 Python 虚拟环境..."
    python3 -m venv venv
fi

echo "激活虚拟环境..."
source venv/bin/activate

echo "安装 Python 依赖..."
pip install -r requirements.txt

echo "运行测试..."
pytest tests/ \
    --verbose \
    --tb=short \
    --html=reports/test-report.html \
    --self-contained-html \
    --junitxml=reports/junit.xml

echo "生成测试报告..."
python scripts/generate_report.py

echo "=== 测试完成 ==="
echo "测试报告: reports/test-report.html"
