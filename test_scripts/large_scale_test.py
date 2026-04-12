#!/usr/bin/env python3
"""
Large 规模性能测试 - 分批写入避免内存问题
数据规模: 100 指标 × 100 序列 × 10,000 样本 = 100,000,000 样本
"""

import requests
import time
import random
import sys
import statistics
from concurrent.futures import ThreadPoolExecutor, as_completed

# 服务器地址
CHRONODB_URL = "http://localhost:9090"
WRITE_URL = f"{CHRONODB_URL}/api/v1/write"
QUERY_URL = f"{CHRONODB_URL}/api/v1/query"
QUERY_RANGE_URL = f"{CHRONODB_URL}/api/v1/query_range"

# 测试配置
METRICS_COUNT = 100
SERIES_PER_METRIC = 100
SAMPLES_PER_SERIES = 10000
BATCH_SIZE = 500  # 每批写入的样本数
WRITE_CONCURRENCY = 5  # 并发写入线程数
QUERY_CONCURRENCY = 10  # 并发查询线程数

class LargeScaleTest:
    def __init__(self):
        self.total_samples = 0
        self.write_times = []
        self.query_times = []
        self.errors = []
        
    def generate_metric_name(self, index):
        """生成指标名称"""
        metric_types = [
            "cpu", "memory", "disk", "network", "http_requests",
            "http_errors", "request_duration", "queue_length", "active_connections",
            "cache_hits", "cache_misses", "gc_duration", "thread_count"
        ]
        base = metric_types[index % len(metric_types)]
        suffix = index // len(metric_types)
        return f"{base}_{suffix}" if suffix > 0 else base
    
    def generate_series_labels(self, metric_index, series_index):
        """生成序列标签"""
        jobs = ["frontend", "backend", "api", "worker", "scheduler"]
        instances = [f"instance-{i:03d}" for i in range(20)]
        regions = ["us-east-1", "us-west-2", "eu-west-1", "ap-southeast-1"]
        environments = ["production", "staging", "development"]
        
        job = jobs[series_index % len(jobs)]
        instance = instances[(series_index // len(jobs)) % len(instances)]
        region = regions[(series_index // (len(jobs) * len(instances))) % len(regions)]
        environment = environments[(series_index // (len(jobs) * len(instances) * len(regions))) % len(environments)]
        
        return f'job="{job}", instance="{instance}", region="{region}", environment="{environment}"'
    
    def generate_samples(self, metric_name, labels, start_time, count):
        """生成样本数据"""
        lines = []
        base_value = random.uniform(10, 100)
        
        for i in range(count):
            timestamp = start_time + i * 1000  # 每秒一个数据点
            
            # 根据指标类型生成不同的值模式
            if "cpu" in metric_name or "memory" in metric_name:
                value = base_value + random.uniform(-20, 20)
                value = max(0, min(100, value))
            elif "http_requests" in metric_name:
                value = int(base_value + i * random.uniform(0.5, 2))
            elif "duration" in metric_name:
                value = random.uniform(0.001, 2.0)
            else:
                value = base_value + random.uniform(-10, 10)
            
            line = f'{metric_name}{{{labels}}} {value:.6f} {timestamp}'
            lines.append(line)
        
        return "\n".join(lines)
    
    def write_batch(self, batch_data):
        """写入一批数据"""
        try:
            start_time = time.time()
            response = requests.post(
                WRITE_URL,
                data=batch_data.encode('utf-8'),
                headers={"Content-Type": "text/plain"},
                timeout=30
            )
            elapsed = time.time() - start_time
            
            if response.status_code == 204:
                return True, elapsed, None
            else:
                return False, elapsed, f"HTTP {response.status_code}: {response.text}"
        except Exception as e:
            return False, 0, str(e)
    
    def run_write_test(self):
        """运行写入测试"""
        print(f"\n{'='*60}")
        print("Large Scale Write Test")
        print(f"{'='*60}")
        print(f"Metrics: {METRICS_COUNT}")
        print(f"Series per metric: {SERIES_PER_METRIC}")
        print(f"Samples per series: {SAMPLES_PER_SERIES}")
        print(f"Total samples: {METRICS_COUNT * SERIES_PER_METRIC * SAMPLES_PER_SERIES:,}")
        print(f"Batch size: {BATCH_SIZE}")
        print(f"Write concurrency: {WRITE_CONCURRENCY}")
        
        start_time = time.time()
        total_samples = 0
        batches = []
        
        # 生成批次数据
        now = int(time.time() * 1000) - SAMPLES_PER_SERIES * 1000  # 从过去开始
        
        for metric_idx in range(METRICS_COUNT):
            metric_name = self.generate_metric_name(metric_idx)
            
            for series_idx in range(SERIES_PER_METRIC):
                labels = self.generate_series_labels(metric_idx, series_idx)
                
                # 将每个序列分成多个批次
                for batch_start in range(0, SAMPLES_PER_SERIES, BATCH_SIZE):
                    batch_count = min(BATCH_SIZE, SAMPLES_PER_SERIES - batch_start)
                    batch_time = now + batch_start * 1000
                    
                    batch_data = self.generate_samples(
                        metric_name, labels, batch_time, batch_count
                    )
                    batches.append(batch_data)
                    total_samples += batch_count
        
        print(f"\nGenerated {len(batches)} batches, {total_samples:,} total samples")
        print("Starting write test...\n")
        
        # 并发写入
        success_count = 0
        failed_count = 0
        
        with ThreadPoolExecutor(max_workers=WRITE_CONCURRENCY) as executor:
            futures = {executor.submit(self.write_batch, batch): i 
                      for i, batch in enumerate(batches)}
            
            for future in as_completed(futures):
                batch_idx = futures[future]
                success, elapsed, error = future.result()
                
                if success:
                    success_count += 1
                    self.write_times.append(elapsed)
                    if success_count % 100 == 0:
                        print(f"  Written {success_count}/{len(batches)} batches...")
                else:
                    failed_count += 1
                    self.errors.append(f"Batch {batch_idx}: {error}")
                    if failed_count <= 5:
                        print(f"  ❌ Batch {batch_idx} failed: {error}")
        
        elapsed = time.time() - start_time
        self.total_samples = total_samples
        
        print(f"\n{'='*60}")
        print("Write Test Results")
        print(f"{'='*60}")
        print(f"Total batches: {len(batches)}")
        print(f"Successful: {success_count}")
        print(f"Failed: {failed_count}")
        print(f"Total time: {elapsed:.2f}s")
        print(f"Throughput: {total_samples / elapsed:,.0f} samples/sec")
        
        if self.write_times:
            print(f"\nLatency Statistics:")
            print(f"  Min: {min(self.write_times)*1000:.2f}ms")
            print(f"  Max: {max(self.write_times)*1000:.2f}ms")
            print(f"  Avg: {statistics.mean(self.write_times)*1000:.2f}ms")
            print(f"  P95: {sorted(self.write_times)[int(len(self.write_times)*0.95)]*1000:.2f}ms")
        
        return failed_count == 0
    
    def run_query_test(self):
        """运行查询测试"""
        print(f"\n{'='*60}")
        print("Large Scale Query Test")
        print(f"{'='*60}")
        
        # 测试查询
        queries = [
            ("Basic query", "cpu"),
            ("With label filter", 'cpu{job="frontend"}'),
            ("Aggregation - sum", "sum(http_requests)"),
            ("Aggregation - avg", "avg(cpu)"),
            ("Range query", "cpu[1h]"),
            ("Rate function", 'rate(http_requests[5m])'),
        ]
        
        query_results = []
        
        for name, query in queries:
            try:
                start_time = time.time()
                response = requests.get(
                    QUERY_URL,
                    params={"query": query},
                    timeout=60
                )
                elapsed = time.time() - start_time
                
                if response.status_code == 200:
                    data = response.json()
                    result_count = len(data.get("data", {}).get("result", []))
                    print(f"  ✅ {name}: {elapsed*1000:.2f}ms ({result_count} results)")
                    self.query_times.append(elapsed)
                    query_results.append((name, True, elapsed, result_count))
                else:
                    print(f"  ❌ {name}: HTTP {response.status_code}")
                    query_results.append((name, False, elapsed, 0))
            except Exception as e:
                print(f"  ❌ {name}: {str(e)}")
                query_results.append((name, False, 0, 0))
        
        if self.query_times:
            print(f"\nQuery Statistics:")
            print(f"  Min: {min(self.query_times)*1000:.2f}ms")
            print(f"  Max: {max(self.query_times)*1000:.2f}ms")
            print(f"  Avg: {statistics.mean(self.query_times)*1000:.2f}ms")
            if len(self.query_times) >= 2:
                print(f"  Median: {statistics.median(self.query_times)*1000:.2f}ms")
        
        return query_results
    
    def check_server_health(self):
        """检查服务器健康状态"""
        try:
            response = requests.get(f"{CHRONODB_URL}/health", timeout=5)
            return response.status_code == 200
        except Exception:
            return False
    
    def run(self):
        """运行完整的 Large 规模测试"""
        print("="*60)
        print("ChronoDB Large Scale Performance Test")
        print("="*60)
        
        # 检查服务器健康
        print("\nChecking server health...")
        if not self.check_server_health():
            print("❌ Server is not healthy. Please start ChronoDB first.")
            return False
        
        print("✅ Server is healthy")
        
        # 运行写入测试
        write_success = self.run_write_test()
        
        if not write_success:
            print("\n❌ Write test failed. Check errors above.")
            return False
        
        # 等待数据稳定
        print("\nWaiting for data to stabilize...")
        time.sleep(2)
        
        # 运行查询测试
        query_results = self.run_query_test()
        
        # 输出总结
        print(f"\n{'='*60}")
        print("Test Summary")
        print(f"{'='*60}")
        print(f"Total samples written: {self.total_samples:,}")
        print(f"Write throughput: {self.total_samples / sum(self.write_times):,.0f} samples/sec" if self.write_times else "N/A")
        
        if query_results:
            successful_queries = sum(1 for _, success, _, _ in query_results if success)
            print(f"Successful queries: {successful_queries}/{len(query_results)}")
        
        if self.errors:
            print(f"\nErrors encountered: {len(self.errors)}")
            for error in self.errors[:5]:
                print(f"  - {error}")
        
        return write_success

if __name__ == "__main__":
    test = LargeScaleTest()
    success = test.run()
    sys.exit(0 if success else 1)
