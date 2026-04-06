#!/usr/bin/env python3
"""
测试Prometheus和ChronoDB的告警规则兼容性
"""

import requests

# 服务器地址
PROMETHEUS_URL = "http://localhost:9090/api/v1/rules"
CHRONODB_URL = "http://localhost:9090/api/v1/rules"

# 测试告警规则API
def test_alert_rules():
    """测试告警规则API"""
    print("测试告警规则兼容性...\n")
    
    # 测试Prometheus
    print("=== 测试Prometheus ===")
    try:
        response = requests.get(PROMETHEUS_URL)
        if response.status_code == 200:
            data = response.json()
            if data.get("status") == "success":
                groups = data.get("data", {}).get("groups", [])
                print(f"✅ Prometheus告警规则API成功，返回了 {len(groups)} 个规则组")
                for group in groups:
                    print(f"  规则组: {group.get('name')}")
                    rules = group.get('rules', [])
                    print(f"  规则数量: {len(rules)}")
                    for rule in rules:
                        print(f"    - {rule.get('alert')}")
            else:
                print(f"❌ Prometheus告警规则API失败 - {data.get('error', 'Unknown error')}")
        else:
            print(f"❌ Prometheus告警规则API失败 ({response.status_code}) - {response.text}")
    except Exception as e:
        print(f"❌ Prometheus告警规则API错误 - {e}")
    
    # 测试ChronoDB
    print("\n=== 测试ChronoDB ===")
    try:
        response = requests.get(CHRONODB_URL)
        if response.status_code == 200:
            data = response.json()
            if data.get("status") == "success":
                groups = data.get("data", {}).get("groups", [])
                print(f"✅ ChronoDB告警规则API成功，返回了 {len(groups)} 个规则组")
                for group in groups:
                    print(f"  规则组: {group.get('name')}")
                    rules = group.get('rules', [])
                    print(f"  规则数量: {len(rules)}")
                    for rule in rules:
                        print(f"    - {rule.get('alert')}")
            else:
                print(f"❌ ChronoDB告警规则API失败 - {data.get('error', 'Unknown error')}")
        else:
            print(f"❌ ChronoDB告警规则API失败 ({response.status_code}) - {response.text}")
    except Exception as e:
        print(f"❌ ChronoDB告警规则API错误 - {e}")

if __name__ == "__main__":
    test_alert_rules()
    print("\n测试完成！")
