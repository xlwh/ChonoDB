#!/usr/bin/env python3
"""
测试Prometheus和ChronoDB的PromQL查询兼容性
"""

import requests

# 服务器地址
PROMETHEUS_URL = "http://localhost:9090/api/v1/query"
CHRONODB_URL = "http://localhost:9090/api/v1/query"

# 测试查询列表
TEST_QUERIES = [
    "up",  # 基本查询
    "sum(up)",  # 聚合函数
    "rate(http_requests_total[5m])",  # 速率函数
    "up{job='prometheus'}",  # 标签过滤
    "sum by (job) (up)",  # 按标签聚合
    "up or vector(0)",  # 逻辑操作
    "time()",  # 时间函数
    "scalar(up)",  # 标量函数
]

# 测试每个查询
def test_query(server_name, url, query):
    """测试单个PromQL查询"""
    try:
        if server_name == "ChronoDB":
            # ChronoDB使用POST方法，但将查询参数放在URL查询字符串中
            response = requests.post(f"{url}?query={requests.utils.quote(query)}")
        else:
            # Prometheus使用GET方法
            response = requests.get(url, params={"query": query})
        
        status_code = response.status_code
        if status_code == 200:
            data = response.json()
            if data.get("status") == "success":
                print(f"✅ {server_name} 查询 '{query}': 成功")
                if data.get("data", {}).get("result"):
                    print(f"  结果数量: {len(data['data']['result'])}")
                else:
                    print(f"  无结果")
            else:
                print(f"❌ {server_name} 查询 '{query}': 失败 - {data.get('error', 'Unknown error')}")
        else:
            print(f"❌ {server_name} 查询 '{query}': 失败 ({status_code}) - {response.text}")
    except Exception as e:
        print(f"❌ {server_name} 查询 '{query}': 错误 - {e}")

# 测试所有查询
def test_all_queries():
    """测试所有PromQL查询"""
    print("测试PromQL查询兼容性...\n")
    
    print("=== 测试Prometheus ===")
    for query in TEST_QUERIES:
        test_query("Prometheus", PROMETHEUS_URL, query)
    
    print("\n=== 测试ChronoDB ===")
    for query in TEST_QUERIES:
        test_query("ChronoDB", CHRONODB_URL, query)

if __name__ == "__main__":
    test_all_queries()
    print("\n测试完成！")
