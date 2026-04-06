#!/usr/bin/env python3
"""
测试ChronoDB和Prometheus的查询性能和数据正确性
"""

import time
import requests

# 服务器地址
PROMETHEUS_URL = "http://localhost:9090/api/v1/query_range"
CHRONODB_URL = "http://localhost:9090/api/v1/query_range"

# 测试查询列表
TEST_QUERIES = [
    # 基本查询
    ("基本查询", "cpu_usage_percent"),
    
    # 聚合函数
    ("sum聚合", "sum(cpu_usage_percent)"),
    ("avg聚合", "avg(cpu_usage_percent)"),
    ("min聚合", "min(cpu_usage_percent)"),
    ("max聚合", "max(cpu_usage_percent)"),
    
    # 标签过滤
    ("标签过滤", "cpu_usage_percent{job='frontend'}"),
    ("多标签过滤", "cpu_usage_percent{job='frontend', environment='production'}"),
    
    # 按标签聚合
    ("按job聚合", "sum(cpu_usage_percent) by (job)"),
    ("按region聚合", "avg(cpu_usage_percent) by (region)"),
    ("多标签聚合", "max(cpu_usage_percent) by (job, region)"),
    
    # 逻辑操作
    ("大于操作", "cpu_usage_percent > 50"),
    ("小于操作", "cpu_usage_percent < 50"),
    
    # 时间函数
    ("时间函数", "time()"),
    
    # 标量函数
    ("标量函数", "scalar(sum(cpu_usage_percent))"),
]

# 测试单个查询
def test_query(server_name, url, query_name, query):
    """测试单个查询"""
    try:
        # 记录开始时间
        start_time = time.time()
        
        # 计算时间范围（毫秒级）
        now_ms = int(time.time() * 1000)  # 当前时间戳（毫秒）
        start_time_range = now_ms - 86400000  # 过去24小时（毫秒）
        end_time_range = now_ms + 86400000  # 未来24小时（毫秒）
        
        # 发送查询请求，添加时间范围参数
        response = requests.get(url, params={"query": query, "start": start_time_range, "end": end_time_range, "step": "15s"})
        
        # 记录结束时间
        end_time = time.time()
        
        # 计算查询时间
        query_time = end_time - start_time
        
        # 检查响应
        if response.status_code == 200:
            data = response.json()
            if data.get("status") == "success":
                result_count = len(data.get("data", {}).get("result", []))
                print(f"✅ {server_name} {query_name}: 成功 (耗时: {query_time:.4f} 秒, 结果数量: {result_count})")
                return True, query_time, result_count
            else:
                print(f"❌ {server_name} {query_name}: 失败 - {data.get('error', 'Unknown error')}")
                return False, query_time, 0
        else:
            print(f"❌ {server_name} {query_name}: 失败 ({response.status_code}) - {response.text}")
            return False, query_time, 0
    except Exception as e:
        print(f"❌ {server_name} {query_name}: 错误 - {e}")
        return False, 0, 0

# 测试所有查询
def test_all_queries():
    """测试所有查询"""
    print("=== 测试ChronoDB和Prometheus的查询性能和数据正确性 ===")
    
    # 存储测试结果
    results = []
    
    # 测试Prometheus
    print("\n=== 测试Prometheus ===")
    prometheus_times = []
    for query_name, query in TEST_QUERIES:
        success, query_time, result_count = test_query("Prometheus", PROMETHEUS_URL, query_name, query)
        prometheus_times.append(query_time)
        results.append((query_name, "Prometheus", success, query_time, result_count))
    
    # 测试ChronoDB
    print("\n=== 测试ChronoDB ===")
    chronodb_times = []
    for query_name, query in TEST_QUERIES:
        success, query_time, result_count = test_query("ChronoDB", CHRONODB_URL, query_name, query)
        chronodb_times.append(query_time)
        results.append((query_name, "ChronoDB", success, query_time, result_count))
    
    # 计算平均查询时间
    prometheus_avg_time = sum(prometheus_times) / len(prometheus_times)
    chronodb_avg_time = sum(chronodb_times) / len(chronodb_times)
    
    # 输出汇总结果
    print("\n=== 汇总结果 ===")
    print(f"Prometheus 平均查询时间: {prometheus_avg_time:.4f} 秒")
    print(f"ChronoDB 平均查询时间: {chronodb_avg_time:.4f} 秒")
    
    # 计算性能差异
    if prometheus_avg_time > 0:
        performance_ratio = chronodb_avg_time / prometheus_avg_time
        if performance_ratio < 1:
            print(f"ChronoDB 比 Prometheus 快 {1/performance_ratio:.2f} 倍")
        else:
            print(f"ChronoDB 比 Prometheus 慢 {performance_ratio:.2f} 倍")
    
    # 检查数据一致性
    print("\n=== 数据一致性检查 ===")
    for i in range(len(TEST_QUERIES)):
        query_name, _ = TEST_QUERIES[i]
        prometheus_result = [r for r in results if r[0] == query_name and r[1] == "Prometheus"][0]
        chronodb_result = [r for r in results if r[0] == query_name and r[1] == "ChronoDB"][0]
        
        prometheus_success, prometheus_time, prometheus_count = prometheus_result[2], prometheus_result[3], prometheus_result[4]
        chronodb_success, chronodb_time, chronodb_count = chronodb_result[2], chronodb_result[3], chronodb_result[4]
        
        if prometheus_success and chronodb_success:
            if prometheus_count == chronodb_count:
                print(f"✅ {query_name}: 数据一致性通过 (结果数量: {prometheus_count})")
            else:
                print(f"❌ {query_name}: 数据一致性失败 (Prometheus: {prometheus_count}, ChronoDB: {chronodb_count})")
        else:
            print(f"⚠️  {query_name}: 无法检查数据一致性 (Prometheus: {'成功' if prometheus_success else '失败'}, ChronoDB: {'成功' if chronodb_success else '失败'})")

if __name__ == "__main__":
    test_all_queries()
    print("\n测试完成！")
