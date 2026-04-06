#!/usr/bin/env python3
"""
测试聚合查询和标签过滤
"""

import requests
import time

# 服务器地址
CHRONODB_URL = "http://localhost:9090/api/v1/query"

# 测试聚合查询
def test_aggregation():
    """测试聚合查询和标签过滤"""
    print("测试聚合查询和标签过滤...")
    
    # 获取当前时间
    now = time.time()
    
    # 测试查询
    test_cases = [
        # 基本查询
        ("基本查询", "http_requests_total"),
        
        # 聚合查询
        ("sum聚合", "sum(http_requests_total)"),
        ("avg聚合", "avg(cpu_usage_percent)"),
        ("max聚合", "max(memory_usage_bytes)"),
        
        # 带标签的聚合
        ("按job聚合", "sum by (job) (http_requests_total)"),
        ("按region聚合", "avg by (region) (cpu_usage_percent)"),
        
        # 标签过滤
        ("job=frontend", "http_requests_total{job='frontend'}"),
        ("environment=production", "http_requests_total{environment='production'}"),
        
        # 带过滤的聚合
        ("production环境的sum", "sum(http_requests_total{environment='production'})"),
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
                    if result:
                        for i, item in enumerate(result[:3]):  # 显示前3个结果
                            print(f"  结果{i+1}: {item}")
                else:
                    print(f"错误: {data.get('error', 'Unknown error')}")
            else:
                print(f"状态码: {response.status_code} - {response.text}")
        except Exception as e:
            print(f"异常: {e}")

if __name__ == "__main__":
    test_aggregation()
