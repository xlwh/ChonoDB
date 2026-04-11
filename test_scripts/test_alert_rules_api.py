#!/usr/bin/env python3
"""
测试ChronoDB的告警规则解析修复
验证告警规则API是否能够正确返回规则详情
"""

import requests

# 服务器地址
CHRONODB_URL = "http://localhost:9091"

# 测试告警规则API
def test_alert_rules_api():
    """测试告警规则API"""
    print("测试告警规则API...")
    
    try:
        response = requests.get(f"{CHRONODB_URL}/api/v1/rules")
        if response.status_code == 200:
            data = response.json()
            if data.get("status") == "success":
                print("✅ 告警规则API测试成功")
                print(f"  响应: {data}")
                
                # 检查是否返回了规则详情
                rules = data.get("data", {}).get("groups", [])
                if rules:
                    print(f"  找到 {len(rules)} 个规则组")
                    for i, group in enumerate(rules):
                        print(f"  规则组 {i+1}: {group.get('name')}")
                        print(f"    规则数量: {len(group.get('rules', []))}")
                        for j, rule in enumerate(group.get('rules', [])):
                            print(f"    规则 {j+1}: {rule.get('name')}")
                            print(f"      类型: {rule.get('type')}")
                            if rule.get('type') == 'alerting':
                                print(f"      持续时间: {rule.get('duration')}")
            else:
                print(f"❌ 告警规则API测试失败: {data.get('error')}")
        else:
            print(f"❌ 告警规则API测试失败: {response.status_code} - {response.text}")
    except Exception as e:
        print(f"❌ 告警规则API测试时发生错误: {e}")

if __name__ == "__main__":
    print("=== 测试ChronoDB的告警规则解析修复 ===")
    test_alert_rules_api()
    print("\n测试完成！")
