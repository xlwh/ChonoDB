#!/usr/bin/env python3
"""
预聚合功能测试模块
测试 ChronoDB 的预聚合功能
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
class PreaggregationResult:
    """预聚合测试结果"""
    test_name: str
    status: str
    query_time_ms: float = 0.0
    raw_query_time_ms: float = 0.0
    speedup_ratio: float = 0.0
    result_count: int = 0
    details: Dict[str, Any] = field(default_factory=dict)


class PreaggregationTester(BaseTest):
    """预聚合功能测试器"""

    def __init__(self, chronodb_url: str = "http://localhost:9091"):
        super().__init__("预聚合测试")
        self.chronodb_url = chronodb_url
        self.config = get_config()
        self.results: List[PreaggregationResult] = []

    def _do_setup(self):
        """抽象方法实现"""
        pass

    def _do_teardown(self):
        """抽象方法实现"""
        pass

    def setup(self):
        """测试准备"""
        self.logger.info("准备预聚合测试数据...")
        self._write_multi_resolution_data()
    
    def _write_multi_resolution_data(self):
        """写入多分辨率测试数据"""
        import random
        
        # 生成不同分辨率的数据
        metrics = [
            ("raw_metric", 1000),      # 1秒分辨率
            ("min5_metric", 300000),   # 5分钟分辨率
            ("hour_metric", 3600000),  # 1小时分辨率
        ]
        
        end_time = int(time.time() * 1000)
        
        for metric_name, resolution in metrics:
            samples = []
            start_time = end_time - 24 * 3600 * 1000  # 24小时前
            
            for ts in range(start_time, end_time, resolution):
                value = random.uniform(10, 100)
                samples.append({"timestamp": ts, "value": value})
            
            data = {
                "labels": {
                    "__name__": metric_name,
                    "job": "preagg_test",
                    "resolution": str(resolution)
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
                    self.logger.info(f"成功写入 {metric_name}: {len(samples)} 个样本")
            except Exception as e:
                self.logger.error(f"写入 {metric_name} 失败: {e}")
    
    def test_preaggregation_performance(self) -> PreaggregationResult:
        """测试预聚合性能提升"""
        self.logger.info("测试预聚合性能...")
        
        try:
            # 测试原始数据查询（需要实时聚合）
            raw_times = []
            for _ in range(5):
                start = time.perf_counter()
                response = requests.post(
                    f"{self.chronodb_url}/api/v1/query",
                    data={"query": "sum(raw_metric)"},
                    timeout=30
                )
                raw_times.append((time.perf_counter() - start) * 1000)
            
            raw_avg_time = statistics.mean(raw_times)
            
            # 测试预聚合数据查询
            preagg_times = []
            for _ in range(5):
                start = time.perf_counter()
                response = requests.post(
                    f"{self.chronodb_url}/api/v1/query",
                    data={"query": "sum(hour_metric)"},
                    timeout=30
                )
                preagg_times.append((time.perf_counter() - start) * 1000)
            
            preagg_avg_time = statistics.mean(preagg_times)
            
            # 计算性能提升
            speedup = raw_avg_time / preagg_avg_time if preagg_avg_time > 0 else 0
            
            self.logger.info(f"原始查询: {raw_avg_time:.2f}ms, 预聚合查询: {preagg_avg_time:.2f}ms, 加速比: {speedup:.2f}x")
            
            return PreaggregationResult(
                test_name="preaggregation_performance",
                status="success",
                query_time_ms=preagg_avg_time,
                raw_query_time_ms=raw_avg_time,
                speedup_ratio=speedup,
                details={
                    "raw_times": raw_times,
                    "preagg_times": preagg_times,
                    "speedup": speedup
                }
            )
            
        except Exception as e:
            return PreaggregationResult(
                test_name="preaggregation_performance",
                status="error",
                details={"error": str(e)}
            )
    
    def test_aggregation_accuracy(self) -> PreaggregationResult:
        """测试预聚合精度"""
        self.logger.info("测试预聚合精度...")
        
        try:
            # 查询原始数据的聚合结果
            raw_response = requests.post(
                f"{self.chronodb_url}/api/v1/query",
                data={"query": "avg(raw_metric)"},
                timeout=30
            )
            
            # 查询预聚合数据
            preagg_response = requests.post(
                f"{self.chronodb_url}/api/v1/query",
                data={"query": "avg(hour_metric)"},
                timeout=30
            )
            
            if raw_response.status_code == 200 and preagg_response.status_code == 200:
                raw_data = raw_response.json()
                preagg_data = preagg_response.json()
                
                raw_value = float(raw_data.get('data', {}).get('result', [{}])[0].get('value', [0, 0])[1])
                preagg_value = float(preagg_data.get('data', {}).get('result', [{}])[0].get('value', [0, 0])[1])
                
                # 计算误差（预聚合和实时聚合之间允许一定误差）
                error_rate = abs(raw_value - preagg_value) / raw_value * 100 if raw_value != 0 else 0
                
                self.logger.info(f"原始聚合: {raw_value:.4f}, 预聚合: {preagg_value:.4f}, 误差: {error_rate:.2f}%")
                
                return PreaggregationResult(
                    test_name="aggregation_accuracy",
                    status="success" if error_rate < 10 else "warning",
                    details={
                        "raw_value": raw_value,
                        "preagg_value": preagg_value,
                        "error_rate": error_rate
                    }
                )
            else:
                return PreaggregationResult(
                    test_name="aggregation_accuracy",
                    status="failed"
                )
                
        except Exception as e:
            return PreaggregationResult(
                test_name="aggregation_accuracy",
                status="error",
                details={"error": str(e)}
            )
    
    def test_range_query_optimization(self) -> PreaggregationResult:
        """测试范围查询优化"""
        self.logger.info("测试范围查询优化...")
        
        try:
            end_time = int(time.time() * 1000)
            start_time = end_time - 7 * 24 * 3600 * 1000  # 7天前
            
            # 大范围查询原始数据
            raw_start = time.perf_counter()
            raw_response = requests.post(
                f"{self.chronodb_url}/api/v1/query_range",
                data={
                    "query": "raw_metric",
                    "start": str(start_time),
                    "end": str(end_time),
                    "step": "3600"  # 1小时步长
                },
                timeout=60
            )
            raw_time = (time.perf_counter() - raw_start) * 1000
            
            # 查询预聚合数据
            preagg_start = time.perf_counter()
            preagg_response = requests.post(
                f"{self.chronodb_url}/api/v1/query_range",
                data={
                    "query": "hour_metric",
                    "start": str(start_time),
                    "end": str(end_time),
                    "step": "3600"
                },
                timeout=30
            )
            preagg_time = (time.perf_counter() - preagg_start) * 1000
            
            if raw_response.status_code == 200 and preagg_response.status_code == 200:
                raw_data = raw_response.json()
                preagg_data = preagg_response.json()
                
                raw_count = len(raw_data.get('data', {}).get('result', [{}])[0].get('values', []))
                preagg_count = len(preagg_data.get('data', {}).get('result', [{}])[0].get('values', []))
                
                speedup = raw_time / preagg_time if preagg_time > 0 else 0
                
                self.logger.info(f"原始查询: {raw_time:.2f}ms ({raw_count} 点), 预聚合: {preagg_time:.2f}ms ({preagg_count} 点), 加速: {speedup:.2f}x")
                
                return PreaggregationResult(
                    test_name="range_query_optimization",
                    status="success",
                    query_time_ms=preagg_time,
                    raw_query_time_ms=raw_time,
                    speedup_ratio=speedup,
                    result_count=preagg_count,
                    details={
                        "raw_count": raw_count,
                        "preagg_count": preagg_count,
                        "time_range": "7d"
                    }
                )
            else:
                return PreaggregationResult(
                    test_name="range_query_optimization",
                    status="failed",
                    details={
                        "raw_status": raw_response.status_code,
                        "preagg_status": preagg_response.status_code
                    }
                )
                
        except Exception as e:
            return PreaggregationResult(
                test_name="range_query_optimization",
                status="error",
                details={"error": str(e)}
            )
    
    def test_multi_resolution_query(self) -> List[PreaggregationResult]:
        """测试多分辨率查询"""
        self.logger.info("测试多分辨率查询...")
        
        resolutions = [
            ("raw", "raw_metric", 1000),
            ("5min", "min5_metric", 300000),
            ("1hour", "hour_metric", 3600000),
        ]
        
        results = []
        for name, metric, resolution_ms in resolutions:
            try:
                start = time.perf_counter()
                response = requests.post(
                    f"{self.chronodb_url}/api/v1/query",
                    data={"query": f"sum({metric})"},
                    timeout=30
                )
                duration_ms = (time.perf_counter() - start) * 1000
                
                if response.status_code == 200:
                    results.append(PreaggregationResult(
                        test_name=f"multi_resolution_{name}",
                        status="success",
                        query_time_ms=duration_ms,
                        details={"resolution": resolution_ms, "metric": metric}
                    ))
                    self.logger.info(f"  {name} ({resolution_ms}ms): {duration_ms:.2f}ms")
                else:
                    results.append(PreaggregationResult(
                        test_name=f"multi_resolution_{name}",
                        status="failed"
                    ))
            except Exception as e:
                results.append(PreaggregationResult(
                    test_name=f"multi_resolution_{name}",
                    status="error",
                    details={"error": str(e)}
                ))
        
        return results
    
    def test_complex_aggregation(self) -> PreaggregationResult:
        """测试复杂预聚合查询"""
        self.logger.info("测试复杂预聚合查询...")
        
        try:
            queries = [
                ("simple_sum", "sum(hour_metric)"),
                ("avg_by_time", "avg(hour_metric)"),
                ("rate_calc", "rate(hour_metric[1h])"),
                ("complex_agg", "sum(rate(hour_metric[1h])) by (job)"),
            ]
            
            query_times = {}
            for name, query in queries:
                start = time.perf_counter()
                response = requests.post(
                    f"{self.chronodb_url}/api/v1/query",
                    data={"query": query},
                    timeout=30
                )
                duration_ms = (time.perf_counter() - start) * 1000
                query_times[name] = duration_ms
                
                if response.status_code == 200:
                    self.logger.info(f"  {name}: {duration_ms:.2f}ms")
            
            avg_time = statistics.mean(query_times.values()) if query_times else 0
            
            return PreaggregationResult(
                test_name="complex_aggregation",
                status="success",
                query_time_ms=avg_time,
                details={"query_times": query_times}
            )
            
        except Exception as e:
            return PreaggregationResult(
                test_name="complex_aggregation",
                status="error",
                details={"error": str(e)}
            )
    
    def run_all_tests(self) -> TestSuite:
        """运行所有预聚合测试"""
        self.logger.section("预聚合功能测试")
        
        suite = TestSuite("预聚合测试")
        
        # 准备数据
        self.setup()
        
        # 运行测试
        tests = [
            ("预聚合性能", self.test_preaggregation_performance),
            ("聚合精度", self.test_aggregation_accuracy),
            ("范围查询优化", self.test_range_query_optimization),
            ("复杂聚合", self.test_complex_aggregation),
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
                            duration_ms=r.query_time_ms,
                            details=r.details
                        ))
                else:
                    self.results.append(result)
                    suite.add_result(TestResult(
                        name=result.test_name,
                        status=result.status == "success",
                        duration_ms=result.query_time_ms,
                        details=result.details
                    ))
            except Exception as e:
                self.logger.error(f"测试 {test_name} 失败: {e}")
                suite.add_result(TestResult(
                    name=test_name,
                    status=False,
                    error=str(e)
                ))
        
        # 运行多分辨率测试
        multi_results = self.test_multi_resolution_query()
        for r in multi_results:
            suite.add_result(TestResult(
                name=r.test_name,
                status=r.status == "success",
                duration_ms=r.query_time_ms,
                details=r.details
            ))
        
        # 汇总结果
        passed = sum(1 for r in suite.results if r.status)
        total = len(suite.results)
        self.logger.info(f"预聚合测试完成: {passed}/{total} 通过")
        
        return suite


def create_preaggregation_tester(chronodb_url: str = "http://localhost:9091") -> PreaggregationTester:
    """创建预聚合测试器"""
    return PreaggregationTester(chronodb_url)
