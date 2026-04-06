#!/usr/bin/env python3
"""
生成测试数据并向Prometheus和ChronoDB服务器写入
"""

import time
import random
import requests
import json

# 服务器地址
PROMETHEUS_URL = "http://localhost:9090/api/v1/query"
CHRONODB_URL = "http://localhost:9090/api/v1/query"

# 生成测试数据并通过查询API验证
def test_query_api():
    """测试查询API"""
    # 测试查询
    query = "up"
    
    print("\n测试Prometheus查询API...")
    try:
        response = requests.get(PROMETHEUS_URL, params={"query": query})
        if response.status_code == 200:
            print(f"Prometheus查询成功: {response.json()}")
        else:
            print(f"Prometheus查询失败: {response.status_code} - {response.text}")
    except Exception as e:
        print(f"Prometheus查询时发生错误: {e}")
    
    print("\n测试ChronoDB查询API...")
    try:
        response = requests.get(CHRONODB_URL, params={"query": query})
        if response.status_code == 200:
            print(f"ChronoDB查询成功: {response.json()}")
        else:
            print(f"ChronoDB查询失败: {response.status_code} - {response.text}")
    except Exception as e:
        print(f"ChronoDB查询时发生错误: {e}")

# 测试元数据API
def test_metadata_api():
    """测试元数据API"""
    print("\n测试Prometheus元数据API...")
    try:
        response = requests.get("http://localhost:9090/api/v1/metadata")
        if response.status_code == 200:
            print(f"Prometheus元数据查询成功")
        else:
            print(f"Prometheus元数据查询失败: {response.status_code} - {response.text}")
    except Exception as e:
        print(f"Prometheus元数据查询时发生错误: {e}")
    
    print("\n测试ChronoDB元数据API...")
    try:
        response = requests.get("http://localhost:9090/api/v1/metadata")
        if response.status_code == 200:
            print(f"ChronoDB元数据查询成功")
        else:
            print(f"ChronoDB元数据查询失败: {response.status_code} - {response.text}")
    except Exception as e:
        print(f"ChronoDB元数据查询时发生错误: {e}")

if __name__ == "__main__":
    print("测试HTTP API v1兼容性...")
    test_query_api()
    test_metadata_api()
    print("\n测试完成！")
