#!/usr/bin/env python3
"""简单的 Docker 测试"""

import subprocess
import time
import requests

def run_cmd(cmd, timeout=60):
    """运行命令"""
    result = subprocess.run(cmd, shell=True, capture_output=True, text=True, timeout=timeout)
    return result.returncode, result.stdout, result.stderr

# 创建网络
print("创建 Docker 网络...")
run_cmd("docker network create test-network 2>/dev/null || true")

# 启动 Prometheus
print("启动 Prometheus...")
rc, stdout, stderr = run_cmd(
    "docker run -d --name prometheus-test --network test-network -p 9090:9090 prom/prometheus:latest",
    timeout=30
)
print(f"Prometheus 启动: rc={rc}, id={stdout.strip()[:12] if stdout else 'None'}")

if rc != 0:
    print(f"错误: {stderr}")
    exit(1)

# 等待服务就绪
print("等待 Prometheus 就绪...")
for i in range(30):
    try:
        response = requests.get("http://localhost:9090/-/healthy", timeout=2)
        if response.status_code == 200:
            print(f"Prometheus 就绪! 响应: {response.text.strip()}")
            break
    except Exception as e:
        print(f"  尝试 {i+1}/30: {e}")
        time.sleep(1)
else:
    print("Prometheus 启动超时")
    run_cmd("docker logs prometheus-test 2>&1 | tail -20")
    run_cmd("docker rm -f prometheus-test")
    run_cmd("docker network rm test-network")
    exit(1)

# 清理
print("清理...")
run_cmd("docker rm -f prometheus-test")
run_cmd("docker network rm test-network")

print("测试完成!")
