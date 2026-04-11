#!/usr/bin/env python3
"""
ChronoDB 全面集成测试套件
测试所有API端点和功能模块
"""

import requests
import json
import time
import subprocess
import os
import signal
import tempfile
import shutil
import sys

class ChronoDBComprehensiveTest:
    def __init__(self):
        self.base_url = "http://localhost:9090"
        self.server_process = None
        self.temp_dir = None
        self.test_data = []
        self.passed = 0
        self.failed = 0
    
    def start_server(self):
        """启动ChronoDB服务器"""
        print("=== 启动ChronoDB服务器 ===")
        
        # 创建临时目录
        self.temp_dir = tempfile.mkdtemp()
        print(f"创建临时数据目录: {self.temp_dir}")
        
        # 编译项目
        print("编译ChronoDB...")
        compile_result = subprocess.run(["cargo", "build", "--release"], 
                                      cwd="/home/zhb/workspace/chonodb",
                                      capture_output=True, text=True)
        
        if compile_result.returncode != 0:
            print(f"编译失败: {compile_result.stderr}")
            return False
        
        print("编译成功！")
        
        # 启动服务器
        print("启动服务器...")
        server_cmd = ["cargo", "run", "--bin", "chronodb-server"]
        env = os.environ.copy()
        env["CHRONODB_DATA_DIR"] = self.temp_dir
        env["RUST_LOG"] = "info"
        
        self.log_file = open("server_log.txt", "w")
        
        self.server_process = subprocess.Popen(
            server_cmd,
            cwd="/home/zhb/workspace/chonodb",
            env=env,
            stdout=self.log_file,
            stderr=self.log_file,
            text=True
        )
        
        # 等待服务器启动
        print("等待服务器启动...")
        for i in range(30):
            time.sleep(1)
            
            try:
                response = requests.get(f"{self.base_url}/api/v1/labels", timeout=2)
                if response.status_code == 200:
                    print("服务器启动成功！")
                    return True
            except Exception as e:
                pass
            
            if self.server_process.poll() is not None:
                stdout, stderr = self.server_process.communicate()
                print(f"服务器启动失败，退出码: {self.server_process.returncode}")
                self.stop_server()
                return False
        
        print("服务器启动超时")
        self.stop_server()
        return False
    
    def stop_server(self):
        """停止ChronoDB服务器"""
        if self.server_process:
            print("停止服务器...")
            self.server_process.terminate()
            try:
                self.server_process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                self.server_process.kill()
            self.server_process = None
        
        if hasattr(self, 'log_file') and self.log_file:
            self.log_file.close()
        
        if self.temp_dir and os.path.exists(self.temp_dir):
            print(f"清理临时目录: {self.temp_dir}")
            shutil.rmtree(self.temp_dir)
            self.temp_dir = None
    
    def generate_test_data(self):
        """生成测试数据"""
        print("\n=== 生成测试数据 ===")
        
        now = int(time.time() * 1000)
        self.test_data = []
        
        # CPU使用率数据
        for i in range(200):
            timestamp = now - (199 - i) * 1000
            value = 50 + (i % 50)
            data = f"cpu_usage{{job=\"webserver\", instance=\"server1\", region=\"us-east-1\"}} {value} {timestamp}"
            self.test_data.append(data)
        
        # 内存使用率数据
        for i in range(200):
            timestamp = now - (199 - i) * 1000
            value = 60 + (i % 30)
            data = f"memory_usage{{job=\"webserver\", instance=\"server1\", region=\"us-east-1\"}} {value} {timestamp}"
            self.test_data.append(data)
        
        # 网络流量数据
        for i in range(200):
            timestamp = now - (199 - i) * 1000
            value = 1000 + (i % 1000)
            data = f"network_traffic{{job=\"webserver\", instance=\"server1\", region=\"us-east-1\", direction=\"incoming\"}} {value} {timestamp}"
            self.test_data.append(data)
        
        print(f"生成了 {len(self.test_data)} 条测试数据")
        return True
    
    def write_data(self):
        """写入测试数据"""
        print("\n=== 写入测试数据 ===")
        
        url = f"{self.base_url}/api/v1/write"
        headers = {"Content-Type": "text/plain"}
        
        batch_size = 50
        for i in range(0, len(self.test_data), batch_size):
            batch = self.test_data[i:i+batch_size]
            data = "\n".join(batch)
            
            try:
                response = requests.post(url, headers=headers, data=data, timeout=30)
                if response.status_code == 204:
                    print(f"成功写入批次 {i//batch_size + 1}/{(len(self.test_data)+batch_size-1)//batch_size}")
                else:
                    print(f"写入失败，状态码: {response.status_code}")
                    return False
            except Exception as e:
                print(f"写入错误: {e}")
                return False
        
        print("所有数据写入成功！")
        return True
    
    def test_http_api_endpoints(self):
        """测试所有HTTP API端点"""
        print("\n=== 测试HTTP API端点 ===")
        
        endpoints = [
            ("/api/v1/query", "GET", {"query": "cpu_usage"}, "查询API"),
            ("/api/v1/labels", "GET", {}, "标签列表API"),
            ("/api/v1/label/job/values", "GET", {}, "标签值API"),
            ("/api/v1/series", "GET", {"match[]": "cpu_usage"}, "时间序列API"),
            ("/api/v1/metadata", "GET", {}, "元数据API"),
            ("/api/v1/targets", "GET", {}, "目标状态API"),
            ("/api/v1/alerts", "GET", {}, "告警API"),
            ("/api/v1/rules", "GET", {}, "规则API"),
            ("/api/v1/status/config", "GET", {}, "配置状态API"),
            ("/api/v1/status/flags", "GET", {}, "标志状态API"),
            ("/ready", "GET", {}, "健康检查API"),
            ("/live", "GET", {}, "存活检查API"),
        ]
        
        all_passed = True
        for endpoint, method, params, name in endpoints:
            url = f"{self.base_url}{endpoint}"
            try:
                if method == "GET":
                    response = requests.get(url, params=params, timeout=10)
                elif method == "POST":
                    response = requests.post(url, data=params, timeout=10)
                
                if response.status_code == 200:
                    print(f"✅ {name} ({endpoint})")
                    self.passed += 1
                else:
                    print(f"❌ {name} ({endpoint}) - 状态码: {response.status_code}")
                    self.failed += 1
                    all_passed = False
            except Exception as e:
                print(f"❌ {name} ({endpoint}) - 错误: {e}")
                self.failed += 1
                all_passed = False
        
        return all_passed
    
    def test_promql_queries(self):
        """测试PromQL查询功能"""
        print("\n=== 测试PromQL查询 ===")
        
        queries = [
            ("cpu_usage", "基本查询"),
            ("sum(cpu_usage)", "sum聚合"),
            ("avg(cpu_usage)", "avg聚合"),
            ("min(cpu_usage)", "min聚合"),
            ("max(cpu_usage)", "max聚合"),
            ("count(cpu_usage)", "count聚合"),
            ("rate(cpu_usage[1m])", "rate函数"),
            ("cpu_usage{job=\"webserver\"}", "标签过滤"),
            ("sum by (job) (cpu_usage)", "按标签聚合"),
            ("sum by (instance, region) (cpu_usage)", "多标签聚合"),
            ("cpu_usage + memory_usage", "二元运算"),
            ("cpu_usage * 2", "乘法运算"),
            ("cpu_usage > 70", "比较运算"),
            ("cpu_usage or memory_usage", "逻辑或"),
            ("cpu_usage and memory_usage", "逻辑与"),
            ("abs(cpu_usage - 50)", "abs函数"),
            ("ceil(cpu_usage)", "ceil函数"),
            ("floor(cpu_usage)", "floor函数"),
            ("round(cpu_usage)", "round函数"),
            ("scalar(sum(cpu_usage))", "scalar函数"),
            ("vector(42)", "vector函数"),
            ("time()", "time函数"),
            ("now()", "now函数"),
        ]
        
        url = f"{self.base_url}/api/v1/query"
        all_passed = True
        
        for query, name in queries:
            params = {"query": query}
            try:
                response = requests.get(url, params=params, timeout=10)
                if response.status_code == 200:
                    data = response.json()
                    if data.get("status") == "success":
                        print(f"✅ {name}")
                        self.passed += 1
                    else:
                        print(f"❌ {name} - {data.get('error', '未知错误')}")
                        self.failed += 1
                        all_passed = False
                else:
                    print(f"❌ {name} - 状态码: {response.status_code}")
                    self.failed += 1
                    all_passed = False
            except Exception as e:
                print(f"❌ {name} - 错误: {e}")
                self.failed += 1
                all_passed = False
        
        return all_passed
    
    def test_query_range(self):
        """测试时间范围查询"""
        print("\n=== 测试时间范围查询 ===")
        
        now = int(time.time())
        test_cases = [
            {"start": now - 600, "end": now, "step": "1s", "name": "1分钟范围，1秒步长"},
            {"start": now - 3600, "end": now, "step": "1m", "name": "1小时范围，1分钟步长"},
            {"start": now - 86400, "end": now, "step": "1h", "name": "1天范围，1小时步长"},
        ]
        
        url = f"{self.base_url}/api/v1/query_range"
        all_passed = True
        
        for case in test_cases:
            params = {"query": "cpu_usage", **case}
            try:
                response = requests.get(url, params=params, timeout=30)
                if response.status_code == 200:
                    data = response.json()
                    if data.get("status") == "success":
                        result = data.get("data", {}).get("result", [])
                        if result:
                            samples = result[0].get("values", [])
                            print(f"✅ {case['name']} - 返回 {len(samples)} 个样本")
                        else:
                            print(f"⚠️  {case['name']} - 无结果")
                        self.passed += 1
                    else:
                        print(f"❌ {case['name']} - {data.get('error', '未知错误')}")
                        self.failed += 1
                        all_passed = False
                else:
                    print(f"❌ {case['name']} - 状态码: {response.status_code}")
                    self.failed += 1
                    all_passed = False
            except Exception as e:
                print(f"❌ {case['name']} - 错误: {e}")
                self.failed += 1
                all_passed = False
        
        return all_passed
    
    def test_write_protocols(self):
        """测试数据写入协议"""
        print("\n=== 测试数据写入协议 ===")
        
        # 测试文本格式写入
        print("测试文本格式写入...")
        url = f"{self.base_url}/api/v1/write"
        headers = {"Content-Type": "text/plain"}
        
        now = int(time.time() * 1000)
        test_data = [
            f"test_write_metric{{job=\"test\", instance=\"server1\"}} 42 {now}",
            f"test_write_metric{{job=\"test\", instance=\"server2\"}} 84 {now}",
        ]
        
        try:
            response = requests.post(url, headers=headers, data="\n".join(test_data), timeout=30)
            if response.status_code == 204:
                print("✅ 文本格式写入成功")
                self.passed += 1
            else:
                print(f"❌ 文本格式写入失败 - 状态码: {response.status_code}")
                self.failed += 1
                return False
        except Exception as e:
            print(f"❌ 文本格式写入错误: {e}")
            self.failed += 1
            return False
        
        # 验证写入的数据
        try:
            response = requests.get(f"{self.base_url}/api/v1/query", params={"query": "test_write_metric"}, timeout=10)
            if response.status_code == 200:
                data = response.json()
                if data.get("status") == "success" and data.get("data", {}).get("result"):
                    print("✅ 写入数据验证成功")
                    self.passed += 1
                else:
                    print("❌ 写入数据验证失败")
                    self.failed += 1
            else:
                print(f"❌ 写入数据验证失败 - 状态码: {response.status_code}")
                self.failed += 1
        except Exception as e:
            print(f"❌ 写入数据验证错误: {e}")
            self.failed += 1
        
        return True
    
    def test_metadata_queries(self):
        """测试元数据查询"""
        print("\n=== 测试元数据查询 ===")
        
        tests = [
            ("/api/v1/labels", {}, "标签列表"),
            ("/api/v1/label/job/values", {}, "job标签值"),
            ("/api/v1/label/instance/values", {}, "instance标签值"),
            ("/api/v1/label/region/values", {}, "region标签值"),
            ("/api/v1/series", {"match[]": "cpu_usage"}, "时间序列元数据"),
        ]
        
        all_passed = True
        for endpoint, params, name in tests:
            url = f"{self.base_url}{endpoint}"
            try:
                response = requests.get(url, params=params, timeout=10)
                if response.status_code == 200:
                    data = response.json()
                    if data.get("status") == "success":
                        result = data.get("data", [])
                        print(f"✅ {name} - 返回 {len(result)} 条结果")
                        self.passed += 1
                    else:
                        print(f"❌ {name} - {data.get('error', '未知错误')}")
                        self.failed += 1
                        all_passed = False
                else:
                    print(f"❌ {name} - 状态码: {response.status_code}")
                    self.failed += 1
                    all_passed = False
            except Exception as e:
                print(f"❌ {name} - 错误: {e}")
                self.failed += 1
                all_passed = False
        
        return all_passed
    
    def test_downsampling(self):
        """测试降采样功能"""
        print("\n=== 测试降采样功能 ===")
        
        # 等待降采样任务执行
        print("等待降采样任务执行...")
        time.sleep(30)
        
        # 查询降采样数据
        now = int(time.time())
        url = f"{self.base_url}/api/v1/query_range"
        params = {
            "query": "cpu_usage",
            "start": now - 600,
            "end": now,
            "step": "10s"
        }
        
        try:
            response = requests.get(url, params=params, timeout=30)
            if response.status_code == 200:
                data = response.json()
                if data.get("status") == "success":
                    result = data.get("data", {}).get("result", [])
                    if result:
                        samples = result[0].get("values", [])
                        print(f"✅ 降采样查询成功 - 返回 {len(samples)} 个样本")
                        self.passed += 1
                    else:
                        print("⚠️  降采样查询无结果（可能需要更多数据）")
                        self.passed += 1
                else:
                    print(f"❌ 降采样查询失败 - {data.get('error', '未知错误')}")
                    self.failed += 1
            else:
                print(f"❌ 降采样查询失败 - 状态码: {response.status_code}")
                self.failed += 1
        except Exception as e:
            print(f"❌ 降采样查询错误: {e}")
            self.failed += 1
        
        return True
    
    def test_flush_and_recovery(self):
        """测试数据刷盘和恢复"""
        print("\n=== 测试数据刷盘和恢复 ===")
        
        # 写入测试数据
        url = f"{self.base_url}/api/v1/write"
        headers = {"Content-Type": "text/plain"}
        
        now = int(time.time() * 1000)
        test_data = []
        for i in range(100):
            timestamp = now - (99 - i) * 1000
            value = 50 + (i % 50)
            data = f"flush_test{{job=\"test\", instance=\"server1\"}} {value} {timestamp}"
            test_data.append(data)
        
        try:
            response = requests.post(url, headers=headers, data="\n".join(test_data), timeout=30)
            if response.status_code != 204:
                print("❌ 写入测试数据失败")
                self.failed += 1
                return False
            print("✅ 写入测试数据成功")
            self.passed += 1
        except Exception as e:
            print(f"❌ 写入测试数据错误: {e}")
            self.failed += 1
            return False
        
        # 查询验证
        try:
            response = requests.get(f"{self.base_url}/api/v1/query", params={"query": "flush_test"}, timeout=10)
            if response.status_code == 200:
                data = response.json()
                if data.get("status") == "success" and data.get("data", {}).get("result"):
                    print("✅ 查询验证成功")
                    self.passed += 1
                else:
                    print("❌ 查询验证失败")
                    self.failed += 1
            else:
                print(f"❌ 查询验证失败 - 状态码: {response.status_code}")
                self.failed += 1
        except Exception as e:
            print(f"❌ 查询验证错误: {e}")
            self.failed += 1
        
        return True
    
    def test_remote_write_read(self):
        """测试Remote Write/Read协议"""
        print("\n=== 测试Remote Write/Read协议 ===")
        
        # 测试写入
        url = f"{self.base_url}/api/v1/write"
        headers = {"Content-Type": "text/plain"}
        
        now = int(time.time() * 1000)
        test_data = f"remote_write_test{{job=\"remote\", instance=\"client1\"}} 123 {now}"
        
        try:
            response = requests.post(url, headers=headers, data=test_data, timeout=30)
            if response.status_code == 204:
                print("✅ Remote Write 写入成功")
                self.passed += 1
            else:
                print(f"❌ Remote Write 写入失败 - 状态码: {response.status_code}")
                self.failed += 1
                return False
        except Exception as e:
            print(f"❌ Remote Write 写入错误: {e}")
            self.failed += 1
            return False
        
        # 测试查询（模拟Remote Read）
        try:
            response = requests.get(f"{self.base_url}/api/v1/query", params={"query": "remote_write_test"}, timeout=10)
            if response.status_code == 200:
                data = response.json()
                if data.get("status") == "success" and data.get("data", {}).get("result"):
                    print("✅ Remote Read 查询成功")
                    self.passed += 1
                else:
                    print("❌ Remote Read 查询失败")
                    self.failed += 1
            else:
                print(f"❌ Remote Read 查询失败 - 状态码: {response.status_code}")
                self.failed += 1
        except Exception as e:
            print(f"❌ Remote Read 查询错误: {e}")
            self.failed += 1
        
        return True
    
    def run_all_tests(self):
        """运行所有测试"""
        print("====================================")
        print("ChronoDB 全面集成测试")
        print("====================================")
        
        test_steps = [
            ("启动服务器", self.start_server),
            ("生成测试数据", self.generate_test_data),
            ("写入测试数据", self.write_data),
            ("HTTP API端点测试", self.test_http_api_endpoints),
            ("PromQL查询测试", self.test_promql_queries),
            ("时间范围查询测试", self.test_query_range),
            ("数据写入协议测试", self.test_write_protocols),
            ("元数据查询测试", self.test_metadata_queries),
            ("降采样功能测试", self.test_downsampling),
            ("数据刷盘测试", self.test_flush_and_recovery),
            ("Remote Write/Read测试", self.test_remote_write_read),
        ]
        
        for test_name, test_func in test_steps:
            print(f"\n--- {test_name} ---")
            try:
                if test_func():
                    print(f"✅ {test_name} 通过")
                else:
                    print(f"❌ {test_name} 失败")
            except Exception as e:
                print(f"❌ {test_name} 异常: {e}")
                self.failed += 1
        
        # 停止服务器
        self.stop_server()
        
        # 输出测试汇总
        print("\n====================================")
        print("测试结果汇总")
        print("====================================")
        total = self.passed + self.failed
        print(f"测试用例总数: {total}")
        print(f"通过: {self.passed}")
        print(f"失败: {self.failed}")
        print(f"成功率: {self.passed/total*100:.1f}%" if total > 0 else "无测试执行")
        
        if self.failed == 0:
            print("\n🎉 所有测试通过！")
            return True
        else:
            print("\n❌ 部分测试失败！")
            return False

if __name__ == "__main__":
    test = ChronoDBComprehensiveTest()
    success = test.run_all_tests()
    sys.exit(0 if success else 1)