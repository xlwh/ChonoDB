#!/usr/bin/env python3
"""
检查数据是否正确写入
"""

import requests
import time

# 服务器地址
CHRONODB_URL = "http://localhost:9090/api/v1/query"

# 检查数据
def check_data():
    """检查数据是否正确写入"""
    print("检查数据是否正确写入...")
    
    # 获取当前时间
    now = time.time()
    
    # 测试查询 - 使用不同的时间范围
    test_queries = [
        "http_requests_total",
        "cpu_usage_percent",
        "memory_usage_bytes",
        "http_requests_total{job='frontend'}",
        "sum(http_requests_total)",
    ]
    
    for query in test_queries:
        print(f"\n测试查询: {query}")
        
        # 尝试不同的时间参数
        for time_offset in [0, 3600, 7200]:
            query_time = now - time_offset
            
            try:
                response = requests.post(f"{CHRONODB_URL}?query={requests.utils.quote(query)}&time={query_time}")
                
                if response.status_code == 200:
                    data = response.json()
                    if data.get("status") == "success":
                        result_count = len(data.get("data", {}).get("result", []))
                        print(f"  时间: {query_time:.0f} - 结果数量: {result_count}")
                        if result_count > 0:
                            print(f"  第一个结果: {data['data']['result'][0]}")
                    else:
                        print(f"  错误: {data.get('error', 'Unknown error')}")
                else:
                    print(f"  状态码: {response.status_code} - {response.text}")
            except Exception as e:
                print(f"  异常: {e}")

if __name__ == "__main__":
    check_data()
