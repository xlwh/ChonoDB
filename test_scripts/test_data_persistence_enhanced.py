#!/usr/bin/env python3
"""
ChronoDB 增强数据持久化测试
测试场景：
1. 写入第一批数据
2. 调用 flush 刷盘
3. 写入第二批数据（不刷盘）
4. 重启服务
5. 验证重启前后数据一致性
"""

import requests
import json
import time
import subprocess
import os
import sys
from datetime import datetime

class EnhancedDataPersistenceTest:
    def __init__(self):
        self.base_url = "http://localhost:9090"
        self.data_dir = "/Users/zhb/workspace/chonodb/data_test"
        self.server_process = None
        self.first_batch_data = []
        self.second_batch_data = []
        self.all_test_data = []
        
    def setup_data_dir(self):
        """设置数据目录"""
        if os.path.exists(self.data_dir):
            import shutil
            shutil.rmtree(self.data_dir)
        os.makedirs(self.data_dir)
        print(f"✓ 创建数据目录: {self.data_dir}")
        
    def start_server(self):
        """启动 ChronoDB 服务器"""
        print("\n=== 启动 ChronoDB 服务器 ===")
        env = os.environ.copy()
        env["RUST_LOG"] = "info,chronodb=debug"
        
        server_cmd = [
            "./target/release/chronodb-server",
            "--config", "test_scripts/test_simple.yaml"
        ]
        
        if not os.path.exists(server_cmd[0]):
            print("❌ 服务器二进制文件不存在，请先编译: cargo build --release")
            return False
            
        self.server_process = subprocess.Popen(
            server_cmd,
            cwd="/Users/zhb/workspace/chonodb",
            env=env,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True
        )
        
        # 等待服务器启动
        print("等待服务器启动...")
        for i in range(30):
            try:
                response = requests.get(f"{self.base_url}/-/healthy", timeout=2)
                if response.status_code in [200, 204]:
                    print("✓ 服务器启动成功！")
                    return True
            except:
                pass
            time.sleep(1)
            print(f"  等待 {i+1}/30 秒...")
            
        print("❌ 服务器启动超时")
        return False
        
    def stop_server(self):
        """停止 ChronoDB 服务器"""
        print("\n=== 停止 ChronoDB 服务器 ===")
        if self.server_process:
            self.server_process.terminate()
            try:
                self.server_process.wait(timeout=5)
                print("✓ 服务器已停止")
            except subprocess.TimeoutExpired:
                self.server_process.kill()
                print("✓ 服务器已强制停止")
            self.server_process = None
            
    def write_data_batch(self, batch_name, metrics_data, flush_after=False):
        """写入一批数据"""
        print(f"\n=== {batch_name} ===")
        
        url = f"{self.base_url}/api/v1/write"
        headers = {"Content-Type": "text/plain"}
        
        current_time_ms = int(time.time() * 1000)
        batch_records = []
        
        for metric_name, instance, value in metrics_data:
            # 使用 Prometheus 文本格式写入
            data = f'{metric_name}{{instance="{instance}",job="node_exporter",region="us-east-1"}} {value} {current_time_ms}'
            
            try:
                response = requests.post(
                    url,
                    headers=headers,
                    data=data,
                    timeout=10
                )
                if response.status_code in [200, 204]:
                    record = {
                        "metric": metric_name,
                        "instance": instance,
                        "value": value,
                        "timestamp": current_time_ms,
                        "batch": batch_name
                    }
                    batch_records.append(record)
                    print(f"✓ 写入 {metric_name} [{instance}] = {value}")
                else:
                    print(f"❌ 写入失败: {response.status_code}")
                    return False
            except Exception as e:
                print(f"❌ 写入异常: {e}")
                return False
        
        print(f"✓ 成功写入 {len(batch_records)} 条数据")
        
        # 如果需要刷盘
        if flush_after:
            print("\n触发数据刷盘...")
            try:
                response = requests.post(f"{self.base_url}/api/v1/admin/flush", timeout=10)
                if response.status_code in [200, 204]:
                    print("✓ 刷盘操作成功")
                else:
                    print(f"⚠ 刷盘操作返回: {response.status_code}")
            except Exception as e:
                print(f"⚠ 刷盘操作异常: {e}")
            
            # 等待刷盘完成
            print("等待数据写入完成...")
            time.sleep(2)
        
        return batch_records
        
    def query_data(self, phase=""):
        """查询所有测试数据"""
        print(f"\n=== 查询数据 {phase} ===")
        
        results = {}
        all_data = self.first_batch_data + self.second_batch_data
        
        for item in all_data:
            metric = item["metric"]
            instance = item["instance"]
            expected_value = item["value"]
            batch = item["batch"]
            
            query = f'{metric}{{instance="{instance}"}}'
            
            try:
                # 使用 range query 查询时间范围
                end_time = int(time.time())
                start_time = end_time - 300  # 查询过去5分钟的数据
                response = requests.get(
                    f"{self.base_url}/api/v1/query_range",
                    params={
                        "query": query,
                        "start": start_time,
                        "end": end_time,
                        "step": "1s"
                    },
                    timeout=10
                )
                
                if response.status_code == 200:
                    data = response.json()
                    if data.get("status") == "success":
                        result = data.get("data", {}).get("result", [])
                        if result:
                            # range query 返回的是 values 数组
                            values = result[0].get("values", [])
                            if values:
                                # 取最新的值
                                actual_value = float(values[-1][1])
                                key = f"{metric}_{instance}"
                                results[key] = {
                                    "expected": expected_value,
                                    "actual": actual_value,
                                    "match": abs(actual_value - expected_value) < 0.01,
                                    "batch": batch
                                }
                                status = "✓" if results[key]["match"] else "❌"
                                print(f"{status} [{batch}] {metric} [{instance}]: 期望={expected_value}, 实际={actual_value}")
                            else:
                                key = f"{metric}_{instance}"
                                results[key] = {
                                    "expected": expected_value,
                                    "actual": None,
                                    "match": False,
                                    "batch": batch
                                }
                                print(f"❌ [{batch}] {metric} [{instance}]: 返回结果中无 values")
                        else:
                            key = f"{metric}_{instance}"
                            results[key] = {
                                "expected": expected_value,
                                "actual": None,
                                "match": False,
                                "batch": batch
                            }
                            print(f"❌ [{batch}] {metric} [{instance}]: 未返回数据")
                    else:
                        print(f"❌ {metric} [{instance}]: 查询失败 - {data}")
                else:
                    print(f"❌ {metric} [{instance}]: HTTP {response.status_code}")
            except Exception as e:
                print(f"❌ {metric} [{instance}]: 查询异常 - {e}")
                
        return results
        
    def verify_data_integrity(self, results_before, results_after):
        """验证数据完整性"""
        print("\n=== 验证数据完整性 ===")
        
        all_match = True
        first_batch_ok = 0
        first_batch_total = 0
        second_batch_ok = 0
        second_batch_total = 0
        
        for key in results_before:
            batch = results_before[key].get("batch", "unknown")
            if batch == "第一批数据 (已刷盘)":
                first_batch_total += 1
            else:
                second_batch_total += 1
            
            if key not in results_after:
                print(f"❌ {key}: 重启后数据丢失")
                all_match = False
            elif not results_after[key]["match"]:
                print(f"❌ {key}: 数据不匹配")
                print(f"   重启前: {results_before[key]['actual']}")
                print(f"   重启后: {results_after[key]['actual']}")
                all_match = False
            else:
                if batch == "第一批数据 (已刷盘)":
                    first_batch_ok += 1
                else:
                    second_batch_ok += 1
                print(f"✓ {key}: 数据一致 ({batch})")
        
        # 统计结果
        print(f"\n数据一致性统计:")
        print(f"  第一批数据 (已刷盘): {first_batch_ok}/{first_batch_total} 通过")
        print(f"  第二批数据 (未刷盘): {second_batch_ok}/{second_batch_total} 通过")
        
        return all_match, first_batch_ok, first_batch_total, second_batch_ok, second_batch_total
        
    def run_test(self):
        """运行完整测试"""
        print("=" * 70)
        print("ChronoDB 增强数据持久化测试")
        print("=" * 70)
        print("\n测试场景:")
        print("1. 写入第一批数据")
        print("2. 调用 flush 刷盘")
        print("3. 写入第二批数据（不刷盘）")
        print("4. 重启服务")
        print("5. 验证重启前后数据一致性")
        print("=" * 70)
        
        # 步骤 1: 设置数据目录
        self.setup_data_dir()
        
        # 步骤 2: 启动服务器
        if not self.start_server():
            return False
            
        # 步骤 3: 写入第一批数据并刷盘
        first_batch_metrics = [
            ("cpu_usage", "server-01", 45.5),
            ("cpu_usage", "server-02", 67.2),
            ("memory_usage", "server-01", 78.3),
            ("memory_usage", "server-02", 82.1),
        ]
        self.first_batch_data = self.write_data_batch(
            "第一批数据 (已刷盘)", 
            first_batch_metrics, 
            flush_after=True
        )
        if not self.first_batch_data:
            self.stop_server()
            return False
            
        # 步骤 4: 写入第二批数据（不刷盘）
        second_batch_metrics = [
            ("disk_io", "server-01", 1234.5),
            ("disk_io", "server-02", 2345.6),
            ("network_io", "server-01", 5678.9),
            ("network_io", "server-02", 6789.0),
        ]
        self.second_batch_data = self.write_data_batch(
            "第二批数据 (未刷盘)", 
            second_batch_metrics, 
            flush_after=False
        )
        if not self.second_batch_data:
            self.stop_server()
            return False
            
        # 步骤 5: 查询数据（重启前）
        results_before = self.query_data("(重启前)")
        
        # 步骤 6: 停止服务器
        self.stop_server()
        
        # 等待一段时间
        print("\n等待 3 秒...")
        time.sleep(3)
        
        # 步骤 7: 重新启动服务器
        if not self.start_server():
            return False
            
        # 步骤 8: 查询数据（重启后）
        results_after = self.query_data("(重启后)")
        
        # 步骤 9: 验证数据完整性
        integrity_ok, first_ok, first_total, second_ok, second_total = self.verify_data_integrity(
            results_before, results_after
        )
        
        # 清理
        self.stop_server()
        
        # 输出测试结果
        print("\n" + "=" * 70)
        print("测试结果")
        print("=" * 70)
        
        if integrity_ok:
            print("✅ 增强数据持久化测试通过！")
            print("   所有数据在服务器重启后仍然完整可用")
            print(f"   第一批数据 (已刷盘): {first_ok}/{first_total} 通过")
            print(f"   第二批数据 (未刷盘): {second_ok}/{second_total} 通过")
            return True
        else:
            print("❌ 增强数据持久化测试失败！")
            print("   部分数据在服务器重启后出现丢失或不一致")
            print(f"   第一批数据 (已刷盘): {first_ok}/{first_total} 通过")
            print(f"   第二批数据 (未刷盘): {second_ok}/{second_total} 通过")
            return False

if __name__ == "__main__":
    test = EnhancedDataPersistenceTest()
    success = test.run_test()
    sys.exit(0 if success else 1)
