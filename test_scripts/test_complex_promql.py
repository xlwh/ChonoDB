#!/usr/bin/env python3
"""
测试复杂的PromQL查询场景
"""

import requests
import time

# 服务器地址
PROMETHEUS_URL = "http://localhost:9092/api/v1/query"
CHRONODB_URL = "http://localhost:9090/api/v1/query"

# 复杂查询测试用例
COMPLEX_TEST_QUERIES = [
    # 基本聚合查询
    "sum(http_requests_total)",
    "avg(cpu_usage_percent)",
    "max(memory_usage_bytes)",
    "min(disk_usage_percent)",
    
    # 带标签的聚合查询
    "sum by (job) (http_requests_total)",
    "avg by (region) (cpu_usage_percent)",
    "max by (environment) (memory_usage_bytes)",
    "sum by (job, region) (http_errors_total)",
    
    # 速率和变化率查询
    "rate(http_requests_total[5m])",
    "irate(http_requests_total[1m])",
    "increase(http_requests_total[10m])",
    "rate(http_errors_total[5m])",
    
    # 带过滤条件的查询
    "sum by (job) (http_requests_total{environment='production'})",
    "avg(cpu_usage_percent{region='us-east-1'})",
    "max(memory_usage_bytes{job='database'})",
    
    # 数学运算
    "http_requests_total / 1000",
    "cpu_usage_percent * 100",
    "memory_usage_bytes / 1024 / 1024",  # 转换为MB
    
    # 逻辑运算
    "http_requests_total > 5000",
    "cpu_usage_percent < 50",
    "http_errors_total > 0",
    
    # 时间函数
    "time()",
    "timestamp(http_requests_total)",
    "vector(time())",
    
    # 预测函数
    "predict_linear(http_requests_total[10m], 3600)",
    
    # 复杂组合查询
    "sum by (job, region) (rate(http_requests_total[5m]))",
    "avg by (environment) (rate(http_errors_total[5m]) / rate(http_requests_total[5m]))",
    "sum by (job) (rate(http_requests_total[5m])) > 10",
    
    # 直方图相关查询
    "histogram_quantile(0.95, sum(rate(http_request_duration_seconds_bucket[5m])) by (le, job))",
    "sum(rate(http_request_duration_seconds_sum[5m])) by (job) / sum(rate(http_request_duration_seconds_count[5m])) by (job)",
    
    # 时间范围查询 (使用 query_range)
    # 注意：这些需要单独测试
]

# 测试单个查询
def test_query(server_name, url, query):
    """测试单个PromQL查询"""
    try:
        # 获取当前时间作为查询时间
        query_time = time.time()
        
        if server_name == "ChronoDB":
            # ChronoDB使用POST方法，但将查询参数放在URL查询字符串中
            response = requests.post(f"{url}?query={requests.utils.quote(query)}&time={query_time}")
        else:
            # Prometheus使用GET方法
            response = requests.get(url, params={"query": query, "time": query_time})
        
        status_code = response.status_code
        if status_code == 200:
            data = response.json()
            if data.get("status") == "success":
                result_count = len(data.get("data", {}).get("result", []))
                print(f"✅ {server_name} 查询 '{query}': 成功")
                if result_count > 0:
                    print(f"  结果数量: {result_count}")
                else:
                    print(f"  无结果")
            else:
                print(f"❌ {server_name} 查询 '{query}': 失败 - {data.get('error', 'Unknown error')}")
        else:
            print(f"❌ {server_name} 查询 '{query}': 失败 ({status_code}) - {response.text}")
    except Exception as e:
        print(f"❌ {server_name} 查询 '{query}': 错误 - {e}")

# 测试时间范围查询
def test_query_range(server_name, url, query, start, end, step):
    """测试时间范围查询"""
    try:
        if server_name == "ChronoDB":
            # ChronoDB使用POST方法
            response = requests.post(f"{url.replace('/query', '/query_range')}", 
                                  params={"query": query, "start": start, "end": end, "step": step})
        else:
            # Prometheus使用GET方法
            response = requests.get(url.replace('/query', '/query_range'), 
                                 params={"query": query, "start": start, "end": end, "step": step})
        
        status_code = response.status_code
        if status_code == 200:
            data = response.json()
            if data.get("status") == "success":
                result_count = len(data.get("data", {}).get("result", []))
                print(f"✅ {server_name} 时间范围查询 '{query}': 成功")
                if result_count > 0:
                    print(f"  结果数量: {result_count}")
                else:
                    print(f"  无结果")
            else:
                print(f"❌ {server_name} 时间范围查询 '{query}': 失败 - {data.get('error', 'Unknown error')}")
        else:
            print(f"❌ {server_name} 时间范围查询 '{query}': 失败 ({status_code}) - {response.text}")
    except Exception as e:
        print(f"❌ {server_name} 时间范围查询 '{query}': 错误 - {e}")

# 测试所有复杂查询
def test_complex_queries():
    """测试所有复杂PromQL查询"""
    print("测试复杂PromQL查询...\n")
    
    # 测试基本查询
    print("=== 测试基本聚合查询 ===")
    for i, query in enumerate(COMPLEX_TEST_QUERIES[:4]):
        test_query("ChronoDB", CHRONODB_URL, query)
    
    print("\n=== 测试带标签的聚合查询 ===")
    for i, query in enumerate(COMPLEX_TEST_QUERIES[4:8]):
        test_query("ChronoDB", CHRONODB_URL, query)
    
    print("\n=== 测试速率和变化率查询 ===")
    for i, query in enumerate(COMPLEX_TEST_QUERIES[8:12]):
        test_query("ChronoDB", CHRONODB_URL, query)
    
    print("\n=== 测试带过滤条件的查询 ===")
    for i, query in enumerate(COMPLEX_TEST_QUERIES[12:16]):
        test_query("ChronoDB", CHRONODB_URL, query)
    
    print("\n=== 测试数学运算 ===")
    for i, query in enumerate(COMPLEX_TEST_QUERIES[16:19]):
        test_query("ChronoDB", CHRONODB_URL, query)
    
    print("\n=== 测试逻辑运算 ===")
    for i, query in enumerate(COMPLEX_TEST_QUERIES[19:22]):
        test_query("ChronoDB", CHRONODB_URL, query)
    
    print("\n=== 测试时间函数 ===")
    for i, query in enumerate(COMPLEX_TEST_QUERIES[22:25]):
        test_query("ChronoDB", CHRONODB_URL, query)
    
    print("\n=== 测试预测函数 ===")
    for i, query in enumerate(COMPLEX_TEST_QUERIES[25:26]):
        test_query("ChronoDB", CHRONODB_URL, query)
    
    print("\n=== 测试复杂组合查询 ===")
    for i, query in enumerate(COMPLEX_TEST_QUERIES[26:29]):
        test_query("ChronoDB", CHRONODB_URL, query)
    
    # 测试时间范围查询
    print("\n=== 测试时间范围查询 ===")
    now = time.time()
    start = now - 3600  # 1小时前
    end = now
    step = "15s"
    
    range_queries = [
        "sum by (job) (rate(http_requests_total[5m]))",
        "avg(cpu_usage_percent)",
        "max(memory_usage_bytes)"
    ]
    
    for query in range_queries:
        test_query_range("ChronoDB", CHRONODB_URL, query, start, end, step)

if __name__ == "__main__":
    test_complex_queries()
    print("\n复杂查询测试完成！")
