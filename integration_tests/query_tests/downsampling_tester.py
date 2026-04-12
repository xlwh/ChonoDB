#!/usr/bin/env python3
"""
降采样功能测试模块
测试 ChronoDB 的降采样和预聚合功能
"""

import time
import requests
import statistics
from typing import Dict, List, Any, Optional, Tuple
from dataclasses import dataclass, field
from datetime import datetime, timedelta

import sys
from pathlib import Path
sys.path.insert(0, str(Path(__file__).parent.parent))
from core.logger import get_logger
from core.config import get_config
from core.base_test import BaseTest, TestSuite, TestResult


@dataclass
class DownsamplingResult:
    """降采样测试结果"""
    test_name: str
    status: str
    original_samples: int = 0
    downsampled_samples: int = 0
    compression_ratio: float = 0.0
    error_rate: float = 0.0
    duration_ms: float = 0.0
    details: Dict[str, Any] = field(default_factory=dict)


class DownsamplingTester(BaseTest):
    """降采样功能测试器"""

    def __init__(self, chronodb_url: str = "http://localhost:9091"):
        super().__init__("降采样测试")
        self.chronodb_url = chronodb_url
        self.config = get_config()
        self.results: List[DownsamplingResult] = []

    def _do_setup(self):
        """抽象方法实现"""
        pass

    def _do_teardown(self):
        """抽象方法实现"""
        pass

    def setup(self):
        """测试准备"""
        self.logger.info("准备降采样测试数据...")
        # 写入高频数据用于降采样测试
        self._write_high_frequency_data()
    
    def _write_high_frequency_data(self):
        """写入高频测试数据"""
        import random
        
        # 生成每秒一个样本的数据（1小时 = 3600个样本）
        end_time = int(time.time() * 1000)
        start_time = end_time - 3600 * 1000  # 1小时前
        
        samples = []
        for ts in range(start_time, end_time, 1000):  # 每秒一个点
            value = 50 + 20 * random.random() + 10 * random.sin(ts / 60000)  # 模拟波动
            samples.append({"timestamp": ts, "value": value})
        
        # 写入数据
        data = {
            "labels": {
                "__name__": "high_freq_metric",
                "job": "test",
                "instance": "localhost:9090"
            },
            "samples": samples
        }
        
        try:
            response = requests.post(
                f"{self.chronodb_url}/api/v1/write",
                json=data,
                timeout=30
            )
            if response.status_code == 200:
                self.logger.info(f"成功写入 {len(samples)} 个高频样本")
            else:
                self.logger.warning(f"写入数据失败: {response.status_code}")
        except Exception as e:
            self.logger.error(f"写入数据异常: {e}")
    
    def test_basic_downsampling(self) -> DownsamplingResult:
        """测试基本降采样功能"""
        self.logger.info("测试基本降采样...")
        
        start_time = time.perf_counter()
        
        try:
            # 查询原始数据
            original_query = 'high_freq_metric[1h]'
            original_response = requests.post(
                f"{self.chronodb_url}/api/v1/query",
                data={"query": original_query},
                timeout=30
            )
            
            # 查询降采样后的数据（使用5分钟降采样）
            downsampled_query = 'avg_over_time(high_freq_metric[5m])'
            downsampled_response = requests.post(
                f"{self.chronodb_url}/api/v1/query",
                data={"query": downsampled_query},
                timeout=30
            )
            
            duration_ms = (time.perf_counter() - start_time) * 1000
            
            if original_response.status_code == 200 and downsampled_response.status_code == 200:
                original_data = original_response.json()
                downsampled_data = downsampled_response.json()
                
                original_count = len(original_data.get('data', {}).get('result', [{}])[0].get('values', []))
                downsampled_count = len(downsampled_data.get('data', {}).get('result', [{}])[0].get('values', []))
                
                compression = original_count / downsampled_count if downsampled_count > 0 else 0
                
                result = DownsamplingResult(
                    test_name="basic_downsampling",
                    status="success",
                    original_samples=original_count,
                    downsampled_samples=downsampled_count,
                    compression_ratio=compression,
                    duration_ms=duration_ms,
                    details={
                        "original_query": original_query,
                        "downsampled_query": downsampled_query,
                        "expected_ratio": 300,  # 5分钟 / 1秒 = 300
                    }
                )
                
                self.logger.info(f"降采样压缩比: {compression:.1f}x (原始: {original_count}, 降采样后: {downsampled_count})")
                return result
            else:
                return DownsamplingResult(
                    test_name="basic_downsampling",
                    status="failed",
                    duration_ms=duration_ms,
                    details={"error": "Query failed"}
                )
                
        except Exception as e:
            return DownsamplingResult(
                test_name="basic_downsampling",
                status="error",
                details={"error": str(e)}
            )
    
    def test_downsampling_aggregation_methods(self) -> List[DownsamplingResult]:
        """测试不同聚合方法的降采样"""
        self.logger.info("测试不同聚合方法的降采样...")
        
        methods = [
            ("avg_over_time", "avg_over_time(high_freq_metric[5m])"),
            ("min_over_time", "min_over_time(high_freq_metric[5m])"),
            ("max_over_time", "max_over_time(high_freq_metric[5m])"),
            ("sum_over_time", "sum_over_time(high_freq_metric[5m])"),
        ]
        
        results = []
        for method_name, query in methods:
            try:
                start = time.perf_counter()
                response = requests.post(
                    f"{self.chronodb_url}/api/v1/query",
                    data={"query": query},
                    timeout=30
                )
                duration_ms = (time.perf_counter() - start) * 1000
                
                if response.status_code == 200:
                    data = response.json()
                    values = data.get('data', {}).get('result', [{}])[0].get('values', [])
                    
                    results.append(DownsamplingResult(
                        test_name=f"downsampling_{method_name}",
                        status="success",
                        downsampled_samples=len(values),
                        duration_ms=duration_ms,
                        details={"method": method_name, "query": query}
                    ))
                    self.logger.info(f"  {method_name}: {len(values)} 个点, {duration_ms:.2f}ms")
                else:
                    results.append(DownsamplingResult(
                        test_name=f"downsampling_{method_name}",
                        status="failed",
                        duration_ms=duration_ms
                    ))
            except Exception as e:
                results.append(DownsamplingResult(
                    test_name=f"downsampling_{method_name}",
                    status="error",
                    details={"error": str(e)}
                ))
        
        return results
    
    def test_auto_downsampling(self) -> DownsamplingResult:
        """测试自动降采样功能"""
        self.logger.info("测试自动降采样...")
        
        try:
            # 查询大范围数据，触发自动降采样
            start = time.perf_counter()
            response = requests.post(
                f"{self.chronodb_url}/api/v1/query_range",
                data={
                    "query": "high_freq_metric",
                    "start": str(int((time.time() - 3600) * 1000)),  # 1小时前
                    "end": str(int(time.time() * 1000)),
                    "step": "60"  # 1分钟步长
                },
                timeout=30
            )
            duration_ms = (time.perf_counter() - start) * 1000
            
            if response.status_code == 200:
                data = response.json()
                result_count = len(data.get('data', {}).get('result', [{}])[0].get('values', []))
                
                return DownsamplingResult(
                    test_name="auto_downsampling",
                    status="success",
                    downsampled_samples=result_count,
                    duration_ms=duration_ms,
                    details={"step": "60s", "result_count": result_count}
                )
            else:
                return DownsamplingResult(
                    test_name="auto_downsampling",
                    status="failed",
                    duration_ms=duration_ms
                )
        except Exception as e:
            return DownsamplingResult(
                test_name="auto_downsampling",
                status="error",
                details={"error": str(e)}
            )
    
    def test_downsampling_accuracy(self) -> DownsamplingResult:
        """测试降采样精度"""
        self.logger.info("测试降采样精度...")
        
        try:
            # 获取原始数据
            original_resp = requests.post(
                f"{self.chronodb_url}/api/v1/query",
                data={"query": "high_freq_metric[10m]"},
                timeout=30
            )
            
            # 获取降采样数据
            downsampled_resp = requests.post(
                f"{self.chronodb_url}/api/v1/query",
                data={"query": "avg_over_time(high_freq_metric[10m])"},
                timeout=30
            )
            
            if original_resp.status_code == 200 and downsampled_resp.status_code == 200:
                original_data = original_resp.json()
                downsampled_data = downsampled_resp.json()
                
                original_values = [v[1] for v in original_data.get('data', {}).get('result', [{}])[0].get('values', [])]
                downsampled_value = downsampled_data.get('data', {}).get('result', [{}])[0].get('value', [0, 0])[1]
                
                if original_values:
                    original_avg = statistics.mean([float(v) for v in original_values])
                    downsampled_avg = float(downsampled_value)
                    
                    # 计算误差率
                    error_rate = abs(original_avg - downsampled_avg) / original_avg * 100 if original_avg != 0 else 0
                    
                    self.logger.info(f"原始平均值: {original_avg:.4f}, 降采样值: {downsampled_avg:.4f}, 误差: {error_rate:.2f}%")
                    
                    return DownsamplingResult(
                        test_name="downsampling_accuracy",
                        status="success" if error_rate < 5 else "warning",
                        error_rate=error_rate,
                        details={
                            "original_avg": original_avg,
                            "downsampled_avg": downsampled_avg,
                            "error_rate": error_rate
                        }
                    )
            
            return DownsamplingResult(
                test_name="downsampling_accuracy",
                status="failed"
            )
            
        except Exception as e:
            return DownsamplingResult(
                test_name="downsampling_accuracy",
                status="error",
                details={"error": str(e)}
            )
    
    def run_all_tests(self) -> TestSuite:
        """运行所有降采样测试"""
        self.logger.section("降采样功能测试")
        
        suite = TestSuite("降采样测试")
        
        # 准备数据
        self.setup()
        
        # 运行测试
        tests = [
            ("基本降采样", self.test_basic_downsampling),
            ("自动降采样", self.test_auto_downsampling),
            ("降采样精度", self.test_downsampling_accuracy),
        ]
        
        for test_name, test_func in tests:
            try:
                result = test_func()
                if isinstance(result, list):
                    for r in result:
                        self.results.append(r)
                        suite.add_result(TestResult(
                            name=r.test_name,
                            status=r.status == "success",
                            duration_ms=r.duration_ms,
                            details=r.details
                        ))
                else:
                    self.results.append(result)
                    suite.add_result(TestResult(
                        name=result.test_name,
                        status=result.status == "success",
                        duration_ms=result.duration_ms,
                        details=result.details
                    ))
            except Exception as e:
                self.logger.error(f"测试 {test_name} 失败: {e}")
                suite.add_result(TestResult(
                    name=test_name,
                    status=False,
                    error=str(e)
                ))
        
        # 运行聚合方法测试
        method_results = self.test_downsampling_aggregation_methods()
        for r in method_results:
            suite.add_result(TestResult(
                name=r.test_name,
                status=r.status == "success",
                duration_ms=r.duration_ms,
                details=r.details
            ))
        
        # 汇总结果
        passed = sum(1 for r in suite.results if r.status)
        total = len(suite.results)
        self.logger.info(f"降采样测试完成: {passed}/{total} 通过")
        
        return suite


def create_downsampling_tester(chronodb_url: str = "http://localhost:9091") -> DownsamplingTester:
    """创建降采样测试器"""
    return DownsamplingTester(chronodb_url)
