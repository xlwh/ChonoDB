#!/usr/bin/env python3
"""
测试标签过滤功能
"""

import requests
import time

# 服务器地址
CHRONODB_URL = "http://localhost:9090/api/v1/query"

# 测试标签过滤
def test_label_filter():
    """测试标签过滤功能"""
    print("测试标签过滤功能...")
    
    # 获取当前时间
    now = time.time()
    
    # 测试不同的标签过滤
    test_cases = [
        ("无过滤", "http_requests_total"),
        ("job=frontend", "http_requests_total{job='frontend'}"),
        ("job=backend", "http_requests_total{job='backend'}"),
        ("environment=production", "http_requests_total{environment='production'}"),
        ("region=us-east-1", "http_requests_total{region='us-east-1'}"),
        ("复合过滤", "http_requests_total{job='frontend', environment='production'}"),
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
                        # 显示前3个结果的标签
                        for i, item in enumerate(result[:3]):
                            print(f"  结果{i+1}标签: {item['metric']}")
                else:
                    print(f"错误: {data.get('error', 'Unknown error')}")
            else:
                print(f"状态码: {response.status_code} - {response.text}")
        except Exception as e:
            print(f"异常: {e}")

if __name__ == "__main__":
    test_label_filter()
