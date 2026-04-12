#!/usr/bin/env python3
"""
ChronoDB 集成测试主运行脚本
一键运行所有集成测试

用法:
    python run_tests.py --mode standalone --scale small
    python run_tests.py --mode distributed --scale medium --enable-fault-injection
    python run_tests.py --compare --scale large
"""

import argparse
import sys
import time
import signal
from typing import List, Dict, Any, Optional
from datetime import datetime

# 添加项目路径
from pathlib import Path
sys.path.insert(0, str(Path(__file__).parent))

from core.logger import get_logger
from core.config import get_config
from core.base_test import TestSuite
from containers.docker_manager import create_container_manager
from data_generators.metric_generator import create_metric_generator, create_data_writer
from query_tests.promql_tester import create_promql_tester
from fault_injection.fault_injector import create_fault_injector, create_chaos_monkey
from comparators.result_comparator import create_comparison_runner
from reports.report_generator import create_report_generator


class IntegrationTestRunner:
    """集成测试运行器"""
    
    def __init__(self):
        self.logger = get_logger()
        self.config = get_config()
        self.container_manager = None
        self.test_suites: List[TestSuite] = []
        self.comparison_report = None
        self.metadata: Dict[str, Any] = {}
        self._interrupted = False
        
        # 注册信号处理
        signal.signal(signal.SIGINT, self._signal_handler)
        signal.signal(signal.SIGTERM, self._signal_handler)
    
    def _signal_handler(self, signum, frame):
        """信号处理"""
        self.logger.warning("收到中断信号，正在清理...")
        self._interrupted = True
        self.cleanup()
        sys.exit(1)
    
    def setup_environment(self, mode: str = "standalone") -> bool:
        """设置测试环境"""
        self.logger.section("设置测试环境")
        
        # 创建容器管理器
        self.container_manager = create_container_manager()
        
        # 创建Docker网络
        if not self.container_manager.create_network():
            self.logger.error("创建Docker网络失败")
            return False
        
        self.metadata['mode'] = mode
        self.metadata['environment'] = f"docker-{mode}"
        
        return True
    
    def start_prometheus(self, port: int = 9090) -> bool:
        """启动Prometheus容器"""
        self.logger.section("启动Prometheus")
        
        info = self.container_manager.start_prometheus(
            name="prometheus-test",
            port=port
        )
        
        if info:
            self.logger.info(f"Prometheus已启动: {info.url}")
            self.metadata['prometheus_url'] = info.url
            return True
        else:
            self.logger.error("Prometheus启动失败")
            return False
    
    def start_chronodb(self, mode: str = "standalone", port: int = 9091) -> bool:
        """启动ChronoDB容器"""
        self.logger.section("启动ChronoDB")
        
        info = self.container_manager.start_chronodb(
            name="chronodb-test",
            port=port,
            mode=mode
        )
        
        if info:
            self.logger.info(f"ChronoDB已启动: {info.url}")
            self.metadata['chronodb_url'] = info.url
            return True
        else:
            self.logger.error("ChronoDB启动失败")
            return False
    
    def generate_and_write_data(self, scale: str, target: str = "both") -> bool:
        """
        生成并写入测试数据
        
        Args:
            scale: 数据规模 (small, medium, large)
            target: 写入目标 (prometheus, chronodb, both)
        """
        self.logger.section(f"生成测试数据 (规模: {scale})")
        
        # 获取规模配置
        scale_config = self.config.get_scale_config(scale)
        self.logger.info(f"规模配置: {scale_config}")
        
        # 生成数据
        generator = create_metric_generator(seed=42)
        series_list = generator.generate_test_data_set(scale_config)
        
        # 写入数据
        if target in ("prometheus", "both"):
            prom_url = self.metadata.get('prometheus_url')
            if prom_url:
                self.logger.info("写入数据到Prometheus...")
                writer = create_data_writer(prom_url)
                success, fail = writer.write_batch(series_list, batch_size=1000)
                self.logger.info(f"Prometheus写入完成: 成功 {success} 批次, 失败 {fail} 批次")
        
        if target in ("chronodb", "both"):
            chrono_url = self.metadata.get('chronodb_url')
            if chrono_url:
                self.logger.info("写入数据到ChronoDB...")
                writer = create_data_writer(chrono_url)
                success, fail = writer.write_batch(series_list, batch_size=1000)
                self.logger.info(f"ChronoDB写入完成: 成功 {success} 批次, 失败 {fail} 批次")
        
        # 等待数据写入完成
        self.logger.info("等待数据稳定...")
        time.sleep(2)
        
        return True
    
    def run_promql_tests(self, target: str = "chronodb") -> TestSuite:
        """
        运行PromQL测试
        
        Args:
            target: 测试目标 (prometheus, chronodb, both)
        """
        self.logger.section(f"运行PromQL测试 (目标: {target})")
        
        suite = None
        
        if target in ("prometheus", "both"):
            prom_url = self.metadata.get('prometheus_url')
            if prom_url:
                tester = create_promql_tester(prom_url)
                suite = tester.run_all_tests()
                self.test_suites.append(suite)
        
        if target in ("chronodb", "both"):
            chrono_url = self.metadata.get('chronodb_url')
            if chrono_url:
                tester = create_promql_tester(chrono_url)
                suite = tester.run_all_tests()
                self.test_suites.append(suite)
        
        return suite
    
    def run_comparison_tests(self) -> bool:
        """运行对比测试"""
        self.logger.section("运行Prometheus vs ChronoDB对比测试")
        
        prom_url = self.metadata.get('prometheus_url')
        chrono_url = self.metadata.get('chronodb_url')
        
        if not prom_url or not chrono_url:
            self.logger.error("缺少Prometheus或ChronoDB的URL")
            return False
        
        # 创建对比运行器
        runner = create_comparison_runner(prom_url, chrono_url)
        
        # 定义对比查询
        queries = [
            "cpu_usage",
            "memory_usage",
            "sum(cpu_usage)",
            "avg(memory_usage)",
            "cpu_usage{job=\"webserver\"}",
            "rate(cpu_usage[5m])",
            "sum by (job) (cpu_usage)",
            "cpu_usage + memory_usage",
            "cpu_usage > 50",
        ]
        
        # 运行对比
        self.comparison_report = runner.run_comparison(queries)
        
        return True
    
    def run_fault_injection_tests(self, duration_seconds: float = 300) -> bool:
        """运行故障注入测试"""
        self.logger.section("运行故障注入测试")
        
        if not self.container_manager:
            self.logger.error("容器管理器未初始化")
            return False
        
        # 获取容器列表
        container_names = list(self.container_manager.containers.keys())
        if not container_names:
            self.logger.error("没有可用的容器")
            return False
        
        # 创建故障注入器
        fault_injector = create_fault_injector(self.container_manager)
        
        # 创建混沌猴子
        chaos = create_chaos_monkey(self.container_manager, fault_injector)
        
        # 定义验证函数
        def verify():
            chrono_url = self.metadata.get('chronodb_url')
            if not chrono_url:
                return False
            
            import requests
            try:
                response = requests.get(f"{chrono_url}/-/healthy", timeout=5)
                return response.status_code == 200
            except:
                return False
        
        # 运行混沌测试
        result = chaos.run_chaos_test(
            container_names=container_names,
            test_duration_seconds=duration_seconds,
            fault_interval_seconds=30.0,
            verify_func=verify
        )
        
        self.logger.info(f"故障注入测试完成: {result}")
        
        return True
    
    def generate_reports(self) -> Dict[str, str]:
        """生成测试报告"""
        self.logger.section("生成测试报告")
        
        generator = create_report_generator()
        reports = generator.generate_all_reports(
            test_suites=self.test_suites,
            comparison_report=self.comparison_report,
            metadata=self.metadata
        )
        
        return reports
    
    def cleanup(self):
        """清理测试环境"""
        self.logger.section("清理测试环境")
        
        if self.container_manager:
            self.container_manager.cleanup()
            self.logger.info("容器已清理")
    
    def run_full_test(self, args) -> bool:
        """
        运行完整测试流程
        
        Args:
            args: 命令行参数
        """
        try:
            # 1. 设置环境
            if not self.setup_environment(args.mode):
                return False
            
            # 2. 启动服务
            if args.compare:
                # 对比模式：启动Prometheus和ChronoDB
                if not self.start_prometheus(port=9090):
                    return False
                if not self.start_chronodb(mode=args.mode, port=9091):
                    return False
            else:
                # 单服务模式：只启动ChronoDB
                if not self.start_chronodb(mode=args.mode, port=9090):
                    return False
            
            if self._interrupted:
                return False
            
            # 3. 生成并写入数据
            target = "both" if args.compare else "chronodb"
            if not self.generate_and_write_data(args.scale, target=target):
                return False
            
            if self._interrupted:
                return False
            
            # 4. 运行PromQL测试
            if args.test_promql:
                self.run_promql_tests(target="chronodb")
            
            if self._interrupted:
                return False
            
            # 5. 运行对比测试
            if args.compare:
                self.run_comparison_tests()
            
            if self._interrupted:
                return False
            
            # 6. 运行故障注入测试
            if args.enable_fault_injection:
                self.run_fault_injection_tests(duration_seconds=args.fault_duration)
            
            if self._interrupted:
                return False
            
            # 7. 生成报告
            if args.generate_report:
                reports = self.generate_reports()
                self.logger.info(f"报告已生成: {reports}")
            
            return True
        
        except Exception as e:
            self.logger.error(f"测试执行异常: {e}", exc_info=True)
            return False
        
        finally:
            # 清理
            if not args.keep_containers:
                self.cleanup()


def main():
    """主函数"""
    parser = argparse.ArgumentParser(
        description="ChronoDB 集成测试框架",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
示例:
  # 运行单机模式小规模测试
  python run_tests.py --mode standalone --scale small
  
  # 运行对比测试
  python run_tests.py --compare --scale medium
  
  # 运行带故障注入的测试
  python run_tests.py --mode distributed --scale medium --enable-fault-injection
  
  # 运行完整测试并保留容器
  python run_tests.py --compare --scale large --keep-containers
        """
    )
    
    # 测试模式
    parser.add_argument(
        "--mode",
        choices=["standalone", "distributed"],
        default="standalone",
        help="测试模式 (默认: standalone)"
    )
    
    # 数据规模
    parser.add_argument(
        "--scale",
        choices=["small", "medium", "large"],
        default="small",
        help="数据规模 (默认: small)"
    )
    
    # 对比测试
    parser.add_argument(
        "--compare",
        action="store_true",
        help="启用Prometheus vs ChronoDB对比测试"
    )
    
    # PromQL测试
    parser.add_argument(
        "--test-promql",
        action="store_true",
        default=True,
        help="运行PromQL测试 (默认: True)"
    )
    
    # 故障注入
    parser.add_argument(
        "--enable-fault-injection",
        action="store_true",
        help="启用故障注入测试"
    )
    
    parser.add_argument(
        "--fault-duration",
        type=float,
        default=300,
        help="故障注入测试持续时间(秒) (默认: 300)"
    )
    
    # 报告生成
    parser.add_argument(
        "--generate-report",
        action="store_true",
        default=True,
        help="生成测试报告 (默认: True)"
    )
    
    # 容器管理
    parser.add_argument(
        "--keep-containers",
        action="store_true",
        help="测试完成后保留容器"
    )
    
    # 日志级别
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
    logger.section("ChronoDB 集成测试")
    logger.info(f"测试模式: {args.mode}")
    logger.info(f"数据规模: {args.scale}")
    logger.info(f"对比测试: {'启用' if args.compare else '禁用'}")
    logger.info(f"PromQL测试: {'启用' if args.test_promql else '禁用'}")
    logger.info(f"故障注入: {'启用' if args.enable_fault_injection else '禁用'}")
    logger.info(f"生成报告: {'启用' if args.generate_report else '禁用'}")
    
    # 运行测试
    runner = IntegrationTestRunner()
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
