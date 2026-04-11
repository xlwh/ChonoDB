#!/usr/bin/env python3
"""
ChronoDB 数据持久化测试
验证数据写入后，服务重启后数据仍然可以正常查询且不会丢失
"""

import requests
import json
import time
import subprocess
import os
import signal
import sys
from datetime import datetime

class DataPersistenceTest:
    def __init__(self):
        self.base_url = "http://localhost:9090"
        self.data_dir = "/Users/zhb/chronodb_test_data"
        self.server_process = None
        self.test_data = []
        
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
        env["CHRONODB_DATA_DIR"] = self.data_dir
        env["RUST_LOG"] = "info,chronodb=debug"
        
        # 使用已编译的二进制文件
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
            
    def write_test_data(self):
        """写入测试数据"""
        print("\n=== 写入测试数据 ===")
        
        # 生成测试数据（使用毫秒时间戳）
        current_time_ms = int(time.time() * 1000)
        self.write_timestamp = int(time.time())  # 保存秒级时间戳用于查询
        
        url = f"{self.base_url}/api/v1/write"
        headers = {"Content-Type": "text/plain"}
        
        metrics = [
            ("cpu_usage", "server-01", 45.5),
            ("cpu_usage", "server-02", 67.2),
            ("memory_usage", "server-01", 78.3),
            ("memory_usage", "server-02", 82.1),
            ("disk_io", "server-01", 1234.5),
            ("disk_io", "server-02", 2345.6),
            ("network_io", "server-01", 5678.9),
            ("network_io", "server-02", 6789.0),
        ]
        
        for metric_name, instance, value in metrics:
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
                    self.test_data.append({
                        "metric": metric_name,
                        "instance": instance,
                        "value": value,
                        "timestamp": current_time_ms
                    })
                    print(f"✓ 写入 {metric_name} [{instance}] = {value}")
                else:
                    print(f"❌ 写入失败: {response.status_code}")
                    return False
            except Exception as e:
                print(f"❌ 写入异常: {e}")
                return False
                
        print(f"✓ 成功写入 {len(self.test_data)} 条测试数据")
        
        # 触发刷盘操作，确保数据持久化
        print("\n触发数据刷盘...")
        try:
            response = requests.post(f"{self.base_url}/api/v1/admin/flush", timeout=10)
            if response.status_code in [200, 204]:
                print("✓ 刷盘操作成功")
            else:
                print(f"⚠ 刷盘操作返回: {response.status_code}")
        except Exception as e:
            print(f"⚠ 刷盘操作异常: {e}")
        
        # 等待数据写入完成
        print("等待数据写入完成...")
        time.sleep(2)
        
        return True
        
    def query_data(self, phase=""):
        """查询数据"""
        print(f"\n=== 查询数据 {phase} ===")
        
        results = {}
        for item in self.test_data:
            metric = item["metric"]
            instance = item["instance"]
            expected_value = item["value"]
            
            query = f'{metric}{{instance="{instance}"}}'
            
            try:
                # 使用 range query 查询时间范围（使用秒级时间戳）
                # 使用写入时保存的时间戳，前后扩展60秒
                end_time = self.write_timestamp + 60
                start_time = self.write_timestamp - 60
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
                                results[f"{metric}_{instance}"] = {
                                    "expected": expected_value,
                                    "actual": actual_value,
                                    "match": abs(actual_value - expected_value) < 0.01
                                }
                                status = "✓" if results[f"{metric}_{instance}"]["match"] else "❌"
                                print(f"{status} {metric} [{instance}]: 期望={expected_value}, 实际={actual_value}")
                            else:
                                print(f"❌ {metric} [{instance}]: 返回结果中无 values")
                                print(f"   响应内容: {data}")
                                results[f"{metric}_{instance}"] = {
                                    "expected": expected_value,
                                    "actual": None,
                                    "match": False
                                }
                        else:
                            print(f"❌ {metric} [{instance}]: 未返回数据")
                            print(f"   响应内容: {data}")
                            results[f"{metric}_{instance}"] = {
                                "expected": expected_value,
                                "actual": None,
                                "match": False
                            }
                    else:
                        print(f"❌ {metric} [{instance}]: 查询失败 - {data}")
                        results[f"{metric}_{instance}"] = {
                            "expected": expected_value,
                            "actual": None,
                            "match": False
                        }
                else:
                    print(f"❌ {metric} [{instance}]: HTTP {response.status_code}")
                    results[f"{metric}_{instance}"] = {
                        "expected": expected_value,
                        "actual": None,
                        "match": False
                    }
            except Exception as e:
                print(f"❌ {metric} [{instance}]: 查询异常 - {e}")
                results[f"{metric}_{instance}"] = {
                    "expected": expected_value,
                    "actual": None,
                    "match": False
                }
                
        return results
        
    def verify_data_integrity(self, results_before, results_after):
        """验证数据完整性"""
        print("\n=== 验证数据完整性 ===")
        
        all_match = True
        for key in results_before:
            if key not in results_after:
                print(f"❌ {key}: 重启后数据丢失")
                all_match = False
            elif not results_after[key]["match"]:
                print(f"❌ {key}: 数据不匹配")
                print(f"   重启前: {results_before[key]['actual']}")
                print(f"   重启后: {results_after[key]['actual']}")
                all_match = False
            else:
                print(f"✓ {key}: 数据一致")
                
        return all_match
        
    def run_test(self):
        """运行完整测试"""
        print("=" * 60)
        print("ChronoDB 数据持久化测试")
        print("=" * 60)
        
        # 步骤 1: 设置数据目录
        self.setup_data_dir()
        
        # 步骤 2: 启动服务器
        if not self.start_server():
            return False
            
        # 步骤 3: 写入测试数据
        if not self.write_test_data():
            self.stop_server()
            return False
            
        # 步骤 4: 查询数据（重启前）
        results_before = self.query_data("(重启前)")
        
        # 步骤 5: 停止服务器
        self.stop_server()
        
        # 等待一段时间
        print("\n等待 3 秒...")
        time.sleep(3)
        
        # 步骤 6: 重新启动服务器
        if not self.start_server():
            return False
            
        # 步骤 7: 查询数据（重启后）
        results_after = self.query_data("(重启后)")
        
        # 步骤 8: 验证数据完整性
        integrity_ok = self.verify_data_integrity(results_before, results_after)
        
        # 清理
        self.stop_server()
        
        # 输出测试结果
        print("\n" + "=" * 60)
        print("测试结果")
        print("=" * 60)
        
        if integrity_ok:
            print("✅ 数据持久化测试通过！")
            print("   数据在服务器重启后仍然完整可用")
            return True
        else:
            print("❌ 数据持久化测试失败！")
            print("   数据在服务器重启后出现丢失或不一致")
            return False

if __name__ == "__main__":
    test = DataPersistenceTest()
    success = test.run_test()
    sys.exit(0 if success else 1)
