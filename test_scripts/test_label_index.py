#!/usr/bin/env python3
"""
测试标签索引
"""

import requests
import time

# 服务器地址
CHRONODB_URL = "http://localhost:9090/api/v1/query"

# 测试标签索引
def test_label_index():
    """测试标签索引"""
    print("测试标签索引...")
    
    # 获取当前时间
    now = time.time()
    
    # 首先获取所有数据，查看实际的标签值
    print("\n=== 获取所有数据的标签 ===")
    response = requests.post(f"{CHRONODB_URL}?query=http_requests_total&time={now}")
    if response.status_code == 200:
        data = response.json()
        if data.get("status") == "success":
            result = data.get("data", {}).get("result", [])
            if result:
                # 收集所有唯一的标签值
                jobs = set()
                instances = set()
                regions = set()
                environments = set()
                
                for item in result[:100]:  # 只检查前100个结果
                    metric = item.get('metric', {})
                    if 'job' in metric:
                        jobs.add(metric['job'])
                    if 'instance' in metric:
                        instances.add(metric['instance'])
                    if 'region' in metric:
                        regions.add(metric['region'])
                    if 'environment' in metric:
                        environments.add(metric['environment'])
                
                print(f"唯一的 job 值: {sorted(jobs)}")
                print(f"唯一的 instance 值: {sorted(instances)[:5]}...")  # 只显示前5个
                print(f"唯一的 region 值: {sorted(regions)}")
                print(f"唯一的 environment 值: {sorted(environments)}")
    
    # 测试使用实际的标签值进行查询
    print("\n=== 使用实际标签值测试 ===")
    test_cases = [
        ("job=api", "http_requests_total{job='api'}"),
        ("job=frontend", "http_requests_total{job='frontend'}"),
        ("environment=production", "http_requests_total{environment='production'}"),
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
            else:
                print(f"错误: {data.get('error', 'Unknown error')}")
        else:
            print(f"状态码: {response.status_code} - {response.text}")

if __name__ == "__main__":
    test_label_index()
