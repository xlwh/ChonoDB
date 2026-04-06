#!/usr/bin/env python3
"""
测试ChronoDB的单条数据写入和查询
"""

import requests
import time

# 服务器地址
CHRONODB_URL = "http://localhost:9090"

# 测试写入单条数据
def test_single_write():
    """测试写入单条数据"""
    print("测试写入单条数据...")
    
    try:
        # 生成一条测试数据
        now_ms = int(time.time() * 1000)
        metric = f'test_single{{job="test", instance="test"}} 123.45 {now_ms}'
        
        # 发送请求
        response = requests.post(f"{CHRONODB_URL}/api/v1/write", data=metric.encode('utf-8'), headers={"Content-Type": "text/plain"})
        
        if response.status_code == 204:
            print(f"✅ 写入成功")
        else:
            print(f"❌ 写入失败: {response.status_code} - {response.text}")
        
    except Exception as e:
        print(f"❌ 写入时发生错误: {e}")

# 测试查询单条数据
def test_single_query():
    """测试查询单条数据"""
    print("测试查询单条数据...")
    
    try:
        # 发送请求
        now_s = time.time()
        response = requests.get(f"{CHRONODB_URL}/api/v1/query", params={"query": "test_single", "time": now_s})
        
        if response.status_code == 200:
            data = response.json()
            if data.get("status") == "success":
                result = data.get("data", {})
                result_type = result.get("resultType")
                results = result.get("result", [])
                print(f"✅ 查询成功，结果类型: {result_type}, 结果数量: {len(results)}")
                if results:
                    print(f"✅ 第一个结果: {results[0]}")
            else:
                print(f"❌ 查询失败: {data.get('error')}")
        else:
            print(f"❌ 查询失败: {response.status_code} - {response.text}")
        
    except Exception as e:
        print(f"❌ 查询时发生错误: {e}")

if __name__ == "__main__":
    print("=== 测试ChronoDB的单条数据写入和查询 ===")
    test_single_write()
    test_single_query()
    print("\n测试完成！")
