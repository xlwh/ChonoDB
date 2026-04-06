#!/usr/bin/env python3
"""
测试ChronoDB内存存储的统计信息，验证数据是否被正确写入
"""

import requests

# 服务器地址
CHRONODB_URL = "http://localhost:9090"

# 测试内存存储统计信息
def test_memstore_stats():
    """测试内存存储统计信息"""
    print("测试ChronoDB内存存储统计信息...")
    
    try:
        # 发送请求获取标签名称
        response = requests.get(f"{CHRONODB_URL}/api/v1/labels")
        if response.status_code == 200:
            data = response.json()
            if data.get("status") == "success":
                labels = data.get("data", [])
                print(f"✅ 标签名称: {labels}")
            else:
                print(f"❌ 获取标签名称失败: {data.get('error')}")
        else:
            print(f"❌ 获取标签名称失败: {response.status_code} - {response.text}")
        
        # 发送请求获取标签值
        response = requests.get(f"{CHRONODB_URL}/api/v1/label/__name__/values")
        if response.status_code == 200:
            data = response.json()
            if data.get("status") == "success":
                values = data.get("data", [])
                print(f"✅ 指标名称: {values}")
            else:
                print(f"❌ 获取指标名称失败: {data.get('error')}")
        else:
            print(f"❌ 获取指标名称失败: {response.status_code} - {response.text}")
        
        # 发送请求获取系列信息
        import time
        now_ms = int(time.time() * 1000)  # 当前时间戳（毫秒）
        params = {
            "match[]": "test_metric",
            "start": now_ms - 86400000,  # 过去24小时
            "end": now_ms + 86400000    # 未来24小时
        }
        response = requests.get(f"{CHRONODB_URL}/api/v1/series", params=params)
        if response.status_code == 200:
            data = response.json()
            if data.get("status") == "success":
                series = data.get("data", [])
                print(f"✅ 系列数量: {len(series)}")
                if series:
                    print(f"✅ 第一个系列: {series[0]}")
            else:
                print(f"❌ 获取系列信息失败: {data.get('error')}")
        else:
            print(f"❌ 获取系列信息失败: {response.status_code} - {response.text}")
        
    except Exception as e:
        print(f"❌ 测试时发生错误: {e}")

if __name__ == "__main__":
    print("=== 测试ChronoDB内存存储统计信息 ===")
    test_memstore_stats()
    print("\n测试完成！")
