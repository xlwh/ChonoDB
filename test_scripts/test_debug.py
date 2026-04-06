#!/usr/bin/env python3
"""
调试测试 - 检查数据写入和查询的详细信息
"""

import requests
import time

CHRONODB_URL = "http://localhost:9090"

def test_write_and_query():
    """测试写入和查询"""
    print("=== 调试测试 ===")

    # 生成时间戳
    now_ms = int(time.time() * 1000)
    print(f"当前时间戳: {now_ms}")
    print(f"查询时间范围: {now_ms - 3600000} 到 {now_ms}")

    # 生成测试数据 - 使用时间戳在查询范围内
    metric = f'test_debug{{job="test", instance="test"}} 123.45 {now_ms}'
    print(f"写入数据: {metric}")

    # 写入数据
    response = requests.post(f"{CHRONODB_URL}/api/v1/write", data=metric.encode('utf-8'), headers={"Content-Type": "text/plain"})
    print(f"写入响应: {response.status_code}")

    # 等待一小段时间
    time.sleep(0.5)

    # 查询标签名称
    print("\n=== 查询标签名称 ===")
    response = requests.get(f"{CHRONODB_URL}/api/v1/labels")
    print(f"标签响应: {response.status_code}")
    if response.status_code == 200:
        print(f"标签: {response.json()}")

    # 查询指标名称
    print("\n=== 查询指标名称 ===")
    response = requests.get(f"{CHRONODB_URL}/api/v1/label/__name__/values")
    print(f"指标响应: {response.status_code}")
    if response.status_code == 200:
        print(f"指标: {response.json()}")

    # 查询系列
    print("\n=== 查询系列 ===")
    response = requests.get(f"{CHRONODB_URL}/api/v1/series", params={
        "match[]": "test_debug",
        "start": now_ms - 3600000,
        "end": now_ms + 60000
    })
    print(f"系列响应: {response.status_code}")
    if response.status_code == 200:
        print(f"系列: {response.json()}")

    # 查询数据
    print("\n=== 查询数据 ===")
    response = requests.get(f"{CHRONODB_URL}/api/v1/query", params={
        "query": "test_debug",
        "time": (now_ms + 1000) / 1000.0  # 转换为秒
    })
    print(f"查询响应: {response.status_code}")
    if response.status_code == 200:
        print(f"查询结果: {response.json()}")

if __name__ == "__main__":
    test_write_and_query()
