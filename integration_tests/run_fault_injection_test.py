#!/usr/bin/env python3
"""
故障注入测试脚本
测试 ChronoDB 在故障场景下的恢复能力
"""

import sys
import time
import signal
import subprocess
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

from core.logger import get_logger
from core.base_test import BaseTest, TestSuite, TestResult
from data_generators.metric_generator import create_metric_generator, create_data_writer
from query_tests.promql_tester import PromQLQueryClient


class FaultInjectionTest(BaseTest):
    """故障注入测试"""
    
    def __init__(self, chronodb_url: str = "http://localhost:9091"):
        super().__init__("故障注入测试")
        self.chronodb_url = chronodb_url
        self.client = PromQLQueryClient(chronodb_url)
        self.chronodb_pid = None
        self.test_data_written = False
    
    def _do_setup(self):
        """测试准备"""
        # 查找 ChronoDB 进程
        try:
            result = subprocess.run(
                ["pgrep", "-f", "chronodb-server"],
                capture_output=True,
                text=True
            )
            if result.returncode == 0:
                self.chronodb_pid = int(result.stdout.strip().split('\n')[0])
                self.logger.info(f"找到 ChronoDB 进程: PID {self.chronodb_pid}")
        except Exception as e:
            self.logger.warning(f"无法获取 ChronoDB PID: {e}")
        
        # 写入测试数据
        if not self.test_data_written:
            self._write_test_data()
    
    def _write_test_data(self):
        """写入测试数据"""
        self.logger.info("写入测试数据...")
        generator = create_metric_generator(seed=42)
        scale_config = {
            "metrics": 10,
            "series_per_metric": 10,
            "samples_per_series": 100,
            "time_range_hours": 1
        }
        series_list = generator.generate_test_data_set(scale_config)
        writer = create_data_writer(self.chronodb_url)
        success, fail = writer.write_batch(series_list, batch_size=1000)
        self.logger.info(f"数据写入完成: 成功 {success} 批次, 失败 {fail} 批次")
        self.test_data_written = True
        time.sleep(2)
    
    def _do_teardown(self):
        """测试清理"""
        pass
    
    def test_service_availability(self) -> bool:
        """测试服务可用性"""
        import requests
        try:
            response = requests.get(f"{self.chronodb_url}/-/healthy", timeout=5)
            return response.status_code == 200
        except:
            return False
    
    def test_query_functionality(self) -> bool:
        """测试查询功能"""
        try:
            result = self.client.query("up")
            return result.status == "success"
        except:
            return False
    
    def test_container_restart(self):
        """测试容器重启恢复"""
        self.logger.subsection("测试容器重启恢复")
        
        # 1. 验证服务正常
        def test_before_restart():
            if not self.test_service_availability():
                return False, "重启前服务不可用", {}
            if not self.test_query_functionality():
                return False, "重启前查询功能异常", {}
            return True, "重启前服务正常", {}
        
        result = self.run_test(test_before_restart, "重启前服务检查")
        
        # 2. 停止服务
        def stop_service():
            if self.chronodb_pid:
                try:
                    subprocess.run(["kill", str(self.chronodb_pid)], check=True)
                    return True, f"服务已停止 (PID: {self.chronodb_pid})", {}
                except Exception as e:
                    return False, f"停止服务失败: {e}", {}
            return False, "未找到服务 PID", {}
        
        result = self.run_test(stop_service, "停止 ChronoDB 服务")
        
        # 3. 等待服务停止
        time.sleep(2)
        
        # 4. 重新启动服务
        def start_service():
            try:
                subprocess.Popen(
                    ["./target/release/chronodb-server", "--config", "config/test.yaml"],
                    stdout=subprocess.DEVNULL,
                    stderr=subprocess.DEVNULL,
                    cwd="/Users/zhb/workspace/chonodb"
                )
                return True, "服务启动命令已执行", {}
            except Exception as e:
                return False, f"启动服务失败: {e}", {}
        
        result = self.run_test(start_service, "重新启动 ChronoDB 服务")
        
        # 5. 等待服务恢复
        self.logger.info("等待服务恢复...")
        max_wait = 30
        recovered = False
        for i in range(max_wait):
            if self.test_service_availability():
                recovered = True
                break
            time.sleep(1)
        
        def test_recovery():
            if not recovered:
                return False, f"服务在 {max_wait} 秒内未恢复", {}
            if not self.test_query_functionality():
                return False, "服务恢复后查询功能异常", {}
            return True, f"服务已恢复 (耗时 {i+1} 秒)", {}
        
        result = self.run_test(test_recovery, "服务恢复检查")
        
        # 6. 验证数据完整性
        def test_data_integrity():
            try:
                result = self.client.query("cpu_usage")
                if result.status == "success":
                    return True, "数据完整性验证通过", {}
                else:
                    return False, f"数据查询失败: {result.error}", {}
            except Exception as e:
                return False, f"数据完整性验证异常: {e}", {}
        
        result = self.run_test(test_data_integrity, "数据完整性验证")
    
    def test_graceful_shutdown(self):
        """测试优雅关闭"""
        self.logger.subsection("测试优雅关闭")
        
        # 1. 发送优雅关闭信号
        def graceful_shutdown():
            if self.chronodb_pid:
                try:
                    subprocess.run(["kill", "-TERM", str(self.chronodb_pid)], check=True)
                    return True, "已发送 TERM 信号", {}
                except Exception as e:
                    return False, f"发送信号失败: {e}", {}
            return False, "未找到服务 PID", {}
        
        result = self.run_test(graceful_shutdown, "发送优雅关闭信号")
        
        # 2. 等待服务停止
        time.sleep(3)
        
        # 3. 验证服务已停止
        def verify_stopped():
            if self.test_service_availability():
                return False, "服务仍在运行", {}
            return True, "服务已停止", {}
        
        result = self.run_test(verify_stopped, "验证服务已停止")
        
        # 4. 重新启动服务
        def restart_service():
            try:
                subprocess.Popen(
                    ["./target/release/chronodb-server", "--config", "config/test.yaml"],
                    stdout=subprocess.DEVNULL,
                    stderr=subprocess.DEVNULL,
                    cwd="/Users/zhb/workspace/chonodb"
                )
                return True, "服务已重新启动", {}
            except Exception as e:
                return False, f"启动失败: {e}", {}
        
        result = self.run_test(restart_service, "重新启动服务")
        
        # 5. 等待恢复
        time.sleep(5)
        
        # 6. 验证恢复
        def verify_recovery():
            if not self.test_service_availability():
                return False, "服务未恢复", {}
            if not self.test_query_functionality():
                return False, "查询功能未恢复", {}
            return True, "服务已完全恢复", {}
        
        result = self.run_test(verify_recovery, "验证服务恢复")
    
    def test_continuous_queries_during_recovery(self):
        """测试恢复期间持续查询"""
        self.logger.subsection("测试恢复期间持续查询")
        
        import threading
        import requests
        
        query_results = {"success": 0, "failed": 0}
        stop_flag = threading.Event()
        
        def continuous_query():
            while not stop_flag.is_set():
                try:
                    response = requests.get(
                        f"{self.chronodb_url}/api/v1/query?query=up",
                        timeout=2
                    )
                    if response.status_code == 200:
                        query_results["success"] += 1
                    else:
                        query_results["failed"] += 1
                except:
                    query_results["failed"] += 1
                time.sleep(0.5)
        
        # 启动持续查询线程
        query_thread = threading.Thread(target=continuous_query)
        query_thread.start()
        
        # 执行重启
        self.logger.info("执行服务重启...")
        if self.chronodb_pid:
            subprocess.run(["kill", str(self.chronodb_pid)], check=False)
        
        time.sleep(2)
        
        subprocess.Popen(
            ["./target/release/chronodb-server", "--config", "config/test.yaml"],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            cwd="/Users/zhb/workspace/chonodb"
        )
        
        # 等待恢复
        time.sleep(10)
        stop_flag.set()
        query_thread.join(timeout=5)
        
        def test_continuous_query():
            total = query_results["success"] + query_results["failed"]
            if total == 0:
                return False, "未执行任何查询", {}
            
            success_rate = query_results["success"] / total * 100
            self.logger.info(f"持续查询结果: 成功 {query_results['success']}, 失败 {query_results['failed']}, 成功率 {success_rate:.1f}%")
            
            if success_rate >= 50:
                return True, f"持续查询成功率 {success_rate:.1f}%", query_results
            else:
                return False, f"持续查询成功率过低: {success_rate:.1f}%", query_results
        
        result = self.run_test(test_continuous_query, "恢复期间持续查询")
    
    def run_all_tests(self) -> TestSuite:
        """运行所有故障注入测试"""
        self.setup()
        
        try:
            self.test_container_restart()
            self.test_graceful_shutdown()
            self.test_continuous_queries_during_recovery()
        finally:
            self.teardown()
        
        return self.suite


def main():
    """主函数"""
    logger = get_logger()
    logger.section("ChronoDB 故障注入测试")
    
    # 运行测试
    tester = FaultInjectionTest()
    suite = tester.run_all_tests()
    
    # 打印结果
    logger.section("故障注入测试结果")
    logger.info(f"总测试数: {suite.total_count}")
    logger.info(f"通过: {suite.passed_count}")
    logger.info(f"失败: {suite.failed_count}")
    logger.info(f"通过率: {suite.pass_rate:.1f}%")
    
    # 生成报告
    from reports.report_generator import create_report_generator
    generator = create_report_generator()
    reports = generator.generate_all_reports(
        test_suites=[suite],
        metadata={
            "test_type": "fault_injection",
            "chronodb_url": "http://localhost:9091"
        }
    )
    
    logger.info(f"报告已生成: {reports}")
    
    return 0 if suite.failed_count == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
