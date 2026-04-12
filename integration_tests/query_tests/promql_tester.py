#!/usr/bin/env python3
"""
PromQL查询测试模块
测试各种PromQL算子和查询功能
"""

import time
import requests
from typing import Dict, List, Any, Optional, Tuple, Callable
from dataclasses import dataclass, field
from datetime import datetime

import sys
from pathlib import Path
sys.path.insert(0, str(Path(__file__).parent.parent))
from core.logger import get_logger
from core.config import get_config
from core.base_test import BaseTest, TestSuite, TestResult


@dataclass
class QueryResult:
    """查询结果"""
    query: str
    status: str  # success, error, timeout
    data: Optional[Any] = None
    error: Optional[str] = None
    duration_ms: float = 0.0
    timestamp: datetime = field(default_factory=datetime.now)
    
    def to_dict(self) -> Dict[str, Any]:
        return {
            'query': self.query,
            'status': self.status,
            'data': self.data,
            'error': self.error,
            'duration_ms': self.duration_ms,
            'timestamp': self.timestamp.isoformat()
        }


class PromQLQueryClient:
    """PromQL查询客户端"""
    
    def __init__(self, base_url: str, timeout: int = 60):
        self.base_url = base_url
        self.timeout = timeout
        self.logger = get_logger()
    
    def query(self, query: str, time_ms: Optional[int] = None) -> QueryResult:
        """执行即时查询"""
        start_time = time.time()
        
        url = f"{self.base_url}/api/v1/query"
        params = {"query": query}
        if time_ms:
            params["time"] = time_ms // 1000
        
        try:
            response = requests.get(url, params=params, timeout=self.timeout)
            duration_ms = (time.time() - start_time) * 1000
            
            if response.status_code == 200:
                data = response.json()
                if data.get("status") == "success":
                    return QueryResult(
                        query=query,
                        status="success",
                        data=data.get("data"),
                        duration_ms=duration_ms
                    )
                else:
                    return QueryResult(
                        query=query,
                        status="error",
                        error=data.get("error", "Unknown error"),
                        duration_ms=duration_ms
                    )
            else:
                return QueryResult(
                    query=query,
                    status="error",
                    error=f"HTTP {response.status_code}: {response.text}",
                    duration_ms=duration_ms
                )
        except requests.Timeout:
            return QueryResult(
                query=query,
                status="timeout",
                error="Request timeout",
                duration_ms=(time.time() - start_time) * 1000
            )
        except Exception as e:
            return QueryResult(
                query=query,
                status="error",
                error=str(e),
                duration_ms=(time.time() - start_time) * 1000
            )
    
    def query_range(self, query: str, start_ms: int, end_ms: int, 
                   step: str = "15s") -> QueryResult:
        """执行范围查询"""
        start_time = time.time()
        
        url = f"{self.base_url}/api/v1/query_range"
        params = {
            "query": query,
            "start": start_ms // 1000,
            "end": end_ms // 1000,
            "step": step
        }
        
        try:
            response = requests.get(url, params=params, timeout=self.timeout)
            duration_ms = (time.time() - start_time) * 1000
            
            if response.status_code == 200:
                data = response.json()
                if data.get("status") == "success":
                    return QueryResult(
                        query=query,
                        status="success",
                        data=data.get("data"),
                        duration_ms=duration_ms
                    )
                else:
                    return QueryResult(
                        query=query,
                        status="error",
                        error=data.get("error", "Unknown error"),
                        duration_ms=duration_ms
                    )
            else:
                return QueryResult(
                    query=query,
                    status="error",
                    error=f"HTTP {response.status_code}: {response.text}",
                    duration_ms=duration_ms
                )
        except requests.Timeout:
            return QueryResult(
                query=query,
                status="timeout",
                error="Request timeout",
                duration_ms=(time.time() - start_time) * 1000
            )
        except Exception as e:
            return QueryResult(
                query=query,
                status="error",
                error=str(e),
                duration_ms=(time.time() - start_time) * 1000
            )
    
    def get_labels(self) -> QueryResult:
        """获取所有标签名"""
        return self.query_labels()
    
    def query_labels(self, match: Optional[str] = None) -> QueryResult:
        """查询标签名"""
        start_time = time.time()
        
        url = f"{self.base_url}/api/v1/labels"
        params = {}
        if match:
            params["match[]"] = match
        
        try:
            response = requests.get(url, params=params, timeout=self.timeout)
            duration_ms = (time.time() - start_time) * 1000
            
            if response.status_code == 200:
                data = response.json()
                if data.get("status") == "success":
                    return QueryResult(
                        query="labels",
                        status="success",
                        data=data.get("data"),
                        duration_ms=duration_ms
                    )
                else:
                    return QueryResult(
                        query="labels",
                        status="error",
                        error=data.get("error"),
                        duration_ms=duration_ms
                    )
            else:
                return QueryResult(
                    query="labels",
                    status="error",
                    error=f"HTTP {response.status_code}",
                    duration_ms=duration_ms
                )
        except Exception as e:
            return QueryResult(
                query="labels",
                status="error",
                error=str(e),
                duration_ms=(time.time() - start_time) * 1000
            )
    
    def get_label_values(self, label: str) -> QueryResult:
        """获取标签值"""
        start_time = time.time()
        
        url = f"{self.base_url}/api/v1/label/{label}/values"
        
        try:
            response = requests.get(url, timeout=self.timeout)
            duration_ms = (time.time() - start_time) * 1000
            
            if response.status_code == 200:
                data = response.json()
                if data.get("status") == "success":
                    return QueryResult(
                        query=f"label_values({label})",
                        status="success",
                        data=data.get("data"),
                        duration_ms=duration_ms
                    )
                else:
                    return QueryResult(
                        query=f"label_values({label})",
                        status="error",
                        error=data.get("error"),
                        duration_ms=duration_ms
                    )
            else:
                return QueryResult(
                    query=f"label_values({label})",
                    status="error",
                    error=f"HTTP {response.status_code}",
                    duration_ms=duration_ms
                )
        except Exception as e:
            return QueryResult(
                query=f"label_values({label})",
                status="error",
                error=str(e),
                duration_ms=(time.time() - start_time) * 1000
            )
    
    def get_series(self, match: List[str]) -> QueryResult:
        """获取时间序列"""
        start_time = time.time()
        
        url = f"{self.base_url}/api/v1/series"
        params = [("match[]", m) for m in match]
        
        try:
            response = requests.get(url, params=params, timeout=self.timeout)
            duration_ms = (time.time() - start_time) * 1000
            
            if response.status_code == 200:
                data = response.json()
                if data.get("status") == "success":
                    return QueryResult(
                        query=f"series({match})",
                        status="success",
                        data=data.get("data"),
                        duration_ms=duration_ms
                    )
                else:
                    return QueryResult(
                        query=f"series({match})",
                        status="error",
                        error=data.get("error"),
                        duration_ms=duration_ms
                    )
            else:
                return QueryResult(
                    query=f"series({match})",
                    status="error",
                    error=f"HTTP {response.status_code}",
                    duration_ms=duration_ms
                )
        except Exception as e:
            return QueryResult(
                query=f"series({match})",
                status="error",
                error=str(e),
                duration_ms=(time.time() - start_time) * 1000
            )


class PromQLTestSuite(BaseTest):
    """PromQL测试套件"""
    
    def __init__(self, base_url: str, name: str = "PromQL测试"):
        super().__init__(name)
        self.client = PromQLQueryClient(base_url)
        self.config = get_config()
    
    def _do_setup(self):
        """测试准备"""
        pass
    
    def _do_teardown(self):
        """测试清理"""
        pass
    
    def test_simple_queries(self) -> TestSuite:
        """测试简单查询"""
        self.logger.subsection("简单查询测试")
        
        queries = [
            ("cpu_usage", "基本指标查询"),
            ("memory_usage", "内存指标查询"),
            ("up", "up指标查询"),
        ]
        
        for query, desc in queries:
            def test_fn(q=query):
                result = self.client.query(q)
                return result.status == "success", result.error or "查询成功"
            
            self.run_test(test_fn, desc)
        
        return self.suite
    
    def test_aggregation_queries(self) -> TestSuite:
        """测试聚合查询"""
        self.logger.subsection("聚合查询测试")
        
        aggregations = self.config.promql_operators.aggregations
        base_metrics = ["cpu_usage", "memory_usage"]
        
        for agg in aggregations:
            for metric in base_metrics:
                query = f"{agg}({metric})"
                desc = f"{agg}聚合-{metric}"
                
                def test_fn(q=query):
                    result = self.client.query(q)
                    return result.status == "success", result.error or "查询成功"
                
                self.run_test(test_fn, desc)
        
        # 测试带by的聚合
        by_aggregations = [
            "sum by (job) (cpu_usage)",
            "avg by (instance) (memory_usage)",
            "count by (region) (cpu_usage)",
            "max by (job, instance) (memory_usage)",
        ]
        
        for query in by_aggregations:
            def test_fn(q=query):
                result = self.client.query(q)
                return result.status == "success", result.error or "查询成功"
            
            self.run_test(test_fn, f"分组聚合: {query}")
        
        return self.suite
    
    def test_range_queries(self) -> TestSuite:
        """测试范围查询"""
        self.logger.subsection("范围查询测试")
        
        range_functions = self.config.promql_operators.range_functions
        base_metrics = ["cpu_usage", "memory_usage", "disk_io"]
        
        for func in range_functions:
            for metric in base_metrics:
                if func.endswith("("):  # 需要闭合括号
                    query = f"{func}{metric}[5m])"
                else:
                    query = f"{func}({metric}[5m])"
                
                desc = f"{func}-{metric}"
                
                def test_fn(q=query):
                    result = self.client.query(q)
                    return result.status == "success", result.error or "查询成功"
                
                self.run_test(test_fn, desc)
        
        return self.suite
    
    def test_filter_queries(self) -> TestSuite:
        """测试过滤查询"""
        self.logger.subsection("过滤查询测试")
        
        filter_queries = [
            ('cpu_usage{job="webserver"}', "等值过滤"),
            ('cpu_usage{job=~"web.*"}', "正则匹配"),
            ('cpu_usage{job!="database"}', "不等过滤"),
            ('cpu_usage{job!~"cache.*"}', "正则不匹配"),
            ('cpu_usage{job="webserver",region="us-east-1"}', "多标签过滤"),
            ('cpu_usage{job=~"webserver|database"}', "多值匹配"),
            ('{__name__=~"cpu.*"}', "指标名正则"),
        ]
        
        for query, desc in filter_queries:
            def test_fn(q=query):
                result = self.client.query(q)
                return result.status == "success", result.error or "查询成功"
            
            self.run_test(test_fn, f"{desc}: {query}")
        
        return self.suite
    
    def test_math_queries(self) -> TestSuite:
        """测试数学函数查询"""
        self.logger.subsection("数学函数查询测试")
        
        math_functions = self.config.promql_operators.math_functions
        base_metrics = ["cpu_usage", "memory_usage"]
        
        for func in math_functions:
            for metric in base_metrics:
                query = f"{func}({metric})"
                desc = f"{func}-{metric}"
                
                def test_fn(q=query):
                    result = self.client.query(q)
                    return result.status == "success", result.error or "查询成功"
                
                self.run_test(test_fn, desc)
        
        return self.suite
    
    def test_binary_operators(self) -> TestSuite:
        """测试二元运算符"""
        self.logger.subsection("二元运算符测试")
        
        binary_queries = [
            ("cpu_usage + memory_usage", "加法"),
            ("cpu_usage - memory_usage", "减法"),
            ("cpu_usage * 2", "乘法"),
            ("cpu_usage / 100", "除法"),
            ("cpu_usage % 10", "取模"),
            ("cpu_usage > 50", "大于"),
            ("cpu_usage < 80", "小于"),
            ("cpu_usage >= 30", "大于等于"),
            ("cpu_usage <= 90", "小于等于"),
            ("cpu_usage == 50", "等于"),
            ("cpu_usage != 0", "不等于"),
        ]
        
        for query, desc in binary_queries:
            def test_fn(q=query):
                result = self.client.query(q)
                return result.status == "success", result.error or "查询成功"
            
            self.run_test(test_fn, f"{desc}: {query}")
        
        return self.suite
    
    def test_set_operators(self) -> TestSuite:
        """测试集合运算符"""
        self.logger.subsection("集合运算符测试")
        
        set_queries = [
            ("cpu_usage and memory_usage", "AND运算"),
            ("cpu_usage or memory_usage", "OR运算"),
            ("cpu_usage unless memory_usage", "UNLESS运算"),
        ]
        
        for query, desc in set_queries:
            def test_fn(q=query):
                result = self.client.query(q)
                return result.status == "success", result.error or "查询成功"
            
            self.run_test(test_fn, f"{desc}: {query}")
        
        return self.suite
    
    def test_time_functions(self) -> TestSuite:
        """测试时间函数"""
        self.logger.subsection("时间函数测试")
        
        time_functions = self.config.promql_operators.time_functions
        
        for func in time_functions:
            query = f"{func}()" if not func.endswith(")") else func
            desc = func
            
            def test_fn(q=query):
                result = self.client.query(q)
                return result.status == "success", result.error or "查询成功"
            
            self.run_test(test_fn, desc)
        
        return self.suite
    
    def test_query_range_api(self, start_ms: int, end_ms: int) -> TestSuite:
        """测试范围查询API"""
        self.logger.subsection("范围查询API测试")
        
        steps = self.config.test_config.query_steps
        time_ranges = self.config.test_config.query_time_ranges
        base_metrics = ["cpu_usage", "memory_usage"]
        
        for metric in base_metrics:
            for step in steps:
                desc = f"{metric}[step={step}]"
                
                def test_fn(m=metric, s=step):
                    result = self.client.query_range(m, start_ms, end_ms, s)
                    return result.status == "success", result.error or "查询成功"
                
                self.run_test(test_fn, desc)
        
        return self.suite
    
    def test_metadata_apis(self) -> TestSuite:
        """测试元数据API"""
        self.logger.subsection("元数据API测试")
        
        # 测试标签列表
        def test_labels():
            result = self.client.get_labels()
            return result.status == "success", result.error or "查询成功"
        
        self.run_test(test_labels, "获取标签列表")
        
        # 测试标签值
        label_names = ["__name__", "job", "instance", "region"]
        for label in label_names:
            def test_fn(l=label):
                result = self.client.get_label_values(l)
                return result.status == "success", result.error or "查询成功"
            
            self.run_test(test_fn, f"获取标签值: {label}")
        
        # 测试序列查询
        def test_series():
            result = self.client.get_series(["cpu_usage", "memory_usage"])
            return result.status == "success", result.error or "查询成功"
        
        self.run_test(test_series, "获取时间序列")
        
        return self.suite
    
    def test_complex_queries(self) -> TestSuite:
        """测试复杂查询"""
        self.logger.subsection("复杂查询测试")
        
        complex_queries = [
            ("sum(rate(cpu_usage[5m])) by (job)", "速率聚合"),
            ("histogram_quantile(0.95, sum(rate(request_duration_bucket[5m])) by (le))", "P95延迟"),
            ("cpu_usage / ignoring(instance) group_left sum(cpu_usage) by (job)", "占比计算"),
            ("topk(5, cpu_usage)", "TopK查询"),
            ("bottomk(5, cpu_usage)", "BottomK查询"),
            ("sort(cpu_usage)", "排序"),
            ("sort_desc(cpu_usage)", "降序排序"),
            ("clamp(cpu_usage, 0, 100)", "限幅"),
            ("changes(cpu_usage[1h])", "变化次数"),
            ("resets(cpu_usage[1h])", "重置次数"),
            ("avg_over_time(cpu_usage[1h])", "1小时平均"),
            ("max_over_time(cpu_usage[1h])", "1小时最大"),
        ]
        
        for query, desc in complex_queries:
            def test_fn(q=query):
                result = self.client.query(q)
                return result.status == "success", result.error or "查询成功"
            
            self.run_test(test_fn, f"{desc}: {query}")
        
        return self.suite
    
    def run_all_tests(self, start_ms: Optional[int] = None, 
                     end_ms: Optional[int] = None) -> TestSuite:
        """运行所有PromQL测试"""
        self.setup()
        
        try:
            self.test_simple_queries()
            self.test_aggregation_queries()
            self.test_range_queries()
            self.test_filter_queries()
            self.test_math_queries()
            self.test_binary_operators()
            self.test_set_operators()
            self.test_time_functions()
            
            if start_ms and end_ms:
                self.test_query_range_api(start_ms, end_ms)
            
            self.test_metadata_apis()
            self.test_complex_queries()
        
        finally:
            self.teardown()
        
        return self.suite


# 便捷函数
def create_promql_tester(base_url: str) -> PromQLTestSuite:
    """创建PromQL测试套件"""
    return PromQLTestSuite(base_url)
