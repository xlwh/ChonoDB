#!/usr/bin/env python3
"""
结果对比模块
对比Prometheus和ChronoDB的查询结果
"""

import json
from typing import Dict, List, Any, Optional, Tuple, Callable
from dataclasses import dataclass, field
from datetime import datetime

import sys
from pathlib import Path
sys.path.insert(0, str(Path(__file__).parent.parent))
from core.logger import get_logger
from core.config import get_config


@dataclass
class ComparisonResult:
    """对比结果"""
    query: str
    prometheus_result: Optional[Any] = None
    chronodb_result: Optional[Any] = None
    match: bool = False
    differences: List[Dict[str, Any]] = field(default_factory=list)
    prometheus_duration_ms: float = 0.0
    chronodb_duration_ms: float = 0.0
    timestamp: datetime = field(default_factory=datetime.now)
    
    def to_dict(self) -> Dict[str, Any]:
        return {
            'query': self.query,
            'match': self.match,
            'differences': self.differences,
            'prometheus_duration_ms': self.prometheus_duration_ms,
            'chronodb_duration_ms': self.chronodb_duration_ms,
            'timestamp': self.timestamp.isoformat()
        }


@dataclass
class ComparisonReport:
    """对比报告"""
    total_queries: int = 0
    matched_queries: int = 0
    mismatched_queries: int = 0
    prometheus_total_duration_ms: float = 0.0
    chronodb_total_duration_ms: float = 0.0
    results: List[ComparisonResult] = field(default_factory=list)
    start_time: Optional[datetime] = None
    end_time: Optional[datetime] = None
    
    @property
    def match_rate(self) -> float:
        if self.total_queries == 0:
            return 0.0
        return self.matched_queries / self.total_queries * 100
    
    @property
    def avg_prometheus_duration_ms(self) -> float:
        if self.total_queries == 0:
            return 0.0
        return self.prometheus_total_duration_ms / self.total_queries
    
    @property
    def avg_chronodb_duration_ms(self) -> float:
        if self.total_queries == 0:
            return 0.0
        return self.chronodb_total_duration_ms / self.total_queries
    
    @property
    def performance_ratio(self) -> float:
        """ChronoDB相对于Prometheus的性能比"""
        if self.avg_prometheus_duration_ms == 0:
            return 1.0
        return self.avg_chronodb_duration_ms / self.avg_prometheus_duration_ms
    
    def to_dict(self) -> Dict[str, Any]:
        return {
            'total_queries': self.total_queries,
            'matched_queries': self.matched_queries,
            'mismatched_queries': self.mismatched_queries,
            'match_rate': self.match_rate,
            'prometheus_total_duration_ms': self.prometheus_total_duration_ms,
            'chronodb_total_duration_ms': self.chronodb_total_duration_ms,
            'avg_prometheus_duration_ms': self.avg_prometheus_duration_ms,
            'avg_chronodb_duration_ms': self.avg_chronodb_duration_ms,
            'performance_ratio': self.performance_ratio,
            'start_time': self.start_time.isoformat() if self.start_time else None,
            'end_time': self.end_time.isoformat() if self.end_time else None,
            'results': [r.to_dict() for r in self.results]
        }


class ResultComparator:
    """结果对比器"""
    
    def __init__(self, value_tolerance: float = 0.01, 
                 timestamp_tolerance_ms: int = 1000):
        self.logger = get_logger()
        self.config = get_config()
        self.value_tolerance = value_tolerance
        self.timestamp_tolerance_ms = timestamp_tolerance_ms
        self.report = ComparisonReport()
    
    def compare_query_results(self, query: str,
                             prometheus_result: Dict[str, Any],
                             chronodb_result: Dict[str, Any],
                             prometheus_duration_ms: float = 0.0,
                             chronodb_duration_ms: float = 0.0) -> ComparisonResult:
        """
        对比单次查询结果
        
        Args:
            query: 查询语句
            prometheus_result: Prometheus查询结果
            chronodb_result: ChronoDB查询结果
            prometheus_duration_ms: Prometheus查询耗时
            chronodb_duration_ms: ChronoDB查询耗时
        
        Returns:
            对比结果
        """
        result = ComparisonResult(
            query=query,
            prometheus_result=prometheus_result,
            chronodb_result=chronodb_result,
            prometheus_duration_ms=prometheus_duration_ms,
            chronodb_duration_ms=chronodb_duration_ms
        )
        
        # 检查状态
        prom_status = prometheus_result.get("status") if prometheus_result else None
        chrono_status = chronodb_result.get("status") if chronodb_result else None
        
        if prom_status != "success" and chrono_status != "success":
            # 两者都失败，视为匹配
            result.match = True
        elif prom_status != "success":
            result.differences.append({
                "type": "status_mismatch",
                "prometheus": prom_status,
                "chronodb": chrono_status
            })
        elif chrono_status != "success":
            result.differences.append({
                "type": "status_mismatch",
                "prometheus": prom_status,
                "chronodb": chrono_status
            })
        else:
            # 两者都成功，对比数据
            prom_data = prometheus_result.get("data", {})
            chrono_data = chronodb_result.get("data", {})
            
            result.match = self._compare_data(prom_data, chrono_data, result.differences)
        
        # 更新报告
        self.report.total_queries += 1
        self.report.prometheus_total_duration_ms += prometheus_duration_ms
        self.report.chronodb_total_duration_ms += chronodb_duration_ms
        
        if result.match:
            self.report.matched_queries += 1
        else:
            self.report.mismatched_queries += 1
        
        self.report.results.append(result)
        
        return result
    
    def _compare_data(self, prom_data: Any, chrono_data: Any, 
                     differences: List[Dict[str, Any]], path: str = "") -> bool:
        """递归对比数据"""
        # 获取结果类型
        prom_type = prom_data.get("resultType") if isinstance(prom_data, dict) else None
        chrono_type = chrono_data.get("resultType") if isinstance(chrono_data, dict) else None
        
        if prom_type != chrono_type:
            differences.append({
                "type": "result_type_mismatch",
                "path": path,
                "prometheus": prom_type,
                "chronodb": chrono_type
            })
            return False
        
        result_type = prom_type or "unknown"
        
        if result_type == "vector":
            return self._compare_vector(prom_data.get("result", []), 
                                       chrono_data.get("result", []), 
                                       differences, path)
        elif result_type == "matrix":
            return self._compare_matrix(prom_data.get("result", []), 
                                       chrono_data.get("result", []), 
                                       differences, path)
        elif result_type == "scalar":
            return self._compare_scalar(prom_data.get("result"), 
                                       chrono_data.get("result"), 
                                       differences, path)
        elif result_type == "string":
            return self._compare_string(prom_data.get("result"), 
                                       chrono_data.get("result"), 
                                       differences, path)
        else:
            # 未知类型，直接对比JSON
            if json.dumps(prom_data, sort_keys=True) != json.dumps(chrono_data, sort_keys=True):
                differences.append({
                    "type": "data_mismatch",
                    "path": path,
                    "prometheus": prom_data,
                    "chronodb": chrono_data
                })
                return False
            return True
    
    def _compare_vector(self, prom_results: List[Dict], chrono_results: List[Dict],
                       differences: List[Dict], path: str) -> bool:
        """对比向量结果"""
        # 按标签排序
        prom_sorted = sorted(prom_results, key=lambda x: json.dumps(x.get("metric", {}), sort_keys=True))
        chrono_sorted = sorted(chrono_results, key=lambda x: json.dumps(x.get("metric", {}), sort_keys=True))
        
        if len(prom_sorted) != len(chrono_sorted):
            differences.append({
                "type": "series_count_mismatch",
                "path": path,
                "prometheus": len(prom_sorted),
                "chronodb": len(chrono_sorted)
            })
            return False
        
        match = True
        for i, (prom_item, chrono_item) in enumerate(zip(prom_sorted, chrono_sorted)):
            item_path = f"{path}[{i}]"
            
            # 对比标签
            prom_metric = prom_item.get("metric", {})
            chrono_metric = chrono_item.get("metric", {})
            
            if prom_metric != chrono_metric:
                differences.append({
                    "type": "metric_mismatch",
                    "path": item_path,
                    "prometheus": prom_metric,
                    "chronodb": chrono_metric
                })
                match = False
                continue
            
            # 对比值
            prom_value = prom_item.get("value", [])
            chrono_value = chrono_item.get("value", [])
            
            if not self._compare_sample(prom_value, chrono_value, differences, item_path):
                match = False
        
        return match
    
    def _compare_matrix(self, prom_results: List[Dict], chrono_results: List[Dict],
                       differences: List[Dict], path: str) -> bool:
        """对比矩阵结果"""
        # 按标签排序
        prom_sorted = sorted(prom_results, key=lambda x: json.dumps(x.get("metric", {}), sort_keys=True))
        chrono_sorted = sorted(chrono_results, key=lambda x: json.dumps(x.get("metric", {}), sort_keys=True))
        
        if len(prom_sorted) != len(chrono_sorted):
            differences.append({
                "type": "series_count_mismatch",
                "path": path,
                "prometheus": len(prom_sorted),
                "chronodb": len(chrono_sorted)
            })
            return False
        
        match = True
        for i, (prom_item, chrono_item) in enumerate(zip(prom_sorted, chrono_sorted)):
            item_path = f"{path}[{i}]"
            
            # 对比标签
            prom_metric = prom_item.get("metric", {})
            chrono_metric = chrono_item.get("metric", {})
            
            if prom_metric != chrono_metric:
                differences.append({
                    "type": "metric_mismatch",
                    "path": item_path,
                    "prometheus": prom_metric,
                    "chronodb": chrono_metric
                })
                match = False
                continue
            
            # 对比值列表
            prom_values = prom_item.get("values", [])
            chrono_values = chrono_item.get("values", [])
            
            if len(prom_values) != len(chrono_values):
                differences.append({
                    "type": "sample_count_mismatch",
                    "path": item_path,
                    "prometheus": len(prom_values),
                    "chronodb": len(chrono_values)
                })
                match = False
                continue
            
            for j, (prom_val, chrono_val) in enumerate(zip(prom_values, chrono_values)):
                if not self._compare_sample(prom_val, chrono_val, differences, f"{item_path}[{j}]"):
                    match = False
        
        return match
    
    def _compare_sample(self, prom_sample: List, chrono_sample: List,
                       differences: List[Dict], path: str) -> bool:
        """对比单个样本"""
        if len(prom_sample) < 2 or len(chrono_sample) < 2:
            differences.append({
                "type": "invalid_sample",
                "path": path,
                "prometheus": prom_sample,
                "chronodb": chrono_sample
            })
            return False
        
        # 对比时间戳
        prom_ts = float(prom_sample[0])
        chrono_ts = float(chrono_sample[0])
        
        ts_diff_ms = abs(prom_ts - chrono_ts) * 1000
        if ts_diff_ms > self.timestamp_tolerance_ms:
            differences.append({
                "type": "timestamp_mismatch",
                "path": path,
                "prometheus": prom_ts,
                "chronodb": chrono_ts,
                "diff_ms": ts_diff_ms
            })
            return False
        
        # 对比值
        try:
            prom_val = float(prom_sample[1])
            chrono_val = float(chrono_sample[1])
            
            if prom_val == 0:
                val_diff = abs(chrono_val)
            else:
                val_diff = abs(prom_val - chrono_val) / abs(prom_val)
            
            if val_diff > self.value_tolerance:
                differences.append({
                    "type": "value_mismatch",
                    "path": path,
                    "prometheus": prom_val,
                    "chronodb": chrono_val,
                    "relative_diff": val_diff
                })
                return False
        except (ValueError, TypeError) as e:
            # 非数值比较
            if str(prom_sample[1]) != str(chrono_sample[1]):
                differences.append({
                    "type": "value_mismatch",
                    "path": path,
                    "prometheus": prom_sample[1],
                    "chronodb": chrono_sample[1]
                })
                return False
        
        return True
    
    def _compare_scalar(self, prom_val: Any, chrono_val: Any,
                       differences: List[Dict], path: str) -> bool:
        """对比标量结果"""
        try:
            prom_float = float(prom_val)
            chrono_float = float(chrono_val)
            
            if prom_float == 0:
                diff = abs(chrono_float)
            else:
                diff = abs(prom_float - chrono_float) / abs(prom_float)
            
            if diff > self.value_tolerance:
                differences.append({
                    "type": "scalar_mismatch",
                    "path": path,
                    "prometheus": prom_val,
                    "chronodb": chrono_val,
                    "relative_diff": diff
                })
                return False
        except (ValueError, TypeError):
            if prom_val != chrono_val:
                differences.append({
                    "type": "scalar_mismatch",
                    "path": path,
                    "prometheus": prom_val,
                    "chronodb": chrono_val
                })
                return False
        
        return True
    
    def _compare_string(self, prom_val: str, chrono_val: str,
                       differences: List[Dict], path: str) -> bool:
        """对比字符串结果"""
        if prom_val != chrono_val:
            differences.append({
                "type": "string_mismatch",
                "path": path,
                "prometheus": prom_val,
                "chronodb": chrono_val
            })
            return False
        return True
    
    def get_report(self) -> ComparisonReport:
        """获取对比报告"""
        return self.report
    
    def print_summary(self):
        """打印对比摘要"""
        self.logger.section("Prometheus vs ChronoDB 对比结果")
        
        self.logger.info(f"总查询数: {self.report.total_queries}")
        self.logger.info(f"匹配数: {self.report.matched_queries}")
        self.logger.info(f"不匹配数: {self.report.mismatched_queries}")
        self.logger.info(f"匹配率: {self.report.match_rate:.2f}%")
        
        self.logger.info(f"\nPrometheus平均耗时: {self.report.avg_prometheus_duration_ms:.2f}ms")
        self.logger.info(f"ChronoDB平均耗时: {self.report.avg_chronodb_duration_ms:.2f}ms")
        
        if self.report.performance_ratio < 1.0:
            speedup = 1.0 / self.report.performance_ratio
            self.logger.info(f"ChronoDB比Prometheus快 {speedup:.2f}x")
        else:
            self.logger.info(f"ChronoDB比Prometheus慢 {self.report.performance_ratio:.2f}x")
        
        # 打印不匹配的查询
        if self.report.mismatched_queries > 0:
            self.logger.warning(f"\n不匹配的查询:")
            for result in self.report.results:
                if not result.match:
                    self.logger.warning(f"  - {result.query}")
                    for diff in result.differences[:3]:  # 只显示前3个差异
                        self.logger.warning(f"    {diff['type']}: {diff}")


class ComparisonTestRunner:
    """对比测试运行器"""
    
    def __init__(self, prometheus_url: str, chronodb_url: str):
        self.prometheus_url = prometheus_url
        self.chronodb_url = chronodb_url
        self.logger = get_logger()
        self.config = get_config()
        self.comparator = ResultComparator()
    
    def run_comparison(self, queries: List[str],
                      start_ms: Optional[int] = None,
                      end_ms: Optional[int] = None) -> ComparisonReport:
        """
        运行对比测试
        
        Args:
            queries: 查询列表
            start_ms: 范围查询起始时间
            end_ms: 范围查询结束时间
        
        Returns:
            对比报告
        """
        from query_tests.promql_tester import PromQLQueryClient
        
        self.logger.section("开始Prometheus vs ChronoDB对比测试")
        
        prom_client = PromQLQueryClient(self.prometheus_url)
        chrono_client = PromQLQueryClient(self.chronodb_url)
        
        self.comparator.report.start_time = datetime.now()
        
        for query in queries:
            self.logger.info(f"对比查询: {query}")
            
            # 执行查询
            if start_ms and end_ms:
                prom_result = prom_client.query_range(query, start_ms, end_ms)
                chrono_result = chrono_client.query_range(query, start_ms, end_ms)
            else:
                prom_result = prom_client.query(query)
                chrono_result = chrono_client.query(query)
            
            # 对比结果
            comparison = self.comparator.compare_query_results(
                query,
                prom_result.data if prom_result.status == "success" else {"status": "error", "error": prom_result.error},
                chrono_result.data if chrono_result.status == "success" else {"status": "error", "error": chrono_result.error},
                prom_result.duration_ms,
                chrono_result.duration_ms
            )
            
            if comparison.match:
                self.logger.test_pass(f"查询匹配: {query}")
            else:
                self.logger.test_fail(f"查询不匹配: {query}")
        
        self.comparator.report.end_time = datetime.now()
        
        self.comparator.print_summary()
        
        return self.comparator.get_report()


# 便捷函数
def create_result_comparator(value_tolerance: float = 0.01,
                            timestamp_tolerance_ms: int = 1000) -> ResultComparator:
    """创建结果对比器"""
    return ResultComparator(value_tolerance, timestamp_tolerance_ms)


def create_comparison_runner(prometheus_url: str, chronodb_url: str) -> ComparisonTestRunner:
    """创建对比测试运行器"""
    return ComparisonTestRunner(prometheus_url, chronodb_url)
