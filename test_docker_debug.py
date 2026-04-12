#!/usr/bin/env python3
"""调试 Docker 测试"""

import subprocess
import time
import tempfile
import os
import requests
import shutil

def run_cmd(cmd, timeout=60):
    """运行命令"""
    result = subprocess.run(cmd, shell=True, capture_output=True, text=True, timeout=timeout)
    return result.returncode, result.stdout, result.stderr

# 清理之前的容器
run_cmd("docker rm -f prometheus-debug 2>/dev/null || true")
run_cmd("docker network rm debug-network 2>/dev/null || true")

# 创建网络
print("创建 Docker 网络...")
rc, stdout, stderr = run_cmd("docker network create debug-network")
print(f"网络创建: rc={rc}, id={stdout.strip()[:12] if stdout else 'None'}")

# 创建临时目录
temp_dir = tempfile.mkdtemp(prefix="prometheus_debug_")
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

# 启动 Prometheus - 使用与测试脚本相同的参数
print("\n启动 Prometheus (使用测试脚本相同的参数)...")
cmd_list = [
    "docker", "run", "-d",
    "--name", "prometheus-debug",
    "--network", "debug-network",
    "-p", "9092:9090",
    "-v", f"{config_path}:/etc/prometheus/prometheus.yml:ro",
    "-v", f"{data_dir}:/prometheus",
    "--user", "root",
    "prom/prometheus:latest",
    "--config.file=/etc/prometheus/prometheus.yml",
    "--storage.tsdb.path=/prometheus",
    "--web.enable-lifecycle"
]

print(f"命令: {' '.join(cmd_list)}")
result = subprocess.run(cmd_list, capture_output=True, text=True)
print(f"启动结果: rc={result.returncode}")
print(f"stdout: {result.stdout.strip()[:50] if result.stdout else 'None'}")
print(f"stderr: {result.stderr.strip()[:200] if result.stderr else 'None'}")

if result.returncode != 0:
    print("启动失败!")
    shutil.rmtree(temp_dir, ignore_errors=True)
    exit(1)

container_id = result.stdout.strip()
print(f"容器ID: {container_id[:12]}")

# 等待几秒让容器启动
print("\n等待 5 秒让容器启动...")
time.sleep(5)

# 检查容器状态
print("\n检查容器状态...")
rc, stdout, stderr = run_cmd("docker ps -a | grep prometheus-debug")
print(f"容器状态: {stdout}")

# 检查容器日志
print("\n容器日志:")
rc, stdout, stderr = run_cmd("docker logs prometheus-debug 2>&1 | tail -30")
print(stdout)

# 尝试健康检查
print("\n尝试健康检查...")
for i in range(10):
    try:
        response = requests.get("http://localhost:9092/-/healthy", timeout=2)
        print(f"健康检查成功! 响应: {response.text.strip()}")
        break
    except Exception as e:
        print(f"  尝试 {i+1}/10: {type(e).__name__}: {e}")
        time.sleep(1)
else:
    print("健康检查失败!")

# 清理
print("\n清理...")
run_cmd("docker rm -f prometheus-debug")
run_cmd("docker network rm debug-network")
shutil.rmtree(temp_dir, ignore_errors=True)

print("调试完成!")
