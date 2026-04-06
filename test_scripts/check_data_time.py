#!/usr/bin/env python3
"""
检查数据的时间范围
"""

import requests
import time

# 服务器地址
CHRONODB_URL = "http://localhost:9090/api/v1/query"

# 检查数据时间范围
def check_data_time():
    """检查数据的时间范围"""
    print("检查数据的时间范围...")
    
    # 获取当前时间
    now = time.time()
    print(f"当前时间: {now}")
    
    # 测试查询 - 获取所有数据的时间戳
    query = "http_requests_total"
    
    try:
        response = requests.post(f"{CHRONODB_URL}?query={requests.utils.quote(query)}&time={now}")
        
        if response.status_code == 200:
            data = response.json()
            if data.get("status") == "success":
                result = data.get("data", {}).get("result", [])
                print(f"结果数量: {len(result)}")
                
                if result:
                    # 收集所有时间戳
                    timestamps = []
                    for item in result:
                        for sample in item.get('values', [item.get('value', [])]):
                            if len(sample) >= 2:
                                timestamps.append(float(sample[0]))
                    
                    if timestamps:
                        min_ts = min(timestamps)
                        max_ts = max(timestamps)
                        print(f"数据时间范围: {min_ts} - {max_ts}")
                        print(f"当前时间与数据最大时间的差值: {now - max_ts} 秒")
                        
                        # 测试使用数据最大时间作为查询时间
                        print(f"\n使用数据最大时间作为查询时间: {max_ts}")
                        response = requests.post(f"{CHRONODB_URL}?query={requests.utils.quote('http_requests_total{job=\'api\'}')}&time={max_ts}")
                        if response.status_code == 200:
                            data = response.json()
                            if data.get("status") == "success":
                                result = data.get("data", {}).get("result", [])
                                print(f"带过滤条件的结果数量: {len(result)}")
                    else:
                        print("没有找到时间戳")
                else:
                    print("没有结果")
            else:
                print(f"错误: {data.get('error', 'Unknown error')}")
        else:
            print(f"状态码: {response.status_code} - {response.text}")
    except Exception as e:
        print(f"异常: {e}")

if __name__ == "__main__":
    check_data_time()
