#!/usr/bin/env python3
"""
调试 find_series 方法
"""

import requests
import time

# 服务器地址
CHRONODB_URL = "http://localhost:9090/api/v1/query"

# 调试 find_series 方法
def debug_find_series():
    """调试 find_series 方法"""
    print("调试 find_series 方法...")
    
    # 获取当前时间
    now = time.time()
    
    # 测试不同的查询组合
    test_cases = [
        ("只查询 __name__", "http_requests_total"),
        ("只查询 job", "{job='api'}"),
        ("查询 __name__ 和 job", "http_requests_total{job='api'}"),
    ]
    
    for name, query in test_cases:
        print(f"\n测试: {name}")
        print(f"查询: {query}")
        
        try:
            response = requests.post(f"{CHRONODB_URL}?query={requests.utils.quote(query)}&time={now}")
            
            if response.status_code == 200:
                data = response.json()
                if data.get("status") == "success":
                    result = data.get("data", {}).get("result", [])
                    print(f"结果数量: {len(result)}")
                else:
                    print(f"错误: {data.get('error', 'Unknown error')}")
            else:
                print(f"状态码: {response.status_code} - {response.text}")
        except Exception as e:
            print(f"异常: {e}")

if __name__ == "__main__":
    debug_find_series()
