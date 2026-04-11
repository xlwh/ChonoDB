#!/usr/bin/env python3
"""
测试ChronoDB的时间范围限制修复
验证系列API是否能够处理大时间范围的查询
"""

import requests

# 服务器地址
CHRONODB_URL = "http://localhost:9091"

# 测试系列API的时间范围查询
def test_series_api():
    """测试系列API的时间范围查询"""
    print("测试系列API的时间范围查询...")
    
    # 测试大时间范围的查询
    print("测试大时间范围的查询...")
    try:
        params = {
            "match[]": "up",
            "start": "1712345678",
            "end": "1712349278"
        }
        response = requests.get(f"{CHRONODB_URL}/api/v1/series", params=params)
        if response.status_code == 200:
            data = response.json()
            if data.get("status") == "success":
                print("✅ 大时间范围查询测试成功")
                print(f"  响应: {data}")
            else:
                print(f"❌ 大时间范围查询测试失败: {data.get('error')}")
        else:
            print(f"❌ 大时间范围查询测试失败: {response.status_code} - {response.text}")
    except Exception as e:
        print(f"❌ 大时间范围查询测试时发生错误: {e}")
    
    # 测试更大时间范围的查询
    print("\n测试更大时间范围的查询...")
    try:
        params = {
            "match[]": "up",
            "start": "1680000000",  # 约2023年3月
            "end": "1712349278"   # 约2024年4月
        }
        response = requests.get(f"{CHRONODB_URL}/api/v1/series", params=params)
        if response.status_code == 200:
            data = response.json()
            if data.get("status") == "success":
                print("✅ 更大时间范围查询测试成功")
                print(f"  响应: {data}")
            else:
                print(f"❌ 更大时间范围查询测试失败: {data.get('error')}")
        else:
            print(f"❌ 更大时间范围查询测试失败: {response.status_code} - {response.text}")
    except Exception as e:
        print(f"❌ 更大时间范围查询测试时发生错误: {e}")

if __name__ == "__main__":
    print("=== 测试ChronoDB的时间范围限制修复 ===")
    test_series_api()
    print("\n测试完成！")
