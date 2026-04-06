#!/usr/bin/env python3
"""
检查查询的时间范围
"""

import requests
import time

# 服务器地址
CHRONODB_URL = "http://localhost:9090/api/v1/query"

# 检查查询时间范围
def check_query_time():
    """检查查询的时间范围"""
    print("检查查询的时间范围...")
    
    # 获取当前时间
    now = time.time()
    print(f"当前时间: {now}")
    
    # 测试不同的时间范围
    test_cases = [
        ("当前时间", now),
        ("1分钟前", now - 60),
        ("10分钟前", now - 600),
        ("20分钟前", now - 1200),
        ("30分钟前", now - 1800),
    ]
    
    for name, query_time in test_cases:
        print(f"\n测试: {name} ({query_time})")
        
        # 测试基本查询
        response = requests.post(f"{CHRONODB_URL}?query=http_requests_total&time={query_time}")
        if response.status_code == 200:
            data = response.json()
            if data.get("status") == "success":
                result = data.get("data", {}).get("result", [])
                print(f"  基本查询结果数量: {len(result)}")
        
        # 测试带过滤条件的查询
        response = requests.post(f"{CHRONODB_URL}?query=http_requests_total{{job=\'api\'}}&time={query_time}")
        if response.status_code == 200:
            data = response.json()
            if data.get("status") == "success":
                result = data.get("data", {}).get("result", [])
                print(f"  带过滤条件的查询结果数量: {len(result)}")

if __name__ == "__main__":
    check_query_time()
