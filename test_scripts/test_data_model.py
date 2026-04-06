#!/usr/bin/env python3
"""
测试Prometheus和ChronoDB的数据模型兼容性
"""

import requests

# 服务器地址
PROMETHEUS_BASE = "http://localhost:9090"
CHRONODB_BASE = "http://localhost:9090"

# 测试标签名称API
def test_labels():
    """测试标签名称API"""
    print("测试标签名称API...")
    
    # 测试Prometheus
    print("\n=== 测试Prometheus ===")
    try:
        response = requests.get(f"{PROMETHEUS_BASE}/api/v1/labels")
        if response.status_code == 200:
            data = response.json()
            if data.get("status") == "success":
                labels = data.get("data", [])
                print(f"✅ Prometheus标签名称API成功，返回了 {len(labels)} 个标签")
                for label in labels:
                    print(f"  - {label}")
            else:
                print(f"❌ Prometheus标签名称API失败 - {data.get('error', 'Unknown error')}")
        else:
            print(f"❌ Prometheus标签名称API失败 ({response.status_code}) - {response.text}")
    except Exception as e:
        print(f"❌ Prometheus标签名称API错误 - {e}")
    
    # 测试ChronoDB
    print("\n=== 测试ChronoDB ===")
    try:
        response = requests.get(f"{CHRONODB_BASE}/api/v1/labels")
        if response.status_code == 200:
            data = response.json()
            if data.get("status") == "success":
                labels = data.get("data", [])
                print(f"✅ ChronoDB标签名称API成功，返回了 {len(labels)} 个标签")
                for label in labels:
                    print(f"  - {label}")
            else:
                print(f"❌ ChronoDB标签名称API失败 - {data.get('error', 'Unknown error')}")
        else:
            print(f"❌ ChronoDB标签名称API失败 ({response.status_code}) - {response.text}")
    except Exception as e:
        print(f"❌ ChronoDB标签名称API错误 - {e}")

# 测试元数据API
def test_metadata():
    """测试元数据API"""
    print("\n测试元数据API...")
    
    # 测试Prometheus
    print("\n=== 测试Prometheus ===")
    try:
        response = requests.get(f"{PROMETHEUS_BASE}/api/v1/metadata")
        if response.status_code == 200:
            data = response.json()
            if data.get("status") == "success":
                metrics = data.get("data", {})
                print(f"✅ Prometheus元数据API成功，返回了 {len(metrics)} 个指标")
                for metric in list(metrics.keys())[:5]:  # 只显示前5个
                    print(f"  - {metric}")
                if len(metrics) > 5:
                    print(f"  ... 还有 {len(metrics) - 5} 个指标")
            else:
                print(f"❌ Prometheus元数据API失败 - {data.get('error', 'Unknown error')}")
        else:
            print(f"❌ Prometheus元数据API失败 ({response.status_code}) - {response.text}")
    except Exception as e:
        print(f"❌ Prometheus元数据API错误 - {e}")
    
    # 测试ChronoDB
    print("\n=== 测试ChronoDB ===")
    try:
        response = requests.get(f"{CHRONODB_BASE}/api/v1/metadata")
        if response.status_code == 200:
            data = response.json()
            if data.get("status") == "success":
                metrics = data.get("data", {})
                print(f"✅ ChronoDB元数据API成功，返回了 {len(metrics)} 个指标")
                for metric in list(metrics.keys())[:5]:  # 只显示前5个
                    print(f"  - {metric}")
                if len(metrics) > 5:
                    print(f"  ... 还有 {len(metrics) - 5} 个指标")
            else:
                print(f"❌ ChronoDB元数据API失败 - {data.get('error', 'Unknown error')}")
        else:
            print(f"❌ ChronoDB元数据API失败 ({response.status_code}) - {response.text}")
    except Exception as e:
        print(f"❌ ChronoDB元数据API错误 - {e}")

# 测试系列API
def test_series():
    """测试系列API"""
    print("\n测试系列API...")
    
    # 测试Prometheus
    print("\n=== 测试Prometheus ===")
    try:
        response = requests.get(f"{PROMETHEUS_BASE}/api/v1/series", params={"match[]": "up"})
        if response.status_code == 200:
            data = response.json()
            if data.get("status") == "success":
                series = data.get("data", [])
                print(f"✅ Prometheus系列API成功，返回了 {len(series)} 个系列")
                for s in series[:3]:  # 只显示前3个
                    print(f"  - {s}")
                if len(series) > 3:
                    print(f"  ... 还有 {len(series) - 3} 个系列")
            else:
                print(f"❌ Prometheus系列API失败 - {data.get('error', 'Unknown error')}")
        else:
            print(f"❌ Prometheus系列API失败 ({response.status_code}) - {response.text}")
    except Exception as e:
        print(f"❌ Prometheus系列API错误 - {e}")
    
    # 测试ChronoDB
    print("\n=== 测试ChronoDB ===")
    try:
        response = requests.get(f"{CHRONODB_BASE}/api/v1/series", params={"match[]": "up"})
        if response.status_code == 200:
            data = response.json()
            if data.get("status") == "success":
                series = data.get("data", [])
                print(f"✅ ChronoDB系列API成功，返回了 {len(series)} 个系列")
                for s in series[:3]:  # 只显示前3个
                    print(f"  - {s}")
                if len(series) > 3:
                    print(f"  ... 还有 {len(series) - 3} 个系列")
            else:
                print(f"❌ ChronoDB系列API失败 - {data.get('error', 'Unknown error')}")
        else:
            print(f"❌ ChronoDB系列API失败 ({response.status_code}) - {response.text}")
    except Exception as e:
        print(f"❌ ChronoDB系列API错误 - {e}")

if __name__ == "__main__":
    test_labels()
    test_metadata()
    test_series()
    print("\n测试完成！")
