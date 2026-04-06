#!/usr/bin/env python3
"""
测试Prometheus和ChronoDB的HTTP API v1兼容性
"""

import requests
import json

# 服务器地址
PROMETHEUS_BASE = "http://localhost:9090"
CHRONODB_BASE = "http://localhost:9090"

# 测试端点列表
ENDPOINTS = [
    "/api/v1/query",
    "/api/v1/query_range",
    "/api/v1/series",
    "/api/v1/labels",
    "/api/v1/metadata",
    "/api/v1/targets",
    "/api/v1/alerts",
    "/api/v1/rules"
]

# 测试每个端点
def test_endpoint(server_name, base_url, endpoint):
    """测试单个API端点"""
    url = f"{base_url}{endpoint}"
    
    # 根据端点类型添加必要的参数
    params = {}
    if endpoint == "/api/v1/query":
        params = {"query": "up"}
    elif endpoint == "/api/v1/query_range":
        import time
        now = int(time.time())
        params = {
            "query": "up",
            "start": now - 3600,
            "end": now,
            "step": "60s"
        }
    elif endpoint == "/api/v1/series":
        params = {"match[]": "up"}
    
    try:
        # 根据服务器和端点选择正确的HTTP方法
        if server_name == "ChronoDB" and (endpoint == "/api/v1/query" or endpoint == "/api/v1/query_range"):
            # ChronoDB使用POST方法
            response = requests.post(url, data=params)
        else:
            # Prometheus使用GET方法
            response = requests.get(url, params=params)
        
        status_code = response.status_code
        if status_code == 200:
            print(f"✅ {server_name} {endpoint}: 成功 ({status_code})")
            # 尝试解析响应
            try:
                data = response.json()
                if data.get("status") == "success":
                    print(f"  响应状态: success")
                else:
                    print(f"  响应状态: {data.get('status')}")
            except:
                print(f"  响应不是有效的JSON")
        else:
            print(f"❌ {server_name} {endpoint}: 失败 ({status_code})")
            print(f"  响应: {response.text}")
    except Exception as e:
        print(f"❌ {server_name} {endpoint}: 错误 - {e}")

# 测试所有端点
def test_all_endpoints():
    """测试所有API端点"""
    print("测试HTTP API v1兼容性...\n")
    
    print("=== 测试Prometheus ===")
    for endpoint in ENDPOINTS:
        test_endpoint("Prometheus", PROMETHEUS_BASE, endpoint)
    
    print("\n=== 测试ChronoDB ===")
    for endpoint in ENDPOINTS:
        test_endpoint("ChronoDB", CHRONODB_BASE, endpoint)

if __name__ == "__main__":
    test_all_endpoints()
    print("\n测试完成！")
