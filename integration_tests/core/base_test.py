#!/usr/bin/env python3
"""
集成测试基类
"""

import time
import random
from abc import ABC, abstractmethod
from typing import Dict, List, Any, Optional, Callable
from dataclasses import dataclass, field
from datetime import datetime

from .logger import get_logger
from .config import get_config


@dataclass
class TestResult:
    """测试结果"""
    name: str
    passed: bool
    duration_ms: float
    message: str = ""
    details: Dict[str, Any] = field(default_factory=dict)
    timestamp: datetime = field(default_factory=datetime.now)
    
    def to_dict(self) -> Dict[str, Any]:
        return {
            'name': self.name,
            'passed': self.passed,
            'duration_ms': self.duration_ms,
            'message': self.message,
            'details': self.details,
            'timestamp': self.timestamp.isoformat()
        }


@dataclass
class TestSuite:
    """测试套件"""
    name: str
    results: List[TestResult] = field(default_factory=list)
    start_time: Optional[datetime] = None
    end_time: Optional[datetime] = None
    
    def add_result(self, result: TestResult):
        self.results.append(result)
    
    @property
    def passed_count(self) -> int:
        return sum(1 for r in self.results if r.passed)
    
    @property
    def failed_count(self) -> int:
        return sum(1 for r in self.results if not r.passed)
    
    @property
    def total_count(self) -> int:
        return len(self.results)
    
    @property
    def pass_rate(self) -> float:
        if self.total_count == 0:
            return 0.0
        return self.passed_count / self.total_count * 100
    
    @property
    def duration_ms(self) -> float:
        if self.start_time and self.end_time:
            return (self.end_time - self.start_time).total_seconds() * 1000
        return 0.0
    
    def to_dict(self) -> Dict[str, Any]:
        return {
            'name': self.name,
            'start_time': self.start_time.isoformat() if self.start_time else None,
            'end_time': self.end_time.isoformat() if self.end_time else None,
            'duration_ms': self.duration_ms,
            'passed_count': self.passed_count,
            'failed_count': self.failed_count,
            'total_count': self.total_count,
            'pass_rate': self.pass_rate,
            'results': [r.to_dict() for r in self.results]
        }


class BaseTest(ABC):
    """测试基类"""
    
    def __init__(self, name: str):
        self.name = name
        self.logger = get_logger()
        self.config = get_config()
        self.suite = TestSuite(name)
        self._setup_done = False
    
    def setup(self):
        """测试准备"""
        if not self._setup_done:
            self.logger.section(f"测试套件: {self.name}")
            self.suite.start_time = datetime.now()
            self._do_setup()
            self._setup_done = True
    
    def teardown(self):
        """测试清理"""
        if self._setup_done:
            self.suite.end_time = datetime.now()
            self._do_teardown()
            self._setup_done = False
    
    @abstractmethod
    def _do_setup(self):
        """子类实现的具体准备逻辑"""
        pass
    
    @abstractmethod
    def _do_teardown(self):
        """子类实现的具体清理逻辑"""
        pass
    
    def run_test(self, test_func: Callable, test_name: str, *args, **kwargs) -> TestResult:
        """运行单个测试"""
        start_time = time.time()
        
        try:
            result = test_func(*args, **kwargs)
            
            if isinstance(result, bool):
                passed = result
                message = "测试通过" if passed else "测试失败"
                details = {}
            elif isinstance(result, tuple):
                passed = result[0]
                message = result[1] if len(result) > 1 else ""
                details = result[2] if len(result) > 2 else {}
            else:
                passed = bool(result)
                message = str(result) if not passed else "测试通过"
                details = {}
            
        except Exception as e:
            passed = False
            message = f"异常: {str(e)}"
            details = {'exception': str(e)}
            self.logger.error(f"测试异常: {test_name}", exc_info=True)
        
        duration_ms = (time.time() - start_time) * 1000
        
        test_result = TestResult(
            name=test_name,
            passed=passed,
            duration_ms=duration_ms,
            message=message,
            details=details
        )
        
        self.suite.add_result(test_result)
        
        # 记录结果
        if passed:
            self.logger.test_pass(f"{test_name} ({duration_ms:.1f}ms)")
        else:
            self.logger.test_fail(f"{test_name} ({duration_ms:.1f}ms): {message}")
        
        return test_result
    
    def assert_true(self, condition: bool, message: str = "") -> bool:
        """断言为真"""
        if not condition:
            raise AssertionError(message or "断言失败: 期望为真")
        return True
    
    def assert_false(self, condition: bool, message: str = "") -> bool:
        """断言为假"""
        if condition:
            raise AssertionError(message or "断言失败: 期望为假")
        return True
    
    def assert_equal(self, actual: Any, expected: Any, message: str = "") -> bool:
        """断言相等"""
        if actual != expected:
            msg = message or f"断言失败: 期望 {expected}, 实际 {actual}"
            raise AssertionError(msg)
        return True
    
    def assert_almost_equal(self, actual: float, expected: float, 
                           tolerance: float = 0.01, message: str = "") -> bool:
        """断言近似相等"""
        if expected == 0:
            diff = abs(actual)
        else:
            diff = abs(actual - expected) / max(abs(expected), 1e-10)
        
        if diff > tolerance:
            msg = message or f"断言失败: 期望 {expected}, 实际 {actual}, 偏差 {diff:.2%}"
            raise AssertionError(msg)
        return True
    
    def assert_not_none(self, value: Any, message: str = "") -> bool:
        """断言不为None"""
        if value is None:
            raise AssertionError(message or "断言失败: 期望不为None")
        return True
    
    def assert_in(self, item: Any, container: Any, message: str = "") -> bool:
        """断言包含"""
        if item not in container:
            raise AssertionError(message or f"断言失败: {item} 不在 {container} 中")
        return True
    
    def assert_raises(self, exception_type: type, func: Callable, *args, **kwargs) -> bool:
        """断言抛出异常"""
        try:
            func(*args, **kwargs)
            raise AssertionError(f"断言失败: 期望抛出 {exception_type.__name__}")
        except exception_type:
            return True
    
    def sleep(self, seconds: float):
        """休眠"""
        time.sleep(seconds)
    
    def wait_for(self, condition: Callable, timeout: float = 30.0, 
                 interval: float = 0.5, message: str = "") -> bool:
        """等待条件满足"""
        start_time = time.time()
        while time.time() - start_time < timeout:
            try:
                if condition():
                    return True
            except Exception:
                pass
            time.sleep(interval)
        
        raise TimeoutError(message or f"等待超时 ({timeout}s)")
    
    def retry(self, func: Callable, max_retries: int = 3, 
              delay: float = 1.0, *args, **kwargs) -> Any:
        """重试函数"""
        last_exception = None
        for i in range(max_retries):
            try:
                return func(*args, **kwargs)
            except Exception as e:
                last_exception = e
                if i < max_retries - 1:
                    self.logger.warning(f"重试 {i+1}/{max_retries}: {e}")
                    time.sleep(delay)
        
        raise last_exception
    
    def random_delay(self, min_seconds: float = 0.1, max_seconds: float = 1.0):
        """随机延迟"""
        delay = random.uniform(min_seconds, max_seconds)
        time.sleep(delay)
    
    def get_suite_result(self) -> TestSuite:
        """获取测试套件结果"""
        return self.suite
    
    @abstractmethod
    def run_all_tests(self) -> TestSuite:
        """运行所有测试"""
        pass
