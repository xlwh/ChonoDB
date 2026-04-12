#!/usr/bin/env python3
"""测试 Prometheus Docker 启动"""

import subprocess
import time
import tempfile
import os
import requests

def run_cmd(cmd, timeout=60):
    """运行命令"""
    result = subprocess.run(cmd, shell=True, capture_output=True, text=True, timeout=timeout)
    return result.returncode, result.stdout, result.stderr

# 创建网络
print("创建 Docker 网络...")
run_cmd("docker network create test-network-2 2>/dev/null || true")

# 创建临时目录
temp_dir = tempfile.mkdtemp(prefix="prometheus_test_")
config_dir = os.path.join(temp_dir, "config")
data_dir = os.path.join(temp_dir, "data")
os.makedirs(config_dir, exist_ok=True)
os.makedirs(data_dir, exist_ok=True)

# 创建配置文件
config_content = """
global:
  scrape_interval: 15s
  evaluation_interval: 15s

scrape_configs:
  - job_name: 'prometheus'
    static_configs:
      - targets: ['localhost:9090']
"""
config_path = os.path.join(config_dir, "prometheus.yml")
with open(config_path, 'w') as f:
    f.write(config_content)

print(f"配置文件路径: {config_path}")
print(f"数据目录: {data_dir}")

# 启动 Prometheus
print("启动 Prometheus...")
cmd = f"""docker run -d \
    --name prometheus-test-2 \
    --network test-network-2 \
    -p 9091:9090 \
    -v {config_path}:/etc/prometheus/prometheus.yml:ro \
    -v {data_dir}:/prometheus \
    prom/prometheus:latest \
    --config.file=/etc/prometheus/prometheus.yml \
    --storage.tsdb.path=/prometheus \
    --web.enable-lifecycle"""

rc, stdout, stderr = run_cmd(cmd, timeout=30)
print(f"Prometheus 启动: rc={rc}, id={stdout.strip()[:12] if stdout else 'None'}")

if rc != 0:
    print(f"错误: {stderr}")
    exit(1)

# 等待服务就绪
print("等待 Prometheus 就绪...")
for i in range(30):
    try:
        response = requests.get("http://localhost:9091/-/healthy", timeout=2)
        if response.status_code == 200:
            print(f"Prometheus 就绪! 响应: {response.text.strip()}")
            break
    except Exception as e:
        print(f"  尝试 {i+1}/30: {type(e).__name__}")
        time.sleep(1)
else:
    print("Prometheus 启动超时")
    run_cmd("docker logs prometheus-test-2 2>&1 | tail -30")
    run_cmd("docker rm -f prometheus-test-2")
    run_cmd("docker network rm test-network-2")
    exit(1)

# 清理
print("清理...")
run_cmd("docker rm -f prometheus-test-2")
run_cmd("docker network rm test-network-2")
import shutil
shutil.rmtree(temp_dir, ignore_errors=True)

print("测试完成!")
