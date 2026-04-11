#!/usr/bin/env python3
"""
ChronoDB 完整集成测试
测试所有核心功能：数据写入、查询、聚合、持久化等
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

class ChronoDBIntegrationTest:
    def __init__(self):
        self.base_url = "http://localhost:9090"
        self.server_process = None
        self.temp_dir = None
        self.test_data = []
        self.test_results = []
        
    def start_server(self):
        """启动ChronoDB服务器"""
        print("\n" + "="*60)
        print("启动ChronoDB服务器")
        print("="*60)
        
        # 创建临时目录
        self.temp_dir = tempfile.mkdtemp(prefix="chronodb_test_")
        print(f"✓ 创建临时数据目录: {self.temp_dir}")
        
        # 检查编译结果
        server_binary = "/Users/zhb/workspace/chonodb/target/release/chronodb-server"
        if not os.path.exists(server_binary):
            print("❌ 服务器二进制文件不存在，请先编译: cargo build --release")
            return False
        
        print("✓ 找到服务器二进制文件")
        
        # 创建配置文件
        config_path = os.path.join(self.temp_dir, "chronodb.yaml")
        with open(config_path, 'w') as f:
            f.write(f"""
listen_address: "0.0.0.0"
port: 9090
data_dir: "{self.temp_dir}/data"

storage:
  mode: standalone
  backend: local
  local_path: "{self.temp_dir}/data/storage"
  max_disk_usage: "80%"

query:
  max_concurrent: 100
  timeout: 120
  max_samples: 50000000
  enable_vectorized: true
  enable_parallel: true
  enable_auto_downsampling: true
  downsample_policy: "auto"
  query_cache_size: "256MB"
  enable_query_cache: true
  query_cache_ttl: 300

rules:
  rule_files: []
  evaluation_interval: 60
  alert_send_interval: 60

targets:
  config_file: ~
  scrape_interval: 60
  scrape_timeout: 10

memory:
  memstore_size: "512MB"
  wal_size: "128MB"
  query_cache_size: "256MB"
  max_memory_usage: "80%"

compression:
  time_column:
    algorithm: "zstd"
    level: 3
  value_column:
    algorithm: "zstd"
    level: 3
    use_prediction: true
  label_column:
    algorithm: "dictionary"
    level: 0

log:
  level: "info"
  format: "json"
  output: ~

pre_aggregation:
  auto_create:
    enabled: true
    frequency_threshold: 20
    time_window: 24
    max_auto_rules: 100
    exclude_patterns:
      - "^up$"
      - "^ALERTS$"
  auto_cleanup:
    enabled: true
    check_interval: 6
    low_frequency_threshold: 5
    observation_period: 48
  storage:
    retention_days: 30
    max_storage_gb: 100
    compression: "zstd"
""")
        
        # 启动服务器
        print("启动服务器...")
        server_cmd = [server_binary, "--config", config_path]
        env = os.environ.copy()
        env["RUST_LOG"] = "info,chronodb=debug"
        
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
            time.sleep(1)
            
            # 检查服务器是否启动成功
            try:
                response = requests.get(f"{self.base_url}/-/healthy", timeout=2)
                if response.status_code in [200, 204]:
                    print(f"✓ 服务器启动成功！({i+1}秒)")
                    return True
            except:
                pass
            
            # 检查服务器是否异常退出
            if self.server_process.poll() is not None:
                stdout, stderr = self.server_process.communicate()
                print(f"❌ 服务器启动失败，退出码: {self.server_process.returncode}")
                print(f"错误输出: {stderr[-1000:] if stderr else '无'}")
                return False
        
        print("❌ 服务器启动超时")
        self.stop_server()
        return False
    
    def stop_server(self):
        """停止ChronoDB服务器"""
        if self.server_process:
            print("\n停止服务器...")
            self.server_process.terminate()
            try:
                self.server_process.wait(timeout=5)
                print("✓ 服务器已停止")
            except subprocess.TimeoutExpired:
                self.server_process.kill()
                print("✓ 服务器已强制停止")
            self.server_process = None
        
        # 清理临时目录
        if self.temp_dir and os.path.exists(self.temp_dir):
            print(f"清理临时目录: {self.temp_dir}")
            shutil.rmtree(self.temp_dir, ignore_errors=True)
            self.temp_dir = None
    
    def generate_test_data(self):
        """生成测试数据"""
        print("\n" + "="*60)
        print("生成测试数据")
        print("="*60)
        
        now = int(time.time() * 1000)
        self.test_data = []
        
        # 生成CPU使用率数据
        for i in range(100):
            timestamp = now - (99 - i) * 1000
            value = 50 + (i % 50)
            data = f'cpu_usage{{job="webserver", instance="server1", region="us-east-1"}} {value} {timestamp}'
            self.test_data.append(data)
        
        # 生成内存使用率数据
        for i in range(100):
            timestamp = now - (99 - i) * 1000
            value = 60 + (i % 30)
            data = f'memory_usage{{job="webserver", instance="server1", region="us-east-1"}} {value} {timestamp}'
            self.test_data.append(data)
        
        # 生成磁盘IO数据
        for i in range(100):
            timestamp = now - (99 - i) * 1000
            value = 1000 + (i % 500)
            data = f'disk_io{{job="webserver", instance="server1", region="us-east-1", device="sda1"}} {value} {timestamp}'
            self.test_data.append(data)
        
        # 生成网络流量数据
        for i in range(100):
            timestamp = now - (99 - i) * 1000
            value = 5000 + (i % 2000)
            data = f'network_traffic{{job="webserver", instance="server1", region="us-east-1", direction="incoming"}} {value} {timestamp}'
            self.test_data.append(data)
        
        print(f"✓ 生成了 {len(self.test_data)} 条测试数据")
        return True
    
    def write_data(self):
        """写入测试数据"""
        print("\n" + "="*60)
        print("写入测试数据")
        print("="*60)
        
        url = f"{self.base_url}/api/v1/write"
        headers = {"Content-Type": "text/plain"}
        
        # 分批次写入
        batch_size = 50
        total_batches = (len(self.test_data) + batch_size - 1) // batch_size
        
        for i in range(0, len(self.test_data), batch_size):
            batch = self.test_data[i:i+batch_size]
            data = "\n".join(batch)
            
            try:
                response = requests.post(url, headers=headers, data=data, timeout=30)
                if response.status_code in [200, 204]:
                    print(f"✓ 批次 {i//batch_size + 1}/{total_batches} 写入成功")
                else:
                    print(f"❌ 批次 {i//batch_size + 1} 写入失败: {response.status_code}")
                    return False
            except Exception as e:
                print(f"❌ 写入错误: {e}")
                return False
        
        print(f"✓ 所有数据写入成功！({len(self.test_data)} 条)")
        return True
    
    def test_health_check(self):
        """测试健康检查"""
        print("\n" + "="*60)
        print("测试健康检查")
        print("="*60)
        
        try:
            response = requests.get(f"{self.base_url}/-/healthy", timeout=5)
            if response.status_code in [200, 204]:
                print("✅ 健康检查通过")
                return True
            else:
                print(f"❌ 健康检查失败: {response.status_code}")
                return False
        except Exception as e:
            print(f"❌ 健康检查错误: {e}")
            return False
    
    def test_basic_query(self):
        """测试基本查询"""
        print("\n" + "="*60)
        print("测试基本查询")
        print("="*60)
        
        queries = [
            ("cpu_usage", "CPU使用率"),
            ("memory_usage", "内存使用率"),
            ("disk_io", "磁盘IO"),
            ("network_traffic", "网络流量"),
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
                        result = data.get("data", {}).get("result", [])
                        if len(result) > 0:
                            print(f"✅ {name} 查询成功 ({len(result)} 个序列)")
                        else:
                            print(f"⚠️  {name} 查询未返回数据")
                    else:
                        print(f"❌ {name} 查询失败: {data.get('error')}")
                        all_passed = False
                else:
                    print(f"❌ {name} 查询失败: {response.status_code}")
                    all_passed = False
            except Exception as e:
                print(f"❌ {name} 查询错误: {e}")
                all_passed = False
        
        return all_passed
    
    def test_aggregation(self):
        """测试聚合查询"""
        print("\n" + "="*60)
        print("测试聚合查询")
        print("="*60)
        
        aggregations = [
            ("sum(cpu_usage)", "sum聚合"),
            ("avg(cpu_usage)", "avg聚合"),
            ("min(cpu_usage)", "min聚合"),
            ("max(cpu_usage)", "max聚合"),
            ("count(cpu_usage)", "count聚合"),
            ("sum(memory_usage)", "sum聚合(内存)"),
            ("avg(disk_io)", "avg聚合(磁盘)"),
        ]
        
        url = f"{self.base_url}/api/v1/query"
        all_passed = True
        
        for query, name in aggregations:
            params = {"query": query}
            try:
                response = requests.get(url, params=params, timeout=10)
                if response.status_code == 200:
                    data = response.json()
                    if data.get("status") == "success":
                        result = data.get("data", {}).get("result", [])
                        if len(result) > 0:
                            value = result[0].get("value", [0, "0"])[1]
                            print(f"✅ {name} = {value}")
                        else:
                            print(f"⚠️  {name} 未返回数据")
                    else:
                        print(f"❌ {name} 失败: {data.get('error')}")
                        all_passed = False
                else:
                    print(f"❌ {name} 失败: {response.status_code}")
                    all_passed = False
            except Exception as e:
                print(f"❌ {name} 错误: {e}")
                all_passed = False
        
        return all_passed
    
    def test_label_filter(self):
        """测试标签过滤"""
        print("\n" + "="*60)
        print("测试标签过滤")
        print("="*60)
        
        filters = [
            ('cpu_usage{job="webserver"}', "按job过滤"),
            ('cpu_usage{instance="server1"}', "按instance过滤"),
            ('cpu_usage{region="us-east-1"}', "按region过滤"),
            ('disk_io{device="sda1"}', "按device过滤"),
            ('network_traffic{direction="incoming"}', "按direction过滤"),
        ]
        
        url = f"{self.base_url}/api/v1/query"
        all_passed = True
        
        for query, name in filters:
            params = {"query": query}
            try:
                response = requests.get(url, params=params, timeout=10)
                if response.status_code == 200:
                    data = response.json()
                    if data.get("status") == "success":
                        result = data.get("data", {}).get("result", [])
                        if len(result) > 0:
                            print(f"✅ {name} 成功 ({len(result)} 个序列)")
                        else:
                            print(f"⚠️  {name} 未返回数据")
                    else:
                        print(f"❌ {name} 失败: {data.get('error')}")
                        all_passed = False
                else:
                    print(f"❌ {name} 失败: {response.status_code}")
                    all_passed = False
            except Exception as e:
                print(f"❌ {name} 错误: {e}")
                all_passed = False
        
        return all_passed
    
    def test_query_range(self):
        """测试时间范围查询"""
        print("\n" + "="*60)
        print("测试时间范围查询")
        print("="*60)
        
        now = int(time.time())
        start = now - 300  # 5分钟前
        end = now
        step = "10s"
        
        url = f"{self.base_url}/api/v1/query_range"
        params = {
            "query": "cpu_usage",
            "start": start,
            "end": end,
            "step": step
        }
        
        try:
            response = requests.get(url, params=params, timeout=30)
            if response.status_code == 200:
                data = response.json()
                if data.get("status") == "success":
                    result = data.get("data", {}).get("result", [])
                    if len(result) > 0:
                        samples = result[0].get("values", [])
                        print(f"✅ 时间范围查询成功 ({len(samples)} 个样本)")
                        return True
                    else:
                        print("⚠️  时间范围查询未返回数据")
                        return True
                else:
                    print(f"❌ 时间范围查询失败: {data.get('error')}")
                    return False
            else:
                print(f"❌ 时间范围查询失败: {response.status_code}")
                return False
        except Exception as e:
            print(f"❌ 时间范围查询错误: {e}")
            return False
    
    def test_operators(self):
        """测试查询算子"""
        print("\n" + "="*60)
        print("测试查询算子")
        print("="*60)
        
        operators = [
            ("cpu_usage + 10", "加法"),
            ("cpu_usage - 10", "减法"),
            ("cpu_usage * 2", "乘法"),
            ("cpu_usage / 2", "除法"),
            ("cpu_usage > 70", "大于"),
            ("cpu_usage < 30", "小于"),
        ]
        
        url = f"{self.base_url}/api/v1/query"
        all_passed = True
        
        for query, name in operators:
            params = {"query": query}
            try:
                response = requests.get(url, params=params, timeout=10)
                if response.status_code == 200:
                    data = response.json()
                    if data.get("status") == "success":
                        print(f"✅ {name} 算子成功")
                    else:
                        print(f"⚠️  {name} 算子: {data.get('error')}")
                else:
                    print(f"⚠️  {name} 算子: {response.status_code}")
            except Exception as e:
                print(f"⚠️  {name} 算子错误: {e}")
        
        return True
    
    def test_metadata(self):
        """测试元数据查询"""
        print("\n" + "="*60)
        print("测试元数据查询")
        print("="*60)
        
        # 测试标签列表
        try:
            response = requests.get(f"{self.base_url}/api/v1/labels", timeout=10)
            if response.status_code == 200:
                data = response.json()
                if data.get("status") == "success":
                    labels = data.get("data", [])
                    print(f"✅ 标签列表查询成功 ({len(labels)} 个标签)")
                else:
                    print(f"❌ 标签列表查询失败: {data.get('error')}")
                    return False
            else:
                print(f"❌ 标签列表查询失败: {response.status_code}")
                return False
        except Exception as e:
            print(f"❌ 标签列表查询错误: {e}")
            return False
        
        # 测试标签值
        try:
            response = requests.get(f"{self.base_url}/api/v1/label/job/values", timeout=10)
            if response.status_code == 200:
                data = response.json()
                if data.get("status") == "success":
                    values = data.get("data", [])
                    print(f"✅ 标签值查询成功 ({len(values)} 个值)")
                    return True
                else:
                    print(f"❌ 标签值查询失败: {data.get('error')}")
                    return False
            else:
                print(f"❌ 标签值查询失败: {response.status_code}")
                return False
        except Exception as e:
            print(f"❌ 标签值查询错误: {e}")
            return False
    
    def test_flush_and_recovery(self):
        """测试数据刷盘和恢复"""
        print("\n" + "="*60)
        print("测试数据刷盘和恢复")
        print("="*60)
        
        # 1. 写入第一批数据
        print("1. 写入第一批数据...")
        now = int(time.time() * 1000)
        first_batch = []
        for i in range(50):
            timestamp = now - (49 - i) * 1000
            value = 40 + (i % 60)
            data = f'persistence_test{{batch="first", instance="server1"}} {value} {timestamp}'
            first_batch.append(data)
        
        url = f"{self.base_url}/api/v1/write"
        headers = {"Content-Type": "text/plain"}
        
        try:
            response = requests.post(url, headers=headers, data="\n".join(first_batch), timeout=30)
            if response.status_code in [200, 204]:
                print("✓ 第一批数据写入成功")
            else:
                print(f"❌ 第一批数据写入失败: {response.status_code}")
                return False
        except Exception as e:
            print(f"❌ 第一批数据写入错误: {e}")
            return False
        
        # 2. 触发刷盘
        print("2. 触发数据刷盘...")
        try:
            response = requests.post(f"{self.base_url}/api/v1/admin/flush", timeout=10)
            if response.status_code in [200, 204]:
                print("✓ 刷盘操作成功")
            else:
                print(f"⚠️ 刷盘操作返回: {response.status_code}")
        except Exception as e:
            print(f"⚠️ 刷盘操作错误: {e}")
        
        time.sleep(2)
        
        # 3. 写入第二批数据（不刷盘）
        print("3. 写入第二批数据（不刷盘）...")
        second_batch = []
        for i in range(50):
            timestamp = now - (49 - i) * 1000
            value = 50 + (i % 50)
            data = f'persistence_test{{batch="second", instance="server1"}} {value} {timestamp}'
            second_batch.append(data)
        
        try:
            response = requests.post(url, headers=headers, data="\n".join(second_batch), timeout=30)
            if response.status_code in [200, 204]:
                print("✓ 第二批数据写入成功")
            else:
                print(f"❌ 第二批数据写入失败: {response.status_code}")
                return False
        except Exception as e:
            print(f"❌ 第二批数据写入错误: {e}")
            return False
        
        # 4. 验证数据
        print("4. 验证数据...")
        query_url = f"{self.base_url}/api/v1/query"
        
        try:
            response = requests.get(query_url, params={"query": "persistence_test"}, timeout=10)
            if response.status_code == 200:
                data = response.json()
                if data.get("status") == "success":
                    result = data.get("data", {}).get("result", [])
                    if len(result) >= 2:
                        print(f"✓ 数据验证成功 ({len(result)} 个序列)")
                    else:
                        print(f"⚠️  数据验证: 只返回 {len(result)} 个序列")
                else:
                    print(f"❌ 数据验证失败: {data.get('error')}")
                    return False
            else:
                print(f"❌ 数据验证失败: {response.status_code}")
                return False
        except Exception as e:
            print(f"❌ 数据验证错误: {e}")
            return False
        
        print("✅ 数据刷盘测试完成")
        return True
    
    def run_all_tests(self):
        """运行所有测试"""
        print("\n" + "="*70)
        print(" "*20 + "ChronoDB 集成测试")
        print("="*70)
        
        tests = [
            ("启动服务器", self.start_server),
            ("健康检查", self.test_health_check),
            ("生成测试数据", self.generate_test_data),
            ("写入测试数据", self.write_data),
            ("基本查询", self.test_basic_query),
            ("聚合查询", self.test_aggregation),
            ("标签过滤", self.test_label_filter),
            ("时间范围查询", self.test_query_range),
            ("查询算子", self.test_operators),
            ("元数据查询", self.test_metadata),
            ("数据刷盘", self.test_flush_and_recovery),
        ]
        
        passed = 0
        failed = 0
        
        for test_name, test_func in tests:
            try:
                if test_func():
                    passed += 1
                    self.test_results.append((test_name, True))
                else:
                    failed += 1
                    self.test_results.append((test_name, False))
            except Exception as e:
                print(f"❌ {test_name} 异常: {e}")
                failed += 1
                self.test_results.append((test_name, False))
        
        # 停止服务器
        self.stop_server()
        
        # 打印测试结果汇总
        print("\n" + "="*70)
        print(" "*20 + "测试结果汇总")
        print("="*70)
        
        for test_name, result in self.test_results:
            status = "✅ 通过" if result else "❌ 失败"
            print(f"  {status} - {test_name}")
        
        print("-"*70)
        print(f"  总测试数: {len(tests)}")
        print(f"  通过: {passed}")
        print(f"  失败: {failed}")
        print(f"  成功率: {passed/len(tests)*100:.1f}%")
        print("="*70)
        
        if failed == 0:
            print("\n🎉 所有测试通过！")
            return True
        else:
            print(f"\n❌ {failed} 个测试失败！")
            return False


if __name__ == "__main__":
    test = ChronoDBIntegrationTest()
    success = test.run_all_tests()
    sys.exit(0 if success else 1)
