#!/usr/bin/env python3
"""
测试Prometheus和ChronoDB的Remote Write/Read协议兼容性
"""

import requests
import time
import random
import snappy
import struct

# 服务器地址
PROMETHEUS_WRITE_URL = "http://localhost:9090/api/v1/write"
CHRONODB_WRITE_URL = "http://localhost:9090/api/v1/write"

# 生成简单的Prometheus文本格式指标
def generate_prometheus_text():
    """生成简单的Prometheus文本格式指标"""
    now = int(time.time())
    value = random.uniform(0, 100)
    return f"test_remote_write{{job=\"test_job\", instance=\"test_instance\"}} {value} {now * 1000}\n"

# 测试Remote Write
def test_remote_write():
    """测试Remote Write协议"""
    print("测试Remote Write协议...")
    
    # 生成Prometheus文本格式指标
    metrics = generate_prometheus_text()
    print(f"生成的指标: {metrics.strip()}")
    
    # 压缩数据
    compressed_metrics = snappy.compress(metrics.encode('utf-8'))
    
    # 测试向Prometheus写入
    print("\n向Prometheus写入数据...")
    try:
        response = requests.post(PROMETHEUS_WRITE_URL, data=compressed_metrics, headers={"Content-Type": "application/x-snappy"})
        if response.status_code == 204:
            print("✅ 成功向Prometheus写入数据")
        else:
            print(f"❌ 向Prometheus写入数据失败: {response.status_code} - {response.text}")
    except Exception as e:
        print(f"❌ 向Prometheus写入数据时发生错误: {e}")
    
    # 测试向ChronoDB写入
    print("\n向ChronoDB写入数据...")
    try:
        response = requests.post(CHRONODB_WRITE_URL, data=compressed_metrics, headers={"Content-Type": "application/x-snappy"})
        if response.status_code == 204:
            print("✅ 成功向ChronoDB写入数据")
        else:
            print(f"❌ 向ChronoDB写入数据失败: {response.status_code} - {response.text}")
    except Exception as e:
        print(f"❌ 向ChronoDB写入数据时发生错误: {e}")

# 测试查询API，验证数据是否写入成功
def test_query_after_write():
    """测试查询API，验证数据是否写入成功"""
    print("\n测试查询API，验证数据是否写入成功...")
    
    # 测试查询Prometheus
    print("\n查询Prometheus...")
    try:
        response = requests.get("http://localhost:9090/api/v1/query", params={"query": "test_remote_write"})
        if response.status_code == 200:
            data = response.json()
            if data.get("status") == "success" and data.get("data", {}).get("result"):
                print("✅ Prometheus查询成功，数据已写入")
                print(f"  结果: {data['data']['result']}")
            else:
                print("❌ Prometheus查询成功，但没有返回数据")
        else:
            print(f"❌ Prometheus查询失败: {response.status_code} - {response.text}")
    except Exception as e:
        print(f"❌ Prometheus查询时发生错误: {e}")
    
    # 测试查询ChronoDB
    print("\n查询ChronoDB...")
    try:
        response = requests.post("http://localhost:9090/api/v1/query", data={"query": "test_remote_write"})
        if response.status_code == 200:
            data = response.json()
            if data.get("status") == "success" and data.get("data", {}).get("result"):
                print("✅ ChronoDB查询成功，数据已写入")
                print(f"  结果: {data['data']['result']}")
            else:
                print("❌ ChronoDB查询成功，但没有返回数据")
        else:
            print(f"❌ ChronoDB查询失败: {response.status_code} - {response.text}")
    except Exception as e:
        print(f"❌ ChronoDB查询时发生错误: {e}")

if __name__ == "__main__":
    test_remote_write()
    test_query_after_write()
    print("\n测试完成！")
