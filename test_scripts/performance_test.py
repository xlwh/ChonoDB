#!/usr/bin/env python3
"""
ChronoDB 性能和功能测试脚本
支持小、中、大三种数据规模测试
"""

import requests
import json
import time
import statistics
import argparse
import random

class ChronoDBPerformanceTest:
    def __init__(self, data_size="medium"):
        self.base_url = "http://localhost:9090"
        self.data_size = data_size
        self.results = {}
        
        # 数据规模配置
        self.size_config = {
            "small": {
                "series_count": 10,
                "samples_per_series": 1000,
                "description": "小规模测试 (10 series x 1000 samples = 10K samples)"
            },
            "medium": {
                "series_count": 100,
                "samples_per_series": 10000,
                "description": "中规模测试 (100 series x 10000 samples = 1M samples)"
            },
            "large": {
                "series_count": 1000,
                "samples_per_series": 10000,
                "description": "大规模测试 (1000 series x 10000 samples = 10M samples)"
            }
        }
    
    def generate_test_data(self):
        """生成测试数据"""
        config = self.size_config[self.data_size]
        print(f"\n=== {config['description']} ===")
        
        now = int(time.time() * 1000)
        self.test_data = []
        
        for series_idx in range(config["series_count"]):
            job = f"job_{series_idx % 10}"
            instance = f"instance_{series_idx % 20}"
            region = f"region_{series_idx % 5}"
            
            for sample_idx in range(config["samples_per_series"]):
                timestamp = now - (config["samples_per_series"] - 1 - sample_idx) * 1000
                value = 50 + random.uniform(-20, 30)
                line = f"test_metric{{job=\"{job}\", instance=\"{instance}\", region=\"{region}\"}} {value:.2f} {timestamp}"
                self.test_data.append(line)
        
        print(f"生成了 {len(self.test_data)} 条测试数据")
        return True
    
    def write_data(self):
        """写入测试数据"""
        print("\n=== 写入测试数据 ===")
        url = f"{self.base_url}/api/v1/write"
        headers = {"Content-Type": "text/plain"}
        
        config = self.size_config[self.data_size]
        total_samples = config["series_count"] * config["samples_per_series"]
        
        # 分批次写入
        batch_size = 10000
        total_batches = (len(self.test_data) + batch_size - 1) // batch_size
        write_times = []
        
        start_time = time.time()
        
        for i in range(0, len(self.test_data), batch_size):
            batch = self.test_data[i:i+batch_size]
            data = "\n".join(batch)
            
            batch_start = time.time()
            try:
                response = requests.post(url, headers=headers, data=data, timeout=60)
                if response.status_code == 204:
                    batch_time = time.time() - batch_start
                    write_times.append(batch_time)
                    print(f"批次 {i//batch_size + 1}/{total_batches} 写入成功 ({batch_time:.2f}s)")
                else:
                    print(f"批次 {i//batch_size + 1} 写入失败: {response.status_code}")
                    return False
            except Exception as e:
                print(f"批次写入错误: {e}")
                return False
        
        total_time = time.time() - start_time
        throughput = total_samples / total_time / 1000  # samples per second / 1000 = K samples/s
        
        self.results["write"] = {
            "total_samples": total_samples,
            "total_time": total_time,
            "throughput_ksps": throughput,
            "avg_batch_time": statistics.mean(write_times),
            "min_batch_time": min(write_times),
            "max_batch_time": max(write_times)
        }
        
        print(f"\n写入完成!")
        print(f"总数据量: {total_samples:,} samples")
        print(f"总耗时: {total_time:.2f}s")
        print(f"写入吞吐量: {throughput:.2f} K samples/s")
        return True
    
    def test_query_performance(self):
        """测试查询性能"""
        print("\n=== 测试查询性能 ===")
        
        queries = [
            ("test_metric", "基础查询"),
            ("sum(test_metric)", "sum聚合"),
            ("avg(test_metric)", "avg聚合"),
            ("max(test_metric)", "max聚合"),
            ("min(test_metric)", "min聚合"),
            ("count(test_metric)", "count聚合"),
            ("test_metric{job=\"job_0\"}", "标签过滤"),
            ("sum(test_metric) by (job)", "按job聚合"),
            ("sum(test_metric) by (region)", "按region聚合"),
            ("test_metric * 2 + 10", "数学运算"),
        ]
        
        url = f"{self.base_url}/api/v1/query"
        query_times = {}
        
        for query, name in queries:
            times = []
            # 执行3次取平均值
            for _ in range(3):
                start = time.time()
                try:
                    response = requests.get(url, params={"query": query}, timeout=30)
                    elapsed = time.time() - start
                    times.append(elapsed)
                except Exception as e:
                    print(f"查询失败 {name}: {e}")
                    times.append(float('inf'))
            
            avg_time = statistics.mean(times) * 1000  # ms
            query_times[name] = avg_time
            status = "✅" if avg_time < 500 else "⚠️" if avg_time < 2000 else "❌"
            print(f"{status} {name}: {avg_time:.2f} ms")
        
        self.results["queries"] = query_times
        return True
    
    def test_range_query(self):
        """测试时间范围查询"""
        print("\n=== 测试时间范围查询 ===")
        
        now = int(time.time() * 1000)
        start = now - 3600000  # 1小时前
        end = now
        
        url = f"{self.base_url}/api/v1/query_range"
        
        range_queries = [
            ("test_metric", "基础范围查询", "1m"),
            ("sum(test_metric)", "sum聚合范围查询", "1m"),
            ("avg(test_metric)", "avg聚合范围查询", "1m"),
            ("test_metric{job=\"job_0\"}", "标签过滤范围查询", "1m"),
        ]
        
        query_times = {}
        
        for query, name, step in range_queries:
            times = []
            for _ in range(3):
                start_time = time.time()
                try:
                    response = requests.get(url, params={
                        "query": query,
                        "start": start,
                        "end": end,
                        "step": step
                    }, timeout=60)
                    elapsed = time.time() - start_time
                    times.append(elapsed)
                except Exception as e:
                    print(f"范围查询失败 {name}: {e}")
                    times.append(float('inf'))
            
            avg_time = statistics.mean(times) * 1000  # ms
            query_times[name] = avg_time
            status = "✅" if avg_time < 1000 else "⚠️" if avg_time < 5000 else "❌"
            print(f"{status} {name}: {avg_time:.2f} ms")
        
        self.results["range_queries"] = query_times
        return True
    
    def test_metadata(self):
        """测试元数据查询"""
        print("\n=== 测试元数据查询 ===")
        
        tests = [
            ("/api/v1/labels", "标签列表"),
            ("/api/v1/label/job/values", "标签值"),
            ("/api/v1/series", "系列列表"),
        ]
        
        metadata_times = {}
        
        for endpoint, name in tests:
            times = []
            for _ in range(3):
                start = time.time()
                try:
                    response = requests.get(f"{self.base_url}{endpoint}", timeout=30)
                    elapsed = time.time() - start
                    times.append(elapsed)
                except Exception as e:
                    print(f"元数据查询失败 {name}: {e}")
                    times.append(float('inf'))
            
            avg_time = statistics.mean(times) * 1000  # ms
            metadata_times[name] = avg_time
            status = "✅" if avg_time < 500 else "⚠️" if avg_time < 2000 else "❌"
            print(f"{status} {name}: {avg_time:.2f} ms")
        
        self.results["metadata"] = metadata_times
        return True
    
    def run_all_tests(self):
        """运行所有测试"""
        print("="*60)
        print("ChronoDB 性能和功能测试")
        print("="*60)
        
        tests = [
            ("生成测试数据", self.generate_test_data),
            ("写入测试数据", self.write_data),
            ("即时查询性能", self.test_query_performance),
            ("范围查询性能", self.test_range_query),
            ("元数据查询", self.test_metadata),
        ]
        
        passed = 0
        failed = 0
        
        for test_name, test_func in tests:
            try:
                if test_func():
                    passed += 1
                    print(f"\n✅ {test_name} 通过")
                else:
                    failed += 1
                    print(f"\n❌ {test_name} 失败")
            except Exception as e:
                failed += 1
                print(f"\n❌ {test_name} 异常: {e}")
        
        self.print_summary()
        return failed == 0
    
    def print_summary(self):
        """打印测试结果摘要"""
        print("\n" + "="*60)
        print("测试结果摘要")
        print("="*60)
        
        config = self.size_config[self.data_size]
        print(f"\n数据规模: {config['description']}")
        
        if "write" in self.results:
            w = self.results["write"]
            print(f"\n【写入性能】")
            print(f"  总数据量: {w['total_samples']:,} samples")
            print(f"  总耗时: {w['total_time']:.2f}s")
            print(f"  写入吞吐量: {w['throughput_ksps']:.2f} K samples/s")
            print(f"  平均批次耗时: {w['avg_batch_time']*1000:.2f} ms")
        
        if "queries" in self.results:
            q = self.results["queries"]
            print(f"\n【即时查询性能 (ms)】")
            for name, time_ms in q.items():
                status = "✅" if time_ms < 500 else "⚠️" if time_ms < 2000 else "❌"
                print(f"  {status} {name}: {time_ms:.2f}")
        
        if "range_queries" in self.results:
            rq = self.results["range_queries"]
            print(f"\n【范围查询性能 (ms)】")
            for name, time_ms in rq.items():
                status = "✅" if time_ms < 1000 else "⚠️" if time_ms < 5000 else "❌"
                print(f"  {status} {name}: {time_ms:.2f}")
        
        print("\n" + "="*60)

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="ChronoDB 性能测试")
    parser.add_argument("--size", choices=["small", "medium", "large"], 
                        default="medium", help="测试数据规模")
    args = parser.parse_args()
    
    test = ChronoDBPerformanceTest(data_size=args.size)
    test.run_all_tests()
