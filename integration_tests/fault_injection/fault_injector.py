#!/usr/bin/env python3
"""
故障注入模块
用于模拟各种故障场景，如容器重启、网络故障等
"""

import random
import time
import threading
from typing import Dict, List, Optional, Callable, Any
from dataclasses import dataclass, field
from enum import Enum
from datetime import datetime

import sys
from pathlib import Path
sys.path.insert(0, str(Path(__file__).parent.parent))
from core.logger import get_logger
from core.config import get_config


class FaultType(Enum):
    """故障类型"""
    CONTAINER_KILL = "container_kill"       # 强制停止容器
    CONTAINER_RESTART = "container_restart" # 重启容器
    CONTAINER_PAUSE = "container_pause"     # 暂停容器
    NETWORK_DELAY = "network_delay"         # 网络延迟
    NETWORK_PARTITION = "network_partition" # 网络分区
    CPU_STRESS = "cpu_stress"               # CPU压力
    MEMORY_STRESS = "memory_stress"         # 内存压力


@dataclass
class FaultEvent:
    """故障事件"""
    fault_type: FaultType
    target: str
    timestamp: datetime
    duration_ms: int
    details: Dict[str, Any] = field(default_factory=dict)
    recovered: bool = False
    
    def to_dict(self) -> Dict[str, Any]:
        return {
            'fault_type': self.fault_type.value,
            'target': self.target,
            'timestamp': self.timestamp.isoformat(),
            'duration_ms': self.duration_ms,
            'details': self.details,
            'recovered': self.recovered
        }


class FaultInjector:
    """故障注入器"""
    
    def __init__(self, container_manager):
        self.container_manager = container_manager
        self.logger = get_logger()
        self.config = get_config()
        self.fault_history: List[FaultEvent] = []
        self._injection_thread: Optional[threading.Thread] = None
        self._stop_event = threading.Event()
        self._lock = threading.Lock()
    
    def inject_container_kill(self, container_name: str, 
                             auto_recover: bool = True,
                             recover_after_ms: int = 5000) -> FaultEvent:
        """
        注入容器强制停止故障
        
        Args:
            container_name: 目标容器名
            auto_recover: 是否自动恢复
            recover_after_ms: 自动恢复时间(毫秒)
        """
        self.logger.warning(f"注入故障: 强制停止容器 {container_name}")
        
        event = FaultEvent(
            fault_type=FaultType.CONTAINER_KILL,
            target=container_name,
            timestamp=datetime.now(),
            duration_ms=recover_after_ms,
            details={'auto_recover': auto_recover}
        )
        
        # 执行故障注入
        success = self.container_manager.kill_container(container_name)
        
        if success:
            self.logger.info(f"容器 {container_name} 已被强制停止")
            
            with self._lock:
                self.fault_history.append(event)
            
            # 自动恢复
            if auto_recover:
                def recover():
                    time.sleep(recover_after_ms / 1000)
                    self.logger.info(f"自动恢复容器 {container_name}")
                    self.container_manager.restart_container(container_name)
                    event.recovered = True
                
                threading.Thread(target=recover, daemon=True).start()
        else:
            self.logger.error(f"故障注入失败: 无法停止容器 {container_name}")
        
        return event
    
    def inject_container_restart(self, container_name: str) -> FaultEvent:
        """
        注入容器重启故障
        
        Args:
            container_name: 目标容器名
        """
        self.logger.warning(f"注入故障: 重启容器 {container_name}")
        
        start_time = time.time()
        success = self.container_manager.restart_container(container_name)
        duration_ms = int((time.time() - start_time) * 1000)
        
        event = FaultEvent(
            fault_type=FaultType.CONTAINER_RESTART,
            target=container_name,
            timestamp=datetime.now(),
            duration_ms=duration_ms,
            details={'success': success}
        )
        
        if success:
            self.logger.info(f"容器 {container_name} 重启成功，耗时 {duration_ms}ms")
            event.recovered = True
        else:
            self.logger.error(f"容器 {container_name} 重启失败")
        
        with self._lock:
            self.fault_history.append(event)
        
        return event
    
    def inject_container_pause(self, container_name: str,
                              duration_ms: int = 5000) -> FaultEvent:
        """
        注入容器暂停故障
        
        Args:
            container_name: 目标容器名
            duration_ms: 暂停持续时间(毫秒)
        """
        self.logger.warning(f"注入故障: 暂停容器 {container_name} {duration_ms}ms")
        
        event = FaultEvent(
            fault_type=FaultType.CONTAINER_PAUSE,
            target=container_name,
            timestamp=datetime.now(),
            duration_ms=duration_ms
        )
        
        # 暂停容器
        success = self.container_manager.pause_container(container_name)
        
        if success:
            self.logger.info(f"容器 {container_name} 已暂停")
            
            with self._lock:
                self.fault_history.append(event)
            
            # 定时恢复
            def recover():
                time.sleep(duration_ms / 1000)
                self.logger.info(f"恢复容器 {container_name}")
                self.container_manager.unpause_container(container_name)
                event.recovered = True
            
            threading.Thread(target=recover, daemon=True).start()
        else:
            self.logger.error(f"故障注入失败: 无法暂停容器 {container_name}")
        
        return event
    
    def inject_random_fault(self, container_names: List[str],
                           fault_types: Optional[List[FaultType]] = None) -> FaultEvent:
        """
        随机注入故障
        
        Args:
            container_names: 候选容器列表
            fault_types: 候选故障类型列表，默认所有类型
        """
        if not container_names:
            raise ValueError("容器列表不能为空")
        
        fault_types = fault_types or list(FaultType)
        
        # 随机选择目标和故障类型
        target = random.choice(container_names)
        fault_type = random.choice(fault_types)
        
        self.logger.info(f"随机故障注入: {fault_type.value} -> {target}")
        
        # 根据故障类型执行注入
        if fault_type == FaultType.CONTAINER_KILL:
            return self.inject_container_kill(target)
        elif fault_type == FaultType.CONTAINER_RESTART:
            return self.inject_container_restart(target)
        elif fault_type == FaultType.CONTAINER_PAUSE:
            return self.inject_container_pause(target)
        else:
            self.logger.warning(f"未实现的故障类型: {fault_type.value}")
            return None
    
    def start_random_fault_injection(self, container_names: List[str],
                                    interval_seconds: float = 30.0,
                                    fault_probability: float = 0.1,
                                    fault_types: Optional[List[FaultType]] = None):
        """
        启动随机故障注入线程
        
        Args:
            container_names: 候选容器列表
            interval_seconds: 检查间隔(秒)
            fault_probability: 每次检查的故障注入概率
            fault_types: 候选故障类型列表
        """
        if self._injection_thread and self._injection_thread.is_alive():
            self.logger.warning("故障注入线程已在运行")
            return
        
        self._stop_event.clear()
        
        def injection_loop():
            self.logger.info("随机故障注入线程已启动")
            
            while not self._stop_event.is_set():
                # 随机决定是否注入故障
                if random.random() < fault_probability:
                    try:
                        self.inject_random_fault(container_names, fault_types)
                    except Exception as e:
                        self.logger.error(f"故障注入异常: {e}")
                
                # 等待下一次检查
                self._stop_event.wait(interval_seconds)
            
            self.logger.info("随机故障注入线程已停止")
        
        self._injection_thread = threading.Thread(target=injection_loop, daemon=True)
        self._injection_thread.start()
    
    def stop_random_fault_injection(self):
        """停止随机故障注入"""
        if self._injection_thread and self._injection_thread.is_alive():
            self._stop_event.set()
            self._injection_thread.join(timeout=5)
            self.logger.info("随机故障注入已停止")
    
    def get_fault_history(self) -> List[FaultEvent]:
        """获取故障历史"""
        with self._lock:
            return self.fault_history.copy()
    
    def clear_fault_history(self):
        """清除故障历史"""
        with self._lock:
            self.fault_history.clear()
    
    def wait_for_recovery(self, container_name: str, timeout_seconds: float = 60.0) -> bool:
        """
        等待容器恢复
        
        Args:
            container_name: 容器名
            timeout_seconds: 超时时间(秒)
        
        Returns:
            是否在超时前恢复
        """
        self.logger.info(f"等待容器 {container_name} 恢复...")
        
        start_time = time.time()
        while time.time() - start_time < timeout_seconds:
            if self.container_manager.is_container_running(container_name):
                self.logger.info(f"容器 {container_name} 已恢复")
                return True
            time.sleep(1)
        
        self.logger.warning(f"等待容器 {container_name} 恢复超时")
        return False


class ChaosMonkey:
    """混沌猴子 - 随机故障测试"""
    
    def __init__(self, container_manager, fault_injector: FaultInjector):
        self.container_manager = container_manager
        self.fault_injector = fault_injector
        self.logger = get_logger()
        self.config = get_config()
        self._running = False
        self._results: List[Dict[str, Any]] = []
    
    def run_chaos_test(self, container_names: List[str],
                      test_duration_seconds: float = 300,
                      fault_interval_seconds: float = 30.0,
                      verify_func: Optional[Callable] = None) -> Dict[str, Any]:
        """
        运行混沌测试
        
        Args:
            container_names: 目标容器列表
            test_duration_seconds: 测试持续时间
            fault_interval_seconds: 故障注入间隔
            verify_func: 验证函数，在每次故障后调用
        
        Returns:
            测试结果
        """
        self.logger.section(f"开始混沌测试 (持续时间: {test_duration_seconds}s)")
        
        start_time = time.time()
        fault_count = 0
        verify_passed = 0
        verify_failed = 0
        
        self._running = True
        
        try:
            while self._running and (time.time() - start_time) < test_duration_seconds:
                # 注入随机故障
                try:
                    event = self.fault_injector.inject_random_fault(container_names)
                    if event:
                        fault_count += 1
                        
                        # 等待恢复
                        time.sleep(5)
                        
                        # 执行验证
                        if verify_func:
                            try:
                                if verify_func():
                                    verify_passed += 1
                                else:
                                    verify_failed += 1
                            except Exception as e:
                                self.logger.error(f"验证函数异常: {e}")
                                verify_failed += 1
                        
                        # 等待下一次故障
                        time.sleep(max(0, fault_interval_seconds - 5))
                except Exception as e:
                    self.logger.error(f"故障注入异常: {e}")
                    time.sleep(fault_interval_seconds)
        
        finally:
            self._running = False
        
        # 生成测试结果
        result = {
            'test_duration_seconds': time.time() - start_time,
            'fault_count': fault_count,
            'verify_passed': verify_passed,
            'verify_failed': verify_failed,
            'fault_history': [e.to_dict() for e in self.fault_injector.get_fault_history()]
        }
        
        self._results.append(result)
        
        self.logger.section("混沌测试完成")
        self.logger.info(f"故障注入次数: {fault_count}")
        self.logger.info(f"验证通过: {verify_passed}")
        self.logger.info(f"验证失败: {verify_failed}")
        
        return result
    
    def stop(self):
        """停止混沌测试"""
        self._running = False
        self.logger.info("混沌测试已停止")
    
    def get_results(self) -> List[Dict[str, Any]]:
        """获取所有测试结果"""
        return self._results.copy()


# 便捷函数
def create_fault_injector(container_manager) -> FaultInjector:
    """创建故障注入器"""
    return FaultInjector(container_manager)


def create_chaos_monkey(container_manager, fault_injector: Optional[FaultInjector] = None) -> ChaosMonkey:
    """创建混沌猴子"""
    if fault_injector is None:
        fault_injector = create_fault_injector(container_manager)
    return ChaosMonkey(container_manager, fault_injector)
