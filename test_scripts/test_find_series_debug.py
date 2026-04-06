#!/usr/bin/env python3
"""
调试 find_series 方法
"""

import requests
import time

# 服务器地址
CHRONODB_URL = "http://localhost:9090/api/v1/query"

# 调试 find_series 方法
def test_find_series_debug():
    """调试 find_series 方法"""
    print("调试 find_series 方法...")
    
    # 获取当前时间
    now = time.time()
    
    # 首先获取所有数据，查看实际的标签值
    print("\n=== 获取所有数据的标签 ===")
    response = requests.post(f"{CHRONODB_URL}?query=http_requests_total&time={now}")
    if response.status_code == 200:
        data = response.json()
        if data.get("status") == "success":
            result = data.get("data", {}).get("result", [])
            print(f"所有数据结果数量: {len(result)}")
            
            if result:
                # 查看第一个结果的详细信息
                first_item = result[0]
                print(f"\n第一个结果的标签: {first_item['metric']}")
                print(f"第一个结果的值: {first_item['value']}")
                
                # 提取标签值
                metric = first_item['metric']
                job = metric.get('job', 'unknown')
                instance = metric.get('instance', 'unknown')
                region = metric.get('region', 'unknown')
                environment = metric.get('environment', 'unknown')
                
                print(f"\n提取的标签值:")
                print(f"  job: {job}")
                print(f"  instance: {instance}")
                print(f"  region: {region}")
                print(f"  environment: {environment}")
                
                # 测试使用这些实际的标签值进行查询
                print("\n=== 使用实际标签值测试 ===")
                test_cases = [
                    ("使用实际的 job", f"http_requests_total{{job='{job}'}}")
                ]
                
                for name, query in test_cases:
                    print(f"\n测试: {name}")
                    print(f"查询: {query}")
                    
                    response = requests.post(f"{CHRONODB_URL}?query={requests.utils.quote(query)}&time={now}")
                    if response.status_code == 200:
                        data = response.json()
                        if data.get("status") == "success":
                            result = data.get("data", {}).get("result", [])
                            print(f"结果数量: {len(result)}")
                            if result:
                                print(f"第一个结果的标签: {result[0]['metric']}")
                        else:
                            print(f"错误: {data.get('error', 'Unknown error')}")
                    else:
                        print(f"状态码: {response.status_code} - {response.text}")
    
    # 测试使用不同的时间范围
    print("\n=== 测试不同的时间范围 ===")
    # 计算数据的时间范围（从之前的测试中我们知道数据时间范围是 1775480640.0 - 1775480760.0）
    data_start = 1775480640.0
    data_end = 1775480760.0
    
    # 测试使用数据的时间范围
    test_cases = [
        ("使用数据的时间范围", data_start, data_end)
    ]
    
    for name, start, end in test_cases:
        print(f"\n测试: {name}")
        print(f"时间范围: {start} - {end}")
        
        # 测试基本查询
        response = requests.get("http://localhost:9090/api/v1/query_range", 
                             params={"query": "http_requests_total", "start": start, "end": end, "step": "15s"})
        if response.status_code == 200:
            data = response.json()
            if data.get("status") == "success":
                result = data.get("data", {}).get("result", [])
                print(f"  基本查询结果数量: {len(result)}")
        
        # 测试带过滤条件的查询
        response = requests.get("http://localhost:9090/api/v1/query_range", 
                             params={"query": "http_requests_total{job='api'}", "start": start, "end": end, "step": "15s"})
        if response.status_code == 200:
            data = response.json()
            if data.get("status") == "success":
                result = data.get("data", {}).get("result", [])
                print(f"  带过滤条件的查询结果数量: {len(result)}")

if __name__ == "__main__":
    test_find_series_debug()
