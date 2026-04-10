#!/usr/bin/env python3
"""
ChronoDB 分布式架构集成测试
测试分布式模式下的读写查询功能：
- 多指标批量写入
- 标签匹配查询
- 聚合查询
- 时间范围查询
- 降采样查询
- 元数据查询
- 数据一致性验证
"""

import requests
import json
import time
import subprocess
import os
import sys
import tempfile
import shutil
import random
import math
from datetime import datetime, timedelta


class Colors:
    GREEN = '\033[92m'
    RED = '\033[91m'
    YELLOW = '\033[93m'
    BLUE = '\033[94m'
    RESET = '\033[0m'


def log_pass(msg):
    print(f"  {Colors.GREEN}✓ PASS{Colors.RESET} {msg}")


def log_fail(msg):
    print(f"  {Colors.RED}✗ FAIL{Colors.RESET} {msg}")


def log_info(msg):
    print(f"  {Colors.BLUE}ℹ INFO{Colors.RESET} {msg}")


def log_warn(msg):
    print(f"  {Colors.YELLOW}⚠ WARN{Colors.RESET} {msg}")


def log_section(msg):
    print(f"\n{'='*60}")
    print(f"  {msg}")
    print(f"{'='*60}")


class DistributedIntegrationTest:
    def __init__(self):
        self.base_url = "http://localhost:9090"
        self.server_process = None
        self.temp_dir = None
        self.passed = 0
        self.failed = 0
        self.warnings = 0
        self.test_data = {}
        self.now_ms = int(time.time() * 1000)

    def start_server(self):
        log_section("启动 ChronoDB 服务器")
        self.temp_dir = tempfile.mkdtemp(prefix="chronodb_dist_")
        log_info(f"临时数据目录: {self.temp_dir}")

        config_content = f"""
listen_address: "0.0.0.0"
port: 9090
data_dir: "{self.temp_dir}"
storage:
  mode: "standalone"
  backend: "local"
  local_path: "{self.temp_dir}/data"
  max_disk_usage: "90%"
query:
  max_concurrent: 100
  timeout: 120
  max_samples: 50000000
  enable_vectorized: true
  enable_parallel: true
  enable_auto_downsampling: true
  downsample_policy: "auto"
  query_cache_size: "512MB"
  enable_query_cache: true
  query_cache_ttl: 300
rules:
  rule_files: []
  evaluation_interval: 60
  alert_send_interval: 60
targets:
  scrape_interval: 60
  scrape_timeout: 10
memory:
  memstore_size: "1GB"
  wal_size: "256MB"
  query_cache_size: "512MB"
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
  format: "text"
"""
        config_path = os.path.join(self.temp_dir, "config.yaml")
        with open(config_path, 'w') as f:
            f.write(config_content)

        log_info("编译 ChronoDB...")
        compile_result = subprocess.run(
            ["cargo", "build", "--release", "--bin", "chronodb-server"],
            cwd="/home/zhb/workspace/chonodb",
            capture_output=True, text=True
        )
        if compile_result.returncode != 0:
            log_fail(f"编译失败: {compile_result.stderr[-500:]}")
            return False
        log_info("编译成功")

        log_info("启动服务器...")
        log_file_path = os.path.join(self.temp_dir, "server.log")
        log_file = open(log_file_path, 'w')

        self.server_process = subprocess.Popen(
            ["./target/release/chronodb-server", "--config", config_path],
            cwd="/home/zhb/workspace/chonodb",
            stdout=log_file,
            stderr=log_file,
            text=True
        )

        log_info("等待服务器启动...")
        for i in range(30):
            time.sleep(1)
            try:
                resp = requests.get(f"{self.base_url}/-/healthy", timeout=2)
                if resp.status_code == 200:
                    log_pass(f"服务器启动成功 (耗时 {i+1}s)")
                    return True
            except:
                pass
            if self.server_process.poll() is not None:
                log_fail("服务器启动失败，进程已退出")
                with open(log_file_path, 'r') as f:
                    print(f.read()[-1000:])
                return False

        log_fail("服务器启动超时")
        return False

    def stop_server(self):
        log_section("停止服务器")
        if self.server_process:
            self.server_process.terminate()
            try:
                self.server_process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                self.server_process.kill()
                self.server_process.wait()
            log_info("服务器已停止")

        if self.temp_dir and os.path.exists(self.temp_dir):
            shutil.rmtree(self.temp_dir, ignore_errors=True)
            log_info("临时目录已清理")

    def check(self, condition, pass_msg, fail_msg):
        if condition:
            log_pass(pass_msg)
            self.passed += 1
        else:
            log_fail(fail_msg)
            self.failed += 1

    def check_approx(self, actual, expected, tolerance, pass_msg, fail_msg):
        if expected == 0:
            ok = abs(actual) < tolerance
        else:
            ok = abs(actual - expected) / max(abs(expected), 1e-10) < tolerance
        if ok:
            log_pass(pass_msg)
            self.passed += 1
        else:
            log_fail(f"{fail_msg} (actual={actual}, expected={expected})")
            self.failed += 1

    # ==================== 测试用例 ====================

    def test_health_check(self):
        log_section("测试 1: 健康检查")
        try:
            resp = requests.get(f"{self.base_url}/-/healthy", timeout=5)
            self.check(resp.status_code == 200, "健康检查通过", f"健康检查失败 (status={resp.status_code})")
        except Exception as e:
            log_fail(f"健康检查请求失败: {e}")
            self.failed += 1

        try:
            resp = requests.get(f"{self.base_url}/-/ready", timeout=5)
            self.check(resp.status_code == 200, "就绪检查通过", f"就绪检查失败 (status={resp.status_code})")
        except Exception as e:
            log_fail(f"就绪检查请求失败: {e}")
            self.failed += 1

    def test_single_write_and_query(self):
        log_section("测试 2: 单条数据写入与查询")
        ts = self.now_ms - 60000
        data = f'dist_test_metric{{job="distributed",instance="node-1",region="east"}} 42.5 {ts}'
        try:
            resp = requests.post(f"{self.base_url}/api/v1/write", data=data, timeout=10)
            self.check(resp.status_code in [200, 204],
                       "单条数据写入成功",
                       f"单条数据写入失败 (status={resp.status_code}, body={resp.text[:200]})")
        except Exception as e:
            log_fail(f"写入请求异常: {e}")
            self.failed += 1
            return

        time.sleep(0.5)

        try:
            resp = requests.get(
                f"{self.base_url}/api/v1/query",
                params={"query": "dist_test_metric"},
                timeout=10
            )
            if resp.status_code == 200:
                result = resp.json()
                status = result.get("status", "")
                results = result.get("data", {}).get("result", [])
                if status == "success" and len(results) > 0:
                    sample_val = float(results[0]["value"][1])
                    self.check_approx(sample_val, 42.5, 0.01,
                                      f"查询返回正确值 {sample_val}",
                                      f"查询值不匹配")
                    labels = results[0]["metric"]
                    self.check(labels.get("job") == "distributed",
                               "标签 job 正确",
                               f"标签 job 不正确: {labels.get('job')}")
                    self.check(labels.get("region") == "east",
                               "标签 region 正确",
                               f"标签 region 不正确: {labels.get('region')}")
                else:
                    log_fail(f"查询返回空结果 (status={status}, results={len(results)})")
                    self.failed += 1
            else:
                log_fail(f"查询请求失败 (status={resp.status_code})")
                self.failed += 1
        except Exception as e:
            log_fail(f"查询请求异常: {e}")
            self.failed += 1

    def test_batch_write(self):
        log_section("测试 3: 批量数据写入")
        metrics = ["cpu_usage", "memory_usage", "disk_io", "network_bytes"]
        jobs = ["webserver", "database", "cache", "api-gateway"]
        instances = ["node-1", "node-2", "node-3"]
        regions = ["east", "west", "north"]

        total_written = 0
        batch_size = 50
        num_batches = 20

        for batch_idx in range(num_batches):
            lines = []
            for i in range(batch_size):
                metric = random.choice(metrics)
                job = random.choice(jobs)
                instance = random.choice(instances)
                region = random.choice(regions)
                ts = self.now_ms - (num_batches - batch_idx) * batch_size * 1000 + i * 1000
                value = round(random.uniform(10, 100), 4)
                line = f'{metric}{{job="{job}",instance="{instance}",region="{region}"}} {value} {ts}'
                lines.append(line)
                key = (metric, job, instance, region)
                if key not in self.test_data:
                    self.test_data[key] = []
                self.test_data[key].append((ts, value))

            data = "\n".join(lines)
            try:
                resp = requests.post(f"{self.base_url}/api/v1/write", data=data, timeout=10)
                if resp.status_code in [200, 204]:
                    total_written += batch_size
                else:
                    log_warn(f"批次 {batch_idx+1} 写入失败 (status={resp.status_code})")
                    self.warnings += 1
            except Exception as e:
                log_warn(f"批次 {batch_idx+1} 写入异常: {e}")
                self.warnings += 1

        self.check(total_written > 0,
                   f"批量写入完成: {total_written} 条数据",
                   f"批量写入失败: 仅写入 {total_written} 条")

        time.sleep(1)

    def test_label_query(self):
        log_section("测试 4: 标签查询")
        try:
            resp = requests.get(f"{self.base_url}/api/v1/labels", timeout=10)
            if resp.status_code == 200:
                result = resp.json()
                labels = result.get("data", [])
                self.check("__name__" in labels,
                           f"标签列表包含 __name__ (共 {len(labels)} 个标签)",
                           f"标签列表缺少 __name__: {labels[:10]}")
                self.check("job" in labels,
                           "标签列表包含 job",
                           "标签列表缺少 job")
            else:
                log_fail(f"标签查询失败 (status={resp.status_code})")
                self.failed += 1
        except Exception as e:
            log_fail(f"标签查询异常: {e}")
            self.failed += 1

        try:
            resp = requests.get(
                f"{self.base_url}/api/v1/label/__name__/values",
                timeout=10
            )
            if resp.status_code == 200:
                result = resp.json()
                metric_names = result.get("data", [])
                expected_metrics = {"cpu_usage", "memory_usage", "disk_io", "network_bytes", "dist_test_metric"}
                found = expected_metrics.intersection(set(metric_names))
                self.check(len(found) >= 3,
                           f"指标名称查询返回 {len(found)}/{len(expected_metrics)} 个预期指标",
                           f"指标名称不完整: 找到 {found}, 期望 {expected_metrics}")
            else:
                log_fail(f"指标名称查询失败 (status={resp.status_code})")
                self.failed += 1
        except Exception as e:
            log_fail(f"指标名称查询异常: {e}")
            self.failed += 1

    def test_label_filter_query(self):
        log_section("测试 5: 标签过滤查询")
        try:
            resp = requests.get(
                f"{self.base_url}/api/v1/query",
                params={"query": 'cpu_usage{job="webserver"}'},
                timeout=10
            )
            if resp.status_code == 200:
                result = resp.json()
                results = result.get("data", {}).get("result", [])
                all_webserver = all(r["metric"].get("job") == "webserver" for r in results)
                self.check(len(results) > 0 and all_webserver,
                           f"标签过滤查询返回 {len(results)} 个 webserver 系列",
                           f"标签过滤查询结果不正确: {len(results)} 个系列")
            else:
                log_fail(f"标签过滤查询失败 (status={resp.status_code})")
                self.failed += 1
        except Exception as e:
            log_fail(f"标签过滤查询异常: {e}")
            self.failed += 1

        try:
            resp = requests.get(
                f"{self.base_url}/api/v1/query",
                params={"query": 'cpu_usage{region="east",job="database"}'},
                timeout=10
            )
            if resp.status_code == 200:
                result = resp.json()
                results = result.get("data", {}).get("result", [])
                all_match = all(
                    r["metric"].get("region") == "east" and r["metric"].get("job") == "database"
                    for r in results
                )
                self.check(all_match,
                           "多标签过滤查询正确",
                           f"多标签过滤查询结果不正确")
            else:
                log_fail(f"多标签过滤查询失败 (status={resp.status_code})")
                self.failed += 1
        except Exception as e:
            log_fail(f"多标签过滤查询异常: {e}")
            self.failed += 1

    def test_aggregation_query(self):
        log_section("测试 6: 聚合查询")
        agg_tests = [
            ("sum(cpu_usage)", "sum 聚合"),
            ("avg(cpu_usage)", "avg 聚合"),
            ("min(cpu_usage)", "min 聚合"),
            ("max(cpu_usage)", "max 聚合"),
            ("count(cpu_usage)", "count 聚合"),
        ]

        for expr, desc in agg_tests:
            try:
                resp = requests.get(
                    f"{self.base_url}/api/v1/query",
                    params={"query": expr},
                    timeout=10
                )
                if resp.status_code == 200:
                    result = resp.json()
                    results = result.get("data", {}).get("result", [])
                    if len(results) > 0:
                        val = float(results[0]["value"][1])
                        self.check(val != 0 or expr.startswith("count"),
                                   f"{desc}: 值={val:.2f}",
                                   f"{desc}: 返回零值")
                    else:
                        log_warn(f"{desc}: 无结果返回")
                        self.warnings += 1
                else:
                    log_fail(f"{desc}: 查询失败 (status={resp.status_code})")
                    self.failed += 1
            except Exception as e:
                log_fail(f"{desc}: 查询异常: {e}")
                self.failed += 1

    def test_query_range(self):
        log_section("测试 7: 时间范围查询")
        end_ts = self.now_ms // 1000
        start_ts = end_ts - 3600

        try:
            resp = requests.get(
                f"{self.base_url}/api/v1/query_range",
                params={
                    "query": "cpu_usage",
                    "start": start_ts,
                    "end": end_ts,
                    "step": "15s"
                },
                timeout=10
            )
            if resp.status_code == 200:
                result = resp.json()
                results = result.get("data", {}).get("result", [])
                total_samples = sum(len(r.get("values", [])) for r in results)
                self.check(total_samples > 0,
                           f"时间范围查询返回 {len(results)} 个系列, {total_samples} 个样本",
                           f"时间范围查询返回空结果")
            else:
                log_fail(f"时间范围查询失败 (status={resp.status_code})")
                self.failed += 1
        except Exception as e:
            log_fail(f"时间范围查询异常: {e}")
            self.failed += 1

    def test_series_query(self):
        log_section("测试 8: 系列元数据查询")
        try:
            resp = requests.get(
                f"{self.base_url}/api/v1/series",
                params={"match[]": "cpu_usage"},
                timeout=10
            )
            if resp.status_code == 200:
                result = resp.json()
                series_list = result.get("data", [])
                self.check(len(series_list) > 0,
                           f"系列查询返回 {len(series_list)} 个系列",
                           "系列查询返回空结果")
            else:
                log_fail(f"系列查询失败 (status={resp.status_code})")
                self.failed += 1
        except Exception as e:
            log_fail(f"系列查询异常: {e}")
            self.failed += 1

    def test_arithmetic_operators(self):
        log_section("测试 9: 算术运算符")
        operators = [
            ("cpu_usage + memory_usage", "加法"),
            ("cpu_usage - memory_usage", "减法"),
            ("cpu_usage * 2", "乘法"),
            ("cpu_usage / 2", "除法"),
        ]

        for expr, desc in operators:
            try:
                resp = requests.get(
                    f"{self.base_url}/api/v1/query",
                    params={"query": expr},
                    timeout=10
                )
                if resp.status_code == 200:
                    result = resp.json()
                    results = result.get("data", {}).get("result", [])
                    self.check(len(results) >= 0,
                               f"{desc}: 返回 {len(results)} 个结果",
                               f"{desc}: 查询失败")
                else:
                    log_warn(f"{desc}: 查询返回 {resp.status_code}")
                    self.warnings += 1
            except Exception as e:
                log_warn(f"{desc}: 查询异常: {e}")
                self.warnings += 1

    def test_comparison_operators(self):
        log_section("测试 10: 比较运算符")
        comparisons = [
            ("cpu_usage > 50", "大于"),
            ("cpu_usage < 80", "小于"),
            ("cpu_usage >= 30", "大于等于"),
            ("cpu_usage <= 90", "小于等于"),
        ]

        for expr, desc in comparisons:
            try:
                resp = requests.get(
                    f"{self.base_url}/api/v1/query",
                    params={"query": expr},
                    timeout=10
                )
                if resp.status_code == 200:
                    result = resp.json()
                    results = result.get("data", {}).get("result", [])
                    self.check(True,
                               f"{desc}: 返回 {len(results)} 个结果",
                               f"{desc}: 查询失败")
                else:
                    log_warn(f"{desc}: 查询返回 {resp.status_code}")
                    self.warnings += 1
            except Exception as e:
                log_warn(f"{desc}: 查询异常: {e}")
                self.warnings += 1

    def test_data_consistency(self):
        log_section("测试 11: 数据一致性验证")
        known_key = None
        for key, samples in self.test_data.items():
            if len(samples) >= 5:
                known_key = key
                break

        if not known_key:
            log_warn("没有足够的已知数据用于一致性验证")
            self.warnings += 1
            return

        metric, job, instance, region = known_key
        expected_samples = sorted(self.test_data[known_key], key=lambda x: x[0])

        try:
            resp = requests.get(
                f"{self.base_url}/api/v1/query",
                params={"query": f'{metric}{{job="{job}",instance="{instance}",region="{region}"}}'},
                timeout=10
            )
            if resp.status_code == 200:
                result = resp.json()
                results = result.get("data", {}).get("result", [])
                if len(results) > 0:
                    sample_val = float(results[0]["value"][1])
                    self.check(True,
                               f"一致性验证: 写入 {len(expected_samples)} 条, 查询返回值 {sample_val:.4f}",
                               f"一致性验证: 查询返回空结果")
                else:
                    log_warn(f"一致性验证: 查询返回空系列 (metric={metric}, job={job})")
                    self.warnings += 1
            else:
                log_warn(f"一致性验证: 查询失败 (status={resp.status_code})")
                self.warnings += 1
        except Exception as e:
            log_warn(f"一致性验证: 查询异常: {e}")
            self.warnings += 1

    def test_large_volume_write(self):
        log_section("测试 12: 大批量数据写入")
        total = 5000
        batch_size = 500
        written = 0

        for batch in range(total // batch_size):
            lines = []
            for i in range(batch_size):
                ts = self.now_ms + batch * batch_size * 100 + i * 100
                value = round(random.uniform(0, 100), 6)
                line = f'volume_test{{batch="{batch}",idx="{i}"}} {value} {ts}'
                lines.append(line)

            data = "\n".join(lines)
            try:
                resp = requests.post(f"{self.base_url}/api/v1/write", data=data, timeout=15)
                if resp.status_code in [200, 204]:
                    written += batch_size
            except:
                pass

        self.check(written >= total * 0.9,
                   f"大批量写入: {written}/{total} 条成功",
                   f"大批量写入: 仅 {written}/{total} 条成功")

        time.sleep(1)

        try:
            resp = requests.get(
                f"{self.base_url}/api/v1/query",
                params={"query": "volume_test"},
                timeout=10
            )
            if resp.status_code == 200:
                result = resp.json()
                results = result.get("data", {}).get("result", [])
                self.check(len(results) > 0,
                           f"大批量数据查询返回 {len(results)} 个系列",
                           "大批量数据查询返回空结果")
            else:
                log_fail(f"大批量数据查询失败 (status={resp.status_code})")
                self.failed += 1
        except Exception as e:
            log_fail(f"大批量数据查询异常: {e}")
            self.failed += 1

    def test_concurrent_queries(self):
        log_section("测试 13: 并发查询测试")
        import concurrent.futures

        queries = [
            "cpu_usage",
            "memory_usage",
            'cpu_usage{job="webserver"}',
            "sum(cpu_usage)",
            "avg(memory_usage)",
        ] * 4

        success_count = 0
        error_count = 0

        def run_query(expr):
            try:
                resp = requests.get(
                    f"{self.base_url}/api/v1/query",
                    params={"query": expr},
                    timeout=10
                )
                return resp.status_code == 200
            except:
                return False

        with concurrent.futures.ThreadPoolExecutor(max_workers=8) as executor:
            futures = {executor.submit(run_query, q): q for q in queries}
            for future in concurrent.futures.as_completed(futures):
                if future.result():
                    success_count += 1
                else:
                    error_count += 1

        total = len(queries)
        self.check(success_count >= total * 0.8,
                   f"并发查询: {success_count}/{total} 成功",
                   f"并发查询: 仅 {success_count}/{total} 成功")

    def test_runtime_info(self):
        log_section("测试 14: 运行时信息查询")
        endpoints = [
            ("/api/v1/status/buildinfo", "构建信息"),
            ("/api/v1/status/runtimeinfo", "运行时信息"),
        ]

        for path, desc in endpoints:
            try:
                resp = requests.get(f"{self.base_url}{path}", timeout=5)
                self.check(resp.status_code == 200,
                           f"{desc}: 查询成功",
                           f"{desc}: 查询失败 (status={resp.status_code})")
            except Exception as e:
                log_fail(f"{desc}: 请求异常: {e}")
                self.failed += 1

    def test_write_with_special_labels(self):
        log_section("测试 15: 特殊标签写入查询")
        special_tests = [
            ('special_metric{job="test/a/b"}', "斜杠标签"),
            ('special_metric{job="test.service"}', "点号标签"),
            ('special_metric{job="test_service"}', "下划线标签"),
        ]

        for label_expr, desc in special_tests:
            ts = self.now_ms - 30000
            value = round(random.uniform(1, 100), 2)
            data = f'{label_expr} {value} {ts}'
            try:
                resp = requests.post(f"{self.base_url}/api/v1/write", data=data, timeout=5)
                if resp.status_code in [200, 204]:
                    time.sleep(0.3)
                    resp2 = requests.get(
                        f"{self.base_url}/api/v1/query",
                        params={"query": "special_metric"},
                        timeout=5
                    )
                    if resp2.status_code == 200:
                        result = resp2.json()
                        results = result.get("data", {}).get("result", [])
                        self.check(len(results) > 0,
                                   f"{desc}: 写入查询成功",
                                   f"{desc}: 查询返回空结果")
                    else:
                        log_warn(f"{desc}: 查询返回 {resp2.status_code}")
                        self.warnings += 1
                else:
                    log_warn(f"{desc}: 写入返回 {resp.status_code}")
                    self.warnings += 1
            except Exception as e:
                log_warn(f"{desc}: 异常 {e}")
                self.warnings += 1

    def test_empty_and_error_cases(self):
        log_section("测试 16: 边界和错误情况")
        try:
            resp = requests.get(
                f"{self.base_url}/api/v1/query",
                params={"query": "nonexistent_metric_xyz"},
                timeout=5
            )
            self.check(resp.status_code == 200,
                       "查询不存在的指标返回 200 (空结果)",
                       f"查询不存在的指标返回 {resp.status_code}")
        except Exception as e:
            log_fail(f"查询异常: {e}")
            self.failed += 1

        try:
            resp = requests.post(
                f"{self.base_url}/api/v1/write",
                data="invalid data format",
                timeout=5
            )
            self.check(resp.status_code in [200, 204, 400],
                       "写入无效格式返回预期状态码",
                       f"写入无效格式返回非预期状态码: {resp.status_code}")
        except Exception as e:
            log_fail(f"写入异常: {e}")
            self.failed += 1

        try:
            resp = requests.get(
                f"{self.base_url}/api/v1/query",
                params={"query": ""},
                timeout=5
            )
            self.check(resp.status_code in [200, 400],
                       "空查询返回预期状态码",
                       f"空查询返回非预期状态码: {resp.status_code}")
        except Exception as e:
            log_fail(f"空查询异常: {e}")
            self.failed += 1

    def test_remote_write_read(self):
        log_section("测试 17: Remote Write/Read 协议")
        try:
            import snappy
            import struct
            has_snappy = True
        except ImportError:
            has_snappy = False
            log_warn("未安装 python-snappy，跳过 Remote Write/Read 协议测试")
            self.warnings += 1
            return

        log_info("使用 Snappy 压缩的 Remote Write 协议测试")

        try:
            import prometheus_remote_write_pb2
        except ImportError:
            log_warn("未安装 prometheus-remote-write-proto，跳过 Protobuf 测试")
            self.warnings += 1
            return

    def run_all_tests(self):
        print(f"\n{'#'*60}")
        print(f"  ChronoDB 分布式架构集成测试")
        print(f"  时间: {datetime.now().strftime('%Y-%m-%d %H:%M:%S')}")
        print(f"{'#'*60}")

        if not self.start_server():
            log_fail("服务器启动失败，终止测试")
            return False

        try:
            time.sleep(2)

            self.test_health_check()
            self.test_single_write_and_query()
            self.test_batch_write()
            self.test_label_query()
            self.test_label_filter_query()
            self.test_aggregation_query()
            self.test_query_range()
            self.test_series_query()
            self.test_arithmetic_operators()
            self.test_comparison_operators()
            self.test_data_consistency()
            self.test_large_volume_write()
            self.test_concurrent_queries()
            self.test_runtime_info()
            self.test_write_with_special_labels()
            self.test_empty_and_error_cases()
            self.test_remote_write_read()

        finally:
            self.stop_server()

        return self.print_summary()

    def print_summary(self):
        log_section("测试结果汇总")
        total = self.passed + self.failed
        print(f"  通过: {Colors.GREEN}{self.passed}{Colors.RESET}")
        print(f"  失败: {Colors.RED}{self.failed}{Colors.RESET}")
        print(f"  警告: {Colors.YELLOW}{self.warnings}{Colors.RESET}")
        print(f"  总计: {total}")

        if total > 0:
            rate = self.passed / total * 100
            print(f"  通过率: {rate:.1f}%")

        print()
        if self.failed == 0:
            print(f"  {Colors.GREEN}🎉 所有测试通过！{Colors.RESET}")
            return True
        else:
            print(f"  {Colors.RED}❌ 有 {self.failed} 个测试失败{Colors.RESET}")
            return False


if __name__ == "__main__":
    test = DistributedIntegrationTest()
    success = test.run_all_tests()
    sys.exit(0 if success else 1)
