#!/usr/bin/env python3
"""
测试ChronoDB的HTTP方法兼容性
验证GET和POST方法是否都能正常工作
"""

import requests

# 服务器地址
CHRONODB_URL = "http://localhost:9091"

# 测试GET方法
def test_get_method():
    """测试GET方法"""
    print("测试GET方法...")
    try:
        response = requests.get(f"{CHRONODB_URL}/api/v1/query", params={"query": "up"})
        if response.status_code == 200:
            data = response.json()
            if data.get("status") == "success":
                print("✅ GET方法测试成功")
                print(f"  响应: {data}")
            else:
                print(f"❌ GET方法测试失败: {data.get('error')}")
        else:
            print(f"❌ GET方法测试失败: {response.status_code} - {response.text}")
    except Exception as e:
        print(f"❌ GET方法测试时发生错误: {e}")

# 测试POST方法
def test_post_method():
    """测试POST方法"""
    print("\n测试POST方法...")
    try:
        response = requests.post(f"{CHRONODB_URL}/api/v1/query", data={"query": "up"})
        if response.status_code == 200:
            data = response.json()
            if data.get("status") == "success":
                print("✅ POST方法测试成功")
                print(f"  响应: {data}")
            else:
                print(f"❌ POST方法测试失败: {data.get('error')}")
        else:
            print(f"❌ POST方法测试失败: {response.status_code} - {response.text}")
    except Exception as e:
        print(f"❌ POST方法测试时发生错误: {e}")

# 测试查询范围API的GET和POST方法
def test_query_range_methods():
    """测试查询范围API的GET和POST方法"""
    print("\n测试查询范围API...")
    
    # 测试GET方法
    print("测试GET方法...")
    try:
        params = {
            "query": "up",
            "start": "1712345678",
            "end": "1712349278",
            "step": "15s"
        }
        response = requests.get(f"{CHRONODB_URL}/api/v1/query_range", params=params)
        if response.status_code == 200:
            data = response.json()
            if data.get("status") == "success":
                print("✅ GET方法测试成功")
                print(f"  响应: {data}")
            else:
                print(f"❌ GET方法测试失败: {data.get('error')}")
        else:
            print(f"❌ GET方法测试失败: {response.status_code} - {response.text}")
    except Exception as e:
        print(f"❌ GET方法测试时发生错误: {e}")
    
    # 测试POST方法
    print("\n测试POST方法...")
    try:
        data = {
            "query": "up",
            "start": "1712345678",
            "end": "1712349278",
            "step": "15s"
        }
        response = requests.post(f"{CHRONODB_URL}/api/v1/query_range", data=data)
        if response.status_code == 200:
            data = response.json()
            if data.get("status") == "success":
                print("✅ POST方法测试成功")
                print(f"  响应: {data}")
            else:
                print(f"❌ POST方法测试失败: {data.get('error')}")
        else:
            print(f"❌ POST方法测试失败: {response.status_code} - {response.text}")
    except Exception as e:
        print(f"❌ POST方法测试时发生错误: {e}")

if __name__ == "__main__":
    print("=== 测试ChronoDB的HTTP方法兼容性 ===")
    test_get_method()
    test_post_method()
    test_query_range_methods()
    print("\n测试完成！")
