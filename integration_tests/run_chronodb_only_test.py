#!/usr/bin/env python3
"""
ChronoDB 独立性能测试
仅测试 ChronoDB，不依赖 Prometheus
"""

import argparse
import sys
import time
import signal
import statistics
from pathlib import Path
from datetime import datetime

sys.path.insert(0, str(Path(__file__).parent))

from core.logger import get_logger
from core.config import get_config
from data_generators.metric_generator import create_metric_generator, create_data_writer
from query_tests.promql_tester import create_promql_tester
from query_tests.downsampling_tester import create_downsampling_tester
from query_tests.preaggregation_tester import create_preaggregation_tester
from reports.report_generator import create_report_generator


class ChronoDBOnlyTestRunner:
    """ChronoDB 独立测试运行器"""
    
    def __init__(self, chronodb_url: str = "http://localhost:9091"):
        self.logger = get_logger()
        self.config = get_config()
        self.chronodb_url = chronodb_url
        self.test_suites = []
        self.metadata = {
            "mode": "local-chronodb-only",
            "chronodb_url": chronodb_url,
            "test_time": datetime.now().isoformat()
        }
        self.performance_results = []
        self._interrupted = False
        
        signal.signal(signal.SIGINT, self._signal_handler)
        signal.signal(signal.SIGTERM, self._signal_handler)
    
    def _signal_handler(self, signum, frame):
        """信号处理"""
        self.logger.warning("收到中断信号...")
        self._interrupted = True
        sys.exit(1)
    
    def check_service(self) -> bool:
        """检查 ChronoDB 服务是否可用"""
        import requests
        
        self.logger.section("检查 ChronoDB 服务")
        
        try:
            response = requests.get(f"{self.chronodb_url}/-/healthy", timeout=5)
            if response.status_code == 200:
                self.logger.info(f"✓ ChronoDB 服务正常: {self.chronodb_url}")
                return True
            else:
                self.logger.error(f"✗ ChronoDB 服务异常: {response.status_code}")
                return False
        except Exception as e:
            self.logger.error(f"✗ ChronoDB 服务无法连接: {e}")
            return False
    
    def generate_and_write_data(self, scale: str) -> bool:
        """生成并写入测试数据"""
        self.logger.section(f"生成测试数据 (规模: {scale})")
        
        scale_config = self.config.get_scale_config(scale)
        self.logger.info(f"规模配置: {scale_config}")
        
        # 记录生成时间
        start_time = time.time()
        generator = create_metric_generator(seed=42)
        series_list = generator.generate_test_data_set(scale_config)
        gen_time = time.time() - start_time
        
        total_samples = sum(len(s.samples) for s in series_list)
        self.logger.info(f"数据生成完成: {len(series_list)} 系列, {total_samples} 样本, 耗时 {gen_time:.2f}s")
        
        # 写入数据
        self.logger.info("写入数据到 ChronoDB...")
        start_time = time.time()
        writer = create_data_writer(self.chronodb_url)
        success, fail = writer.write_batch(series_list, batch_size=1000)
        write_time = time.time() - start_time
        
        self.logger.info(f"ChronoDB 写入完成: 成功 {success} 批次, 失败 {fail} 批次, 耗时 {write_time:.2f}s")
        
        self.metadata['data_scale'] = scale
        self.metadata['series_count'] = len(series_list)
        self.metadata['sample_count'] = total_samples
        self.metadata['write_time'] = write_time
        
        self.logger.info("等待数据稳定...")
        time.sleep(2)
        
        return True
    
    def run_performance_tests(self):
        """运行性能测试"""
        self.logger.section("运行性能测试")
        
        import requests
        
        # 测试查询
        test_queries = [
            ("simple_select", "cpu_usage_percent"),
            ("sum_aggregation", "sum(cpu_usage_percent)"),
            ("avg_aggregation", "avg(memory_usage_percent)"),
            ("rate_calculation", "rate(http_requests_total[5m])"),
            ("sum_by_label", "sum by (instance) (cpu_usage_percent)"),
            ("complex_query", "sum(rate(http_requests_total[5m])) by (job)"),
        ]
        
        results = []
        
        for name, query in test_queries:
            if self._interrupted:
                break
                
            self.logger.info(f"测试查询: {name} - {query}")
            
            # 预热
            for _ in range(3):
                try:
                    requests.post(
                        f"{self.chronodb_url}/api/v1/query",
                        data={"query": query},
                        timeout=10
                    )
                except:
                    pass
            
            # 正式测试 - 运行10次取平均
            latencies = []
            for i in range(10):
                try:
                    start = time.perf_counter()
                    response = requests.post(
                        f"{self.chronodb_url}/api/v1/query",
                        data={"query": query},
                        timeout=30
                    )
                    elapsed = (time.perf_counter() - start) * 1000  # ms
                    
                    if response.status_code == 200:
                        latencies.append(elapsed)
                except Exception as e:
                    self.logger.warning(f"查询失败: {e}")
            
            if latencies:
                avg_latency = statistics.mean(latencies)
                min_latency = min(latencies)
                max_latency = max(latencies)
                p95_latency = sorted(latencies)[int(len(latencies) * 0.95)] if len(latencies) > 1 else avg_latency
                
                results.append({
                    'name': name,
                    'query': query,
                    'avg_ms': round(avg_latency, 2),
                    'min_ms': round(min_latency, 2),
                    'max_ms': round(max_latency, 2),
                    'p95_ms': round(p95_latency, 2),
                    'samples': len(latencies)
                })
                
                self.logger.info(f"  平均延迟: {avg_latency:.2f}ms, P95: {p95_latency:.2f}ms")
        
        self.performance_results = results
        return results
    
    def run_promql_tests(self):
        """运行 PromQL 兼容性测试"""
        self.logger.section("运行 PromQL 兼容性测试")
        
        tester = create_promql_tester(self.chronodb_url)
        suite = tester.run_all_tests()
        self.test_suites.append(suite)
        
        return suite
    
    def run_downsampling_tests(self):
        """运行降采样功能测试"""
        self.logger.section("运行降采样功能测试")
        
        tester = create_downsampling_tester(self.chronodb_url)
        suite = tester.run_all_tests()
        self.test_suites.append(suite)
        
        return suite
    
    def run_preaggregation_tests(self):
        """运行预聚合功能测试"""
        self.logger.section("运行预聚合功能测试")
        
        tester = create_preaggregation_tester(self.chronodb_url)
        suite = tester.run_all_tests()
        self.test_suites.append(suite)
        
        return suite
    
    def generate_report(self):
        """生成测试报告"""
        self.logger.section("生成测试报告")
        
        report_time = datetime.now().strftime("%Y%m%d_%H%M%S")
        report_dir = Path("test_reports")
        report_dir.mkdir(exist_ok=True)
        
        # 生成性能报告
        report_file = report_dir / f"performance_report_{self.metadata.get('data_scale', 'unknown')}_{report_time}.md"
        
        with open(report_file, 'w') as f:
            f.write("# ChronoDB 性能测试报告\n\n")
            f.write(f"**测试时间**: {self.metadata.get('test_time', 'N/A')}\n")
            f.write(f"**ChronoDB地址**: {self.chronodb_url}\n")
            f.write(f"**数据规模**: {self.metadata.get('data_scale', 'N/A')}\n")
            f.write(f"**时间序列数**: {self.metadata.get('series_count', 'N/A')}\n")
            f.write(f"**样本总数**: {self.metadata.get('sample_count', 'N/A')}\n")
            f.write(f"**写入耗时**: {self.metadata.get('write_time', 'N/A'):.2f}s\n\n")
            
            f.write("## 查询性能测试结果\n\n")
            f.write("| 测试项 | 查询 | 平均延迟(ms) | P95延迟(ms) | 最小(ms) | 最大(ms) |\n")
            f.write("|--------|------|-------------|-------------|----------|----------|\n")
            
            for r in self.performance_results:
                f.write(f"| {r['name']} | `{r['query']}` | {r['avg_ms']} | {r['p95_ms']} | {r['min_ms']} | {r['max_ms']} |\n")
            
            f.write("\n## 性能指标汇总\n\n")
            if self.performance_results:
                avg_latencies = [r['avg_ms'] for r in self.performance_results]
                f.write(f"- **平均查询延迟**: {statistics.mean(avg_latencies):.2f}ms\n")
                f.write(f"- **最快查询**: {min(avg_latencies):.2f}ms\n")
                f.write(f"- **最慢查询**: {max(avg_latencies):.2f}ms\n")
            
            f.write("\n## 测试配置\n\n")
            f.write("- 数据规模: " + self.metadata.get('data_scale', 'N/A') + "\n")
            f.write("- 时间序列数: " + str(self.metadata.get('series_count', 'N/A')) + "\n")
            f.write("- 样本总数: " + str(self.metadata.get('sample_count', 'N/A')) + "\n")
        
        self.logger.info(f"性能报告已生成: {report_file}")
        return report_file
    
    def run_full_test(self, args):
        """运行完整测试"""
        self.logger.section("ChronoDB 性能测试")
        self.logger.info(f"ChronoDB: {self.chronodb_url}")
        
        # 1. 检查服务
        if not self.check_service():
            self.logger.error("服务检查失败，请确保 ChronoDB 已启动")
            return False
        
        if self._interrupted:
            return False
        
        # 2. 生成并写入数据
        if not self.generate_and_write_data(args.scale):
            return False
        
        if self._interrupted:
            return False
        
        # 3. 运行性能测试
        self.run_performance_tests()
        
        if self._interrupted:
            return False
        
        # 4. 运行 PromQL 测试
        if args.test_promql:
            self.run_promql_tests()
        
        if self._interrupted:
            return False
        
        # 5. 运行降采样测试
        if args.test_downsampling:
            self.run_downsampling_tests()
        
        if self._interrupted:
            return False
        
        # 6. 运行预聚合测试
        if args.test_preaggregation:
            self.run_preaggregation_tests()
        
        if self._interrupted:
            return False
        
        # 7. 生成报告
        if args.generate_report:
            report = self.generate_report()
            self.logger.info(f"报告已生成: {report}")
        
        return True


def main():
    """主函数"""
    parser = argparse.ArgumentParser(
        description="ChronoDB 性能测试",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
示例:
  # 小规模测试
  python run_chronodb_only_test.py --scale small
  
  # 中等规模测试
  python run_chronodb_only_test.py --scale medium
  
  # 指定自定义地址
  python run_chronodb_only_test.py --chronodb-url http://localhost:9091 --scale medium
        """
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
        "--test-promql",
        action="store_true",
        default=True,
        help="运行 PromQL 测试 (默认: True)"
    )

    parser.add_argument(
        "--test-downsampling",
        action="store_true",
        default=True,
        help="运行降采样功能测试 (默认: True)"
    )

    parser.add_argument(
        "--test-preaggregation",
        action="store_true",
        default=True,
        help="运行预聚合功能测试 (默认: True)"
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
    
    # 运行测试
    runner = ChronoDBOnlyTestRunner(chronodb_url=args.chronodb_url)
    success = runner.run_full_test(args)
    
    sys.exit(0 if success else 1)


if __name__ == "__main__":
    main()
