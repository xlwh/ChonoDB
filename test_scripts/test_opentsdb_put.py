#!/usr/bin/env python3
"""
ChronoDB OpenTSDB 协议集成测试
测试 /api/put 端点的 OpenTSDB 兼容写入功能
"""

import requests
import json
import time
import subprocess
import os
import sys
import tempfile
import shutil

BASE_URL = "http://localhost:9090"
OPENTSDB_PUT_URL = f"{BASE_URL}/api/put"

GREEN = '\033[92m'
RED = '\033[91m'
YELLOW = '\033[93m'
RESET = '\033[0m'

passed = 0
failed = 0

def log_pass(msg):
    global passed
    passed += 1
    print(f"  {GREEN}✓ PASS{RESET} {msg}")

def log_fail(msg):
    global failed
    failed += 1
    print(f"  {RED}✗ FAIL{RESET} {msg}")

def log_section(msg):
    print(f"\n{'='*60}")
    print(f"  {msg}")
    print(f"{'='*60}")

server_process = None
temp_dir = None

def start_server():
    global server_process, temp_dir
    log_section("启动 ChronoDB 服务器")
    temp_dir = tempfile.mkdtemp(prefix="chronodb_opentsdb_")

    config_content = f"""
listen_address: "0.0.0.0"
port: 9090
data_dir: "{temp_dir}"
storage:
  mode: "standalone"
  backend: "local"
  local_path: "{temp_dir}/data"
  max_disk_usage: "90%"
query:
  max_concurrent: 100
  timeout: 120
  max_samples: 50000000
  enable_vectorized: true
  enable_parallel: true
  enable_auto_downsampling: true
  downsample_policy: "auto"
  query_cache_size: "512MB"
  enable_query_cache: true
  query_cache_ttl: 300
rules:
  rule_files: []
  evaluation_interval: 60
  alert_send_interval: 60
targets:
  scrape_interval: 60
  scrape_timeout: 10
memory:
  memstore_size: "1GB"
  wal_size: "256MB"
  query_cache_size: "512MB"
  max_memory_usage: "80%"
compression:
  time_column:
    algorithm: "zstd"
    level: 3
  value_column:
    algorithm: "zstd"
    level: 3
    use_prediction: true
  label_column:
    algorithm: "dictionary"
    level: 0
log:
  level: "info"
  format: "text"
"""
    config_path = os.path.join(temp_dir, "config.yaml")
    with open(config_path, 'w') as f:
        f.write(config_content)

    log_info("编译服务器...")
    result = subprocess.run(
        ["cargo", "build", "--release", "--bin", "chronodb-server"],
        cwd="/home/zhb/workspace/chonodb",
        capture_output=True, text=True
    )
    if result.returncode != 0:
        log_fail(f"编译失败: {result.stderr[-300:]}")
        return False

    log_info("启动服务器...")
    log_file = open(os.path.join(temp_dir, "server.log"), 'w')
    server_process = subprocess.Popen(
        ["./target/release/chronodb-server", "--config", config_path],
        cwd="/home/zhb/workspace/chonodb",
        stdout=log_file, stderr=log_file, text=True
    )

    for i in range(30):
        time.sleep(1)
        try:
            resp = requests.get(f"{BASE_URL}/-/healthy", timeout=2)
            if resp.status_code == 200:
                log_pass(f"服务器启动成功 (耗时 {i+1}s)")
                return True
        except:
            pass
        if server_process.poll() is not None:
            log_fail("服务器启动失败")
            return False

    log_fail("服务器启动超时")
    return False

def stop_server():
    global server_process, temp_dir
    if server_process:
        server_process.terminate()
        try:
            server_process.wait(timeout=5)
        except:
            server_process.kill()
    if temp_dir and os.path.exists(temp_dir):
        shutil.rmtree(temp_dir, ignore_errors=True)

def log_info(msg):
    print(f"  {YELLOW}ℹ{RESET} {msg}")


def test_single_put():
    log_section("测试 1: 单条数据写入 (OpenTSDB 格式)")
    ts = int(time.time())
    data = {
        "metric": "sys.cpu.nice",
        "timestamp": ts,
        "value": 18.5,
        "tags": {
            "host": "web01",
            "dc": "lga"
        }
    }

    resp = requests.post(OPENTSDB_PUT_URL, json=data, timeout=5)
    log_pass(f"单条写入返回 {resp.status_code}") if resp.status_code == 204 else log_fail(f"单条写入返回 {resp.status_code}: {resp.text}")

    time.sleep(0.5)

    resp2 = requests.get(f"{BASE_URL}/api/v1/query", params={"query": "sys.cpu.nice"}, timeout=5)
    if resp2.status_code == 200:
        result = resp2.json()
        results = result.get("data", {}).get("result", [])
        if len(results) > 0:
            val = float(results[0]["value"][1])
            log_pass(f"查询返回值 {val:.1f} (预期 18.5)") if abs(val - 18.5) < 0.1 else log_fail(f"查询值 {val} != 18.5")
        else:
            log_fail("查询返回空结果")
    else:
        log_fail(f"查询失败: {resp2.status_code}")


def test_single_put_millisecond_timestamp():
    log_section("测试 2: 毫秒级时间戳写入")
    ts_ms = int(time.time() * 1000)
    data = {
        "metric": "sys.cpu.idle",
        "timestamp": ts_ms,
        "value": 75.3,
        "tags": {
            "host": "web02",
            "dc": "lga"
        }
    }

    resp = requests.post(OPENTSDB_PUT_URL, json=data, timeout=5)
    log_pass(f"毫秒级时间戳写入返回 {resp.status_code}") if resp.status_code == 204 else log_fail(f"毫秒级时间戳写入返回 {resp.status_code}: {resp.text}")

    time.sleep(0.5)

    resp2 = requests.get(f"{BASE_URL}/api/v1/query", params={"query": "sys.cpu.idle"}, timeout=5)
    if resp2.status_code == 200:
        result = resp2.json()
        results = result.get("data", {}).get("result", [])
        if len(results) > 0:
            log_pass("毫秒级时间戳数据查询成功")
        else:
            log_fail("毫秒级时间戳数据查询返回空结果")
    else:
        log_fail(f"查询失败: {resp2.status_code}")


def test_batch_put():
    log_section("测试 3: 批量数据写入 (OpenTSDB 数组格式)")
    ts = int(time.time())
    data = [
        {
            "metric": "sys.cpu.user",
            "timestamp": ts,
            "value": 42.1,
            "tags": {"host": "web01", "dc": "lga"}
        },
        {
            "metric": "sys.cpu.user",
            "timestamp": ts,
            "value": 38.7,
            "tags": {"host": "web02", "dc": "lga"}
        },
        {
            "metric": "sys.cpu.system",
            "timestamp": ts,
            "value": 5.2,
            "tags": {"host": "web01", "dc": "lga"}
        },
        {
            "metric": "sys.mem.used",
            "timestamp": ts,
            "value": 8192,
            "tags": {"host": "web01", "dc": "lga"}
        }
    ]

    resp = requests.post(OPENTSDB_PUT_URL, json=data, timeout=5)
    log_pass(f"批量写入 4 条数据返回 {resp.status_code}") if resp.status_code == 204 else log_fail(f"批量写入返回 {resp.status_code}: {resp.text}")

    time.sleep(0.5)

    resp2 = requests.get(f"{BASE_URL}/api/v1/query", params={"query": "sys.cpu.user"}, timeout=5)
    if resp2.status_code == 200:
        result = resp2.json()
        results = result.get("data", {}).get("result", [])
        log_pass(f"批量写入查询返回 {len(results)} 个系列") if len(results) >= 2 else log_fail(f"批量写入查询仅返回 {len(results)} 个系列")
    else:
        log_fail(f"批量写入查询失败: {resp2.status_code}")


def test_put_with_summary():
    log_section("测试 4: 带 summary 参数的写入")
    ts = int(time.time())
    data = {
        "metric": "sys.disk.read",
        "timestamp": ts,
        "value": 1024.5,
        "tags": {"host": "web01", "disk": "sda"}
    }

    resp = requests.post(f"{OPENTSDB_PUT_URL}?summary", json=data, timeout=5)
    if resp.status_code == 200:
        result = resp.json()
        if "failed" in result and "success" in result:
            log_pass(f"summary 返回: success={result['success']}, failed={result['failed']}")
        else:
            log_fail(f"summary 格式不正确: {result}")
    else:
        log_fail(f"summary 请求返回 {resp.status_code}: {resp.text}")


def test_put_with_details():
    log_section("测试 5: 带 details 参数的写入")
    ts = int(time.time())
    data = [
        {
            "metric": "sys.net.bytes_in",
            "timestamp": ts,
            "value": 5678.9,
            "tags": {"host": "web01", "interface": "eth0"}
        },
        {
            "metric": "sys.net.bytes_out",
            "timestamp": ts,
            "value": 1234.5,
            "tags": {"host": "web01", "interface": "eth0"}
        }
    ]

    resp = requests.post(f"{OPENTSDB_PUT_URL}?details", json=data, timeout=5)
    if resp.status_code == 200:
        result = resp.json()
        if "failed" in result and "success" in result and "errors" in result:
            log_pass(f"details 返回: success={result['success']}, failed={result['failed']}, errors={len(result['errors'])}")
        else:
            log_fail(f"details 格式不正确: {result}")
    else:
        log_fail(f"details 请求返回 {resp.status_code}: {resp.text}")


def test_put_invalid_data():
    log_section("测试 6: 无效数据处理")
    resp = requests.post(OPENTSDB_PUT_URL, data="not json", timeout=5,
                         headers={"Content-Type": "application/json"})
    log_pass(f"无效 JSON 返回 {resp.status_code}") if resp.status_code == 400 else log_fail(f"无效 JSON 返回 {resp.status_code}")

    data_no_tags = {
        "metric": "test.no_tags",
        "timestamp": int(time.time()),
        "value": 1.0,
        "tags": {}
    }
    resp = requests.post(OPENTSDB_PUT_URL, json=data_no_tags, timeout=5)
    log_pass(f"无标签数据返回 {resp.status_code}") if resp.status_code == 400 else log_fail(f"无标签数据返回 {resp.status_code}")

    data_no_metric = {
        "timestamp": int(time.time()),
        "value": 1.0,
        "tags": {"host": "test"}
    }
    resp = requests.post(OPENTSDB_PUT_URL, json=data_no_metric, timeout=5)
    log_pass(f"无 metric 数据返回 {resp.status_code}") if resp.status_code in [400, 422] else log_fail(f"无 metric 数据返回 {resp.status_code}")


def test_put_string_value():
    log_section("测试 7: 字符串格式的数值")
    ts = int(time.time())
    data = {
        "metric": "sys.cpu.guest",
        "timestamp": ts,
        "value": "3.14",
        "tags": {"host": "web01", "dc": "lga"}
    }

    resp = requests.post(OPENTSDB_PUT_URL, json=data, timeout=5)
    log_pass(f"字符串值写入返回 {resp.status_code}") if resp.status_code == 204 else log_fail(f"字符串值写入返回 {resp.status_code}: {resp.text}")

    time.sleep(0.5)

    resp2 = requests.get(f"{BASE_URL}/api/v1/query", params={"query": "sys.cpu.guest"}, timeout=5)
    if resp2.status_code == 200:
        result = resp2.json()
        results = result.get("data", {}).get("result", [])
        if len(results) > 0:
            val = float(results[0]["value"][1])
            log_pass(f"字符串值查询返回 {val:.2f}") if abs(val - 3.14) < 0.01 else log_fail(f"字符串值 {val} != 3.14")
        else:
            log_fail("字符串值查询返回空结果")
    else:
        log_fail(f"字符串值查询失败: {resp2.status_code}")


def test_put_integer_value():
    log_section("测试 8: 整数值写入")
    ts = int(time.time())
    data = {
        "metric": "sys.process.count",
        "timestamp": ts,
        "value": 256,
        "tags": {"host": "web01"}
    }

    resp = requests.post(OPENTSDB_PUT_URL, json=data, timeout=5)
    log_pass(f"整数值写入返回 {resp.status_code}") if resp.status_code == 204 else log_fail(f"整数值写入返回 {resp.status_code}: {resp.text}")

    time.sleep(0.5)

    resp2 = requests.get(f"{BASE_URL}/api/v1/query", params={"query": "sys.process.count"}, timeout=5)
    if resp2.status_code == 200:
        result = resp2.json()
        results = result.get("data", {}).get("result", [])
        if len(results) > 0:
            val = float(results[0]["value"][1])
            log_pass(f"整数值查询返回 {val:.0f}") if abs(val - 256) < 1 else log_fail(f"整数值 {val} != 256")
        else:
            log_fail("整数值查询返回空结果")
    else:
        log_fail(f"整数值查询失败: {resp2.status_code}")


def test_large_batch_put():
    log_section("测试 9: 大批量 OpenTSDB 写入")
    ts = int(time.time())
    data = []
    for i in range(100):
        data.append({
            "metric": "load.avg",
            "timestamp": ts - i * 60,
            "value": round(0.5 + i * 0.01, 2),
            "tags": {"host": f"server-{i % 5}", "group": "production"}
        })

    resp = requests.post(OPENTSDB_PUT_URL, json=data, timeout=10)
    log_pass(f"100 条批量写入返回 {resp.status_code}") if resp.status_code == 204 else log_fail(f"100 条批量写入返回 {resp.status_code}: {resp.text}")

    time.sleep(1)

    resp2 = requests.get(f"{BASE_URL}/api/v1/query", params={"query": "load.avg"}, timeout=5)
    if resp2.status_code == 200:
        result = resp2.json()
        results = result.get("data", {}).get("result", [])
        log_pass(f"大批量写入查询返回 {len(results)} 个系列") if len(results) > 0 else log_fail("大批量写入查询返回空结果")
    else:
        log_fail(f"大批量写入查询失败: {resp2.status_code}")


def test_query_after_opentsdb_put():
    log_section("测试 10: OpenTSDB 写入后用 Prometheus API 查询")
    ts = int(time.time())
    data = {
        "metric": "http.requests.total",
        "timestamp": ts,
        "value": 1024,
        "tags": {"method": "GET", "status": "200", "service": "api"}
    }

    resp = requests.post(OPENTSDB_PUT_URL, json=data, timeout=5)
    if resp.status_code not in [200, 204]:
        log_fail(f"写入失败: {resp.status_code}")
        return

    time.sleep(0.5)

    resp2 = requests.get(
        f"{BASE_URL}/api/v1/labels",
        timeout=5
    )
    if resp2.status_code == 200:
        labels = resp2.json().get("data", [])
        log_pass(f"标签查询返回 {len(labels)} 个标签") if "method" in labels or "status" in labels else log_fail(f"标签查询缺少 OpenTSDB 写入的标签: {labels}")
    else:
        log_fail(f"标签查询失败: {resp2.status_code}")

    resp3 = requests.get(
        f"{BASE_URL}/api/v1/label/__name__/values",
        timeout=5
    )
    if resp3.status_code == 200:
        metrics = resp3.json().get("data", [])
        log_pass(f"指标名查询返回 {len(metrics)} 个指标") if len(metrics) > 0 else log_fail("指标名查询返回空结果")
    else:
        log_fail(f"指标名查询失败: {resp3.status_code}")


if __name__ == "__main__":
    print(f"\n{'#'*60}")
    print(f"  ChronoDB OpenTSDB 协议集成测试")
    print(f"  时间: {time.strftime('%Y-%m-%d %H:%M:%S')}")
    print(f"{'#'*60}")

    if not start_server():
        print("服务器启动失败，终止测试")
        sys.exit(1)

    try:
        time.sleep(1)
        test_single_put()
        test_single_put_millisecond_timestamp()
        test_batch_put()
        test_put_with_summary()
        test_put_with_details()
        test_put_invalid_data()
        test_put_string_value()
        test_put_integer_value()
        test_large_batch_put()
        test_query_after_opentsdb_put()
    finally:
        stop_server()

    log_section("测试结果汇总")
    total = passed + failed
    print(f"  通过: {GREEN}{passed}{RESET}")
    print(f"  失败: {RED}{failed}{RESET}")
    print(f"  总计: {total}")
    if total > 0:
        print(f"  通过率: {passed/total*100:.1f}%")
    print()
    if failed == 0:
        print(f"  {GREEN}🎉 所有测试通过！{RESET}")
    else:
        print(f"  {RED}❌ 有 {failed} 个测试失败{RESET}")
    sys.exit(0 if failed == 0 else 1)
