#!/usr/bin/env python3
"""
生成测试报告
"""
import json
import os
from datetime import datetime
from pathlib import Path

def generate_report():
    reports_dir = Path(__file__).parent.parent / "reports"
    reports_dir.mkdir(exist_ok=True)
    
    junit_file = reports_dir / "junit.xml"
    if not junit_file.exists():
        print("未找到测试结果文件")
        return
    
    report_data = {
        "timestamp": datetime.now().isoformat(),
        "test_framework": "ChronoDB Integration Tests",
        "status": "completed"
    }
    
    report_file = reports_dir / "test-summary.json"
    with open(report_file, 'w') as f:
        json.dump(report_data, f, indent=2)
    
    print(f"测试报告已生成: {report_file}")

if __name__ == "__main__":
    generate_report()
