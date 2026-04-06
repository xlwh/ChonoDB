#!/usr/bin/env python3
"""
生成大规模测试数据，用于测试复杂查询和大规模数据场景
"""

import requests
import time
import random

# 服务器地址
CHRONODB_URL = "http://localhost:9090/api/v1/write"

# 生成测试数据
def generate_large_test_data():
    """生成大规模测试数据"""
    print("生成大规模测试数据...")
    
    # 生成不同的标签组合
    jobs = ["frontend", "backend", "database", "cache", "api"]
    instances = [f"instance_{i}" for i in range(1, 6)]  # 减少到5个实例
    regions = ["us-east-1", "us-west-2"]  # 减少到2个区域
    environments = ["production", "staging"]  # 减少到2个环境
    
    # 当前时间戳
    now = int(time.time() * 1000)
    
    # 生成多个时间序列
    data_lines = []
    
    # 为每个标签组合生成数据
    for job in jobs:
        for instance in instances:
            for region in regions:
                for environment in environments:
                    # 生成多个指标
                    metrics = [
                        ("http_requests_total", random.randint(1000, 10000)),
                        ("http_errors_total", random.randint(10, 100)),
                        ("cpu_usage_percent", random.uniform(10, 90)),
                        ("memory_usage_bytes", random.randint(100000000, 1000000000)),
                        ("disk_usage_percent", random.uniform(20, 80)),
                        ("network_bytes_sent", random.randint(1000000, 10000000)),
                        ("network_bytes_received", random.randint(1000000, 10000000)),
                        ("request_duration_seconds", random.uniform(0.01, 1.0))
                    ]
                    
                    for metric_name, base_value in metrics:
                        # 生成标签字符串
                        labels = f"job=\"{job}\", instance=\"{instance}\", region=\"{region}\", environment=\"{environment}\""
                        
                        # 生成多个时间点的数据
                        for i in range(100):  # 每个时间序列100个数据点
                            timestamp = now - (99 - i) * 1000  # 每秒一个数据点
                            value = base_value * (1 + random.uniform(-0.1, 0.1))  # 添加一些随机波动
                            
                            if metric_name in ["http_requests_total", "http_errors_total"]:
                                # 计数器类型，值递增
                                value = int(base_value + i * random.randint(1, 10))
                            else:
                                #  gauge 类型，值波动
                                value = round(value, 2)
                            
                            # 构建数据行
                            line = f"{metric_name}{{{labels}}} {value} {timestamp}"
                            data_lines.append(line)
    
    print(f"生成了 {len(data_lines)} 条数据")
    
    # 分批次发送数据
    batch_size = 1000
    for i in range(0, len(data_lines), batch_size):
        batch = data_lines[i:i+batch_size]
        data = "\n".join(batch)
        
        try:
            response = requests.post(CHRONODB_URL, data=data.encode('utf-8'), 
                                   headers={"Content-Type": "text/plain"})
            if response.status_code == 204:
                print(f"✅ 成功发送批次 {i//batch_size + 1}/{(len(data_lines)+batch_size-1)//batch_size}")
            else:
                print(f"❌ 发送批次失败: {response.status_code} - {response.text}")
        except Exception as e:
            print(f"❌ 发送数据时出错: {e}")
        
        # 短暂休眠，避免过载
        time.sleep(0.1)

if __name__ == "__main__":
    generate_large_test_data()
    print("大规模测试数据生成完成！")
