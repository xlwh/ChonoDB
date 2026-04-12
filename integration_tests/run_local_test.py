#!/usr/bin/env python3
"""
本地服务模式集成测试
用于测试本地运行的 Prometheus 和 ChronoDB 服务
"""

import argparse
import sys
import time
import subprocess
import signal
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

from core.logger import get_logger
from core.config import get_config
from data_generators.metric_generator import create_metric_generator, create_data_writer
from query_tests.promql_tester import create_promql_tester
from comparators.result_comparator import create_comparison_runner
from reports.report_generator import create_report_generator


class LocalServiceTestRunner:
    """本地服务测试运行器"""
    
    def __init__(self, prometheus_url: str = "http://localhost:9090",
                 chronodb_url: str = "http://localhost:9091"):
        self.logger = get_logger()
        self.config = get_config()
        self.prometheus_url = prometheus_url
        self.chronodb_url = chronodb_url
        self.test_suites = []
        self.comparison_report = None
        self.metadata = {
            "mode": "local",
            "prometheus_url": prometheus_url,
            "chronodb_url": chronodb_url
        }
        self._interrupted = False
        
        signal.signal(signal.SIGINT, self._signal_handler)
        signal.signal(signal.SIGTERM, self._signal_handler)
    
    def _signal_handler(self, signum, frame):
        """信号处理"""
        self.logger.warning("收到中断信号...")
        self._interrupted = True
        sys.exit(1)
    
    def check_services(self) -> bool:
        """检查服务是否可用"""
        import requests
        
        self.logger.section("检查本地服务")
        
        # 检查 Prometheus
        try:
            response = requests.get(f"{self.prometheus_url}/-/healthy", timeout=5)
            if response.status_code == 200:
                self.logger.info(f"✓ Prometheus 服务正常: {self.prometheus_url}")
                prometheus_ok = True
            else:
                self.logger.error(f"✗ Prometheus 服务异常: {response.status_code}")
                prometheus_ok = False
        except Exception as e:
            self.logger.error(f"✗ Prometheus 服务无法连接: {e}")
            prometheus_ok = False
        
        # 检查 ChronoDB
        try:
            response = requests.get(f"{self.chronodb_url}/-/healthy", timeout=5)
            if response.status_code == 200:
                self.logger.info(f"✓ ChronoDB 服务正常: {self.chronodb_url}")
                chronodb_ok = True
            else:
                self.logger.error(f"✗ ChronoDB 服务异常: {response.status_code}")
                chronodb_ok = False
        except Exception as e:
            self.logger.error(f"✗ ChronoDB 服务无法连接: {e}")
            chronodb_ok = False
        
        return prometheus_ok and chronodb_ok
    
    def generate_and_write_data(self, scale: str, target: str = "both") -> bool:
        """生成并写入测试数据"""
        self.logger.section(f"生成测试数据 (规模: {scale})")
        
        scale_config = self.config.get_scale_config(scale)
        self.logger.info(f"规模配置: {scale_config}")
        
        generator = create_metric_generator(seed=42)
        series_list = generator.generate_test_data_set(scale_config)
        
        if target in ("prometheus", "both"):
            self.logger.info("写入数据到 Prometheus...")
            writer = create_data_writer(self.prometheus_url)
            success, fail = writer.write_batch(series_list, batch_size=1000)
            self.logger.info(f"Prometheus 写入完成: 成功 {success} 批次, 失败 {fail} 批次")
        
        if target in ("chronodb", "both"):
            self.logger.info("写入数据到 ChronoDB...")
            writer = create_data_writer(self.chronodb_url)
            success, fail = writer.write_batch(series_list, batch_size=1000)
            self.logger.info(f"ChronoDB 写入完成: 成功 {success} 批次, 失败 {fail} 批次")
        
        self.logger.info("等待数据稳定...")
        time.sleep(2)
        
        return True
    
    def run_promql_tests(self, target: str = "chronodb"):
        """运行 PromQL 测试"""
        self.logger.section(f"运行 PromQL 测试 (目标: {target})")
        
        if target in ("prometheus", "both"):
            tester = create_promql_tester(self.prometheus_url)
            suite = tester.run_all_tests()
            self.test_suites.append(suite)
        
        if target in ("chronodb", "both"):
            tester = create_promql_tester(self.chronodb_url)
            suite = tester.run_all_tests()
            self.test_suites.append(suite)
    
    def run_comparison_tests(self):
        """运行对比测试"""
        self.logger.section("运行 Prometheus vs ChronoDB 对比测试")
        
        runner = create_comparison_runner(self.prometheus_url, self.chronodb_url)
        
        queries = [
            "up",
            "prometheus_http_requests_total",
            "sum(prometheus_http_requests_total)",
            "avg(prometheus_http_requests_total)",
            "rate(prometheus_http_requests_total[5m])",
            "sum by (job) (prometheus_http_requests_total)",
        ]
        
        self.comparison_report = runner.run_comparison(queries)
    
    def generate_reports(self):
        """生成测试报告"""
        self.logger.section("生成测试报告")
        
        generator = create_report_generator()
        reports = generator.generate_all_reports(
            test_suites=self.test_suites,
            comparison_report=self.comparison_report,
            metadata=self.metadata
        )
        
        return reports
    
    def run_full_test(self, args):
        """运行完整测试"""
        self.logger.section("ChronoDB 本地服务集成测试")
        self.logger.info(f"Prometheus: {self.prometheus_url}")
        self.logger.info(f"ChronoDB: {self.chronodb_url}")
        
        # 1. 检查服务
        if not self.check_services():
            self.logger.error("服务检查失败，请确保 Prometheus 和 ChronoDB 已启动")
            return False
        
        if self._interrupted:
            return False
        
        # 2. 生成并写入数据
        target = "both" if args.compare else "chronodb"
        if not self.generate_and_write_data(args.scale, target=target):
            return False
        
        if self._interrupted:
            return False
        
        # 3. 运行 PromQL 测试
        if args.test_promql:
            self.run_promql_tests(target="chronodb")
        
        if self._interrupted:
            return False
        
        # 4. 运行对比测试
        if args.compare:
            self.run_comparison_tests()
        
        if self._interrupted:
            return False
        
        # 5. 生成报告
        if args.generate_report:
            reports = self.generate_reports()
            self.logger.info(f"报告已生成: {reports}")
        
        return True


def main():
    """主函数"""
    parser = argparse.ArgumentParser(
        description="ChronoDB 本地服务集成测试",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
示例:
  # 测试本地 ChronoDB (默认 http://localhost:9091)
  python run_local_test.py --scale small
  
  # 测试对比模式 (需要 Prometheus 在 9090, ChronoDB 在 9091)
  python run_local_test.py --compare --scale medium
  
  # 指定自定义地址
  python run_local_test.py --prometheus-url http://localhost:9090 --chronodb-url http://localhost:9091 --compare
        """
    )
    
    parser.add_argument(
        "--prometheus-url",
        default="http://localhost:9090",
        help="Prometheus 服务地址 (默认: http://localhost:9090)"
    )
    
    parser.add_argument(
        "--chronodb-url",
        default="http://localhost:9091",
        help="ChronoDB 服务地址 (默认: http://localhost:9091)"
    )
    
    parser.add_argument(
        "--scale",
        choices=["small", "medium", "large"],
        default="small",
        help="数据规模 (默认: small)"
    )
    
    parser.add_argument(
        "--compare",
        action="store_true",
        help="启用 Prometheus vs ChronoDB 对比测试"
    )
    
    parser.add_argument(
        "--test-promql",
        action="store_true",
        default=True,
        help="运行 PromQL 测试 (默认: True)"
    )
    
    parser.add_argument(
        "--generate-report",
        action="store_true",
        default=True,
        help="生成测试报告 (默认: True)"
    )
    
    parser.add_argument(
        "--log-level",
        choices=["DEBUG", "INFO", "WARNING", "ERROR"],
        default="INFO",
        help="日志级别 (默认: INFO)"
    )
    
    args = parser.parse_args()
    
    # 设置日志级别
    import logging
    log_level = getattr(logging, args.log_level)
    logger = get_logger(log_level=log_level)
    
    # 打印测试配置
    logger.info(f"数据规模: {args.scale}")
    logger.info(f"对比测试: {'启用' if args.compare else '禁用'}")
    logger.info(f"PromQL测试: {'启用' if args.test_promql else '禁用'}")
    logger.info(f"生成报告: {'启用' if args.generate_report else '禁用'}")
    
    # 运行测试
    runner = LocalServiceTestRunner(
        prometheus_url=args.prometheus_url,
        chronodb_url=args.chronodb_url
    )
    success = runner.run_full_test(args)
    
    # 输出结果
    logger.section("测试完成")
    if success:
        logger.info("✓ 所有测试通过")
        return 0
    else:
        logger.error("✗ 测试失败")
        return 1


if __name__ == "__main__":
    sys.exit(main())
