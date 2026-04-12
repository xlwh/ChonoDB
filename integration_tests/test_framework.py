#!/usr/bin/env python3
"""
集成测试框架验证脚本
用于验证测试框架的各个组件是否正常工作
"""

import sys
from pathlib import Path
sys.path.insert(0, str(Path(__file__).parent))

from core.logger import get_logger
from core.config import get_config
from data_generators.metric_generator import create_metric_generator
from query_tests.promql_tester import PromQLQueryClient
from comparators.result_comparator import create_result_comparator
from reports.report_generator import create_report_generator


def test_logger():
    """测试日志模块"""
    print("=" * 60)
    print("测试日志模块")
    print("=" * 60)
    
    logger = get_logger()
    logger.info("这是一条信息日志")
    logger.warning("这是一条警告日志")
    logger.test_pass("测试通过示例")
    logger.test_fail("测试失败示例")
    print("✓ 日志模块测试通过\n")


def test_config():
    """测试配置模块"""
    print("=" * 60)
    print("测试配置模块")
    print("=" * 60)
    
    config = get_config()
    
    print(f"Prometheus 镜像: {config.container_config.prometheus_image}")
    print(f"ChronoDB 镜像: {config.container_config.chronodb_image}")
    print(f"网络名称: {config.container_config.network_name}")
    
    small_config = config.get_scale_config("small")
    print(f"Small 规模配置: {small_config}")
    
    print("✓ 配置模块测试通过\n")


def test_metric_generator():
    """测试指标生成器"""
    print("=" * 60)
    print("测试指标生成器")
    print("=" * 60)
    
    generator = create_metric_generator(seed=42)
    
    # 生成 Gauge 序列
    gauge = generator.generate_gauge_series(
        name="test_gauge",
        num_samples=10,
        start_time_ms=1609459200000,
        interval_ms=1000,
        min_value=0,
        max_value=100
    )
    print(f"生成 Gauge 序列: {gauge.name}, 样本数: {len(gauge.samples)}")
    
    # 生成 Counter 序列
    counter = generator.generate_counter_series(
        name="test_counter",
        num_samples=10,
        start_time_ms=1609459200000,
        interval_ms=1000
    )
    print(f"生成 Counter 序列: {counter.name}, 样本数: {len(counter.samples)}")
    
    # 生成测试数据集
    scale_config = {
        "metrics": 2,
        "series_per_metric": 2,
        "samples_per_series": 10,
        "time_range_hours": 1
    }
    series_list = generator.generate_test_data_set(scale_config)
    print(f"生成测试数据集: {len(series_list)} 个序列")
    
    print("✓ 指标生成器测试通过\n")


def test_report_generator():
    """测试报告生成器"""
    print("=" * 60)
    print("测试报告生成器")
    print("=" * 60)
    
    from core.base_test import TestSuite, TestResult
    from datetime import datetime
    
    # 创建模拟测试套件
    suite = TestSuite(name="测试套件示例")
    suite.start_time = datetime.now()
    
    # 添加测试结果
    suite.add_result(TestResult(
        name="测试1",
        passed=True,
        duration_ms=100.5,
        message="测试通过"
    ))
    suite.add_result(TestResult(
        name="测试2",
        passed=True,
        duration_ms=200.3,
        message="测试通过"
    ))
    suite.add_result(TestResult(
        name="测试3",
        passed=False,
        duration_ms=50.0,
        message="测试失败: 期望值不匹配"
    ))
    
    suite.end_time = datetime.now()
    
    # 生成报告
    generator = create_report_generator(output_dir="./test_reports")
    reports = generator.generate_all_reports(
        test_suites=[suite],
        metadata={"test": "framework_validation"}
    )
    
    print(f"生成报告:")
    for fmt, path in reports.items():
        print(f"  - {fmt}: {path}")
    
    print("✓ 报告生成器测试通过\n")


def test_result_comparator():
    """测试结果对比器"""
    print("=" * 60)
    print("测试结果对比器")
    print("=" * 60)
    
    comparator = create_result_comparator(
        value_tolerance=0.01,
        timestamp_tolerance_ms=1000
    )
    
    # 测试相同结果
    prom_result = {
        "status": "success",
        "data": {
            "resultType": "vector",
            "result": [
                {
                    "metric": {"__name__": "test", "job": "test"},
                    "value": [1609459200, "100"]
                }
            ]
        }
    }
    
    chrono_result = {
        "status": "success",
        "data": {
            "resultType": "vector",
            "result": [
                {
                    "metric": {"__name__": "test", "job": "test"},
                    "value": [1609459200, "100"]
                }
            ]
        }
    }
    
    result = comparator.compare_query_results(
        query="test_query",
        prometheus_result=prom_result,
        chronodb_result=chrono_result,
        prometheus_duration_ms=50.0,
        chronodb_duration_ms=45.0
    )
    
    print(f"查询: {result.query}")
    print(f"匹配: {result.match}")
    print(f"Prometheus 耗时: {result.prometheus_duration_ms}ms")
    print(f"ChronoDB 耗时: {result.chronodb_duration_ms}ms")
    
    # 测试不同结果
    chrono_result2 = {
        "status": "success",
        "data": {
            "resultType": "vector",
            "result": [
                {
                    "metric": {"__name__": "test", "job": "test"},
                    "value": [1609459200, "101"]  # 不同值
                }
            ]
        }
    }
    
    result2 = comparator.compare_query_results(
        query="test_query2",
        prometheus_result=prom_result,
        chronodb_result=chrono_result2,
        prometheus_duration_ms=50.0,
        chronodb_duration_ms=45.0
    )
    
    print(f"\n查询: {result2.query}")
    print(f"匹配: {result2.match}")
    print(f"差异: {result2.differences}")
    
    print("✓ 结果对比器测试通过\n")


def main():
    """主函数"""
    print("\n" + "=" * 60)
    print("ChronoDB 集成测试框架验证")
    print("=" * 60 + "\n")
    
    try:
        test_logger()
        test_config()
        test_metric_generator()
        test_result_comparator()
        test_report_generator()
        
        print("=" * 60)
        print("所有测试通过！框架工作正常。")
        print("=" * 60)
        return 0
    
    except Exception as e:
        print(f"\n✗ 测试失败: {e}")
        import traceback
        traceback.print_exc()
        return 1


if __name__ == "__main__":
    sys.exit(main())
