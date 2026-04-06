#!/usr/bin/env python3

"""
测试文本格式的指标解析
"""

import requests
import time

# 服务器地址
CHRONODB_URL = "http://localhost:9090"

# 测试文本格式的指标
def test_text_format():
    """测试文本格式的指标"""
    print("=== 测试文本格式的指标 ===")
    
    # 生成一个简单的指标
    metric = "test_metric{job=\"job_0\", instance=\"instance_0\", region=\"region_0\"} 42.11 1775460000000"
    print(f"生成的指标: {metric}")
    
    # 发送请求
    headers = {"Content-Type": "text/plain"}
    response = requests.post(f"{CHRONODB_URL}/api/v1/write", data=metric, headers=headers)
    
    print(f"响应状态码: {response.status_code}")
    print(f"响应内容: {response.text}")
    
    # 等待一秒钟，确保数据被写入
    time.sleep(1)
    
    # 测试标签值
    response = requests.get(f"{CHRONODB_URL}/api/v1/label/__name__/values")
    print(f"标签值响应状态码: {response.status_code}")
    print(f"标签值响应内容: {response.text}")

if __name__ == "__main__":
    test_text_format()
