#!/usr/bin/env python3
"""
测试标签索引中的值
"""

import requests
import time

# 服务器地址
CHRONODB_URL = "http://localhost:9090/api/v1/labels"

# 测试标签索引
def test_index_labels():
    """测试标签索引中的值"""
    print("测试标签索引中的值...")
    
    # 获取所有标签名称
    print("\n=== 获取所有标签名称 ===")
    response = requests.get(CHRONODB_URL)
    if response.status_code == 200:
        data = response.json()
        if data.get("status") == "success":
            labels = data.get("data", [])
            print(f"标签名称: {labels}")
            
            # 获取每个标签的所有值
            for label in labels:
                print(f"\n=== 获取标签 {label} 的所有值 ===")
                response = requests.get(f"{CHRONODB_URL}/{label}/values")
                if response.status_code == 200:
                    data = response.json()
                    if data.get("status") == "success":
                        values = data.get("data", [])
                        print(f"标签 {label} 的值: {values[:10]}...")  # 只显示前10个
                else:
                    print(f"获取标签 {label} 的值失败: {response.status_code}")
    else:
        print(f"获取标签名称失败: {response.status_code}")

if __name__ == "__main__":
    test_index_labels()
