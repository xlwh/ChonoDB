#!/usr/bin/env python3
"""
集成测试日志模块
"""

import logging
import sys
from datetime import datetime
from pathlib import Path
from typing import Optional


class Colors:
    """终端颜色"""
    RESET = '\033[0m'
    BOLD = '\033[1m'
    DIM = '\033[2m'
    
    # 前景色
    BLACK = '\033[30m'
    RED = '\033[31m'
    GREEN = '\033[32m'
    YELLOW = '\033[33m'
    BLUE = '\033[34m'
    MAGENTA = '\033[35m'
    CYAN = '\033[36m'
    WHITE = '\033[37m'
    
    # 背景色
    BG_RED = '\033[41m'
    BG_GREEN = '\033[42m'
    BG_YELLOW = '\033[43m'
    BG_BLUE = '\033[44m'


class ColoredFormatter(logging.Formatter):
    """带颜色的日志格式化器"""
    
    LEVEL_COLORS = {
        logging.DEBUG: Colors.CYAN,
        logging.INFO: Colors.GREEN,
        logging.WARNING: Colors.YELLOW,
        logging.ERROR: Colors.RED,
        logging.CRITICAL: Colors.BG_RED + Colors.WHITE,
    }
    
    def __init__(self, use_colors: bool = True):
        super().__init__()
        self.use_colors = use_colors and sys.stdout.isatty()
    
    def format(self, record: logging.LogRecord) -> str:
        # 获取颜色
        color = self.LEVEL_COLORS.get(record.levelno, Colors.RESET) if self.use_colors else ''
        reset = Colors.RESET if self.use_colors else ''
        
        # 格式化时间
        timestamp = datetime.fromtimestamp(record.created).strftime('%Y-%m-%d %H:%M:%S.%f')[:-3]
        
        # 格式化消息
        level_name = f"{color}{record.levelname:8}{reset}"
        
        # 构建日志字符串
        log_str = f"[{timestamp}] {level_name} [{record.name}] {record.getMessage()}"
        
        # 添加异常信息
        if record.exc_info:
            log_str += f"\n{self.formatException(record.exc_info)}"
        
        return log_str


class TestLogger:
    """测试日志管理器"""
    
    _instance: Optional['TestLogger'] = None
    
    def __new__(cls, *args, **kwargs):
        if cls._instance is None:
            cls._instance = super().__new__(cls)
        return cls._instance
    
    def __init__(self, name: str = "integration_test", log_dir: Optional[str] = None, 
                 log_level: int = logging.INFO, use_colors: bool = True):
        if hasattr(self, '_initialized'):
            return
        
        self._initialized = True
        self.name = name
        self.log_dir = Path(log_dir) if log_dir else Path("./integration_test_logs")
        self.log_level = log_level
        self.use_colors = use_colors
        
        # 创建日志目录
        self.log_dir.mkdir(parents=True, exist_ok=True)
        
        # 创建logger
        self.logger = logging.getLogger(name)
        self.logger.setLevel(log_level)
        self.logger.handlers = []  # 清除现有处理器
        
        # 控制台处理器
        console_handler = logging.StreamHandler(sys.stdout)
        console_handler.setLevel(log_level)
        console_handler.setFormatter(ColoredFormatter(use_colors=use_colors))
        self.logger.addHandler(console_handler)
        
        # 文件处理器
        log_file = self.log_dir / f"{name}_{datetime.now().strftime('%Y%m%d_%H%M%S')}.log"
        file_handler = logging.FileHandler(log_file, encoding='utf-8')
        file_handler.setLevel(log_level)
        file_handler.setFormatter(logging.Formatter(
            '[%(asctime)s] %(levelname)-8s [%(name)s] %(message)s'
        ))
        self.logger.addHandler(file_handler)
        
        self.log_file = log_file
        
        # 测试统计
        self.stats = {
            'tests_run': 0,
            'tests_passed': 0,
            'tests_failed': 0,
            'tests_skipped': 0,
        }
    
    def debug(self, msg: str):
        """调试日志"""
        self.logger.debug(msg)
    
    def info(self, msg: str):
        """信息日志"""
        self.logger.info(msg)
    
    def warning(self, msg: str):
        """警告日志"""
        self.logger.warning(msg)
    
    def error(self, msg: str):
        """错误日志"""
        self.logger.error(msg)
    
    def critical(self, msg: str):
        """严重错误日志"""
        self.logger.critical(msg)
    
    def section(self, title: str):
        """打印章节标题"""
        separator = "=" * 60
        self.logger.info(f"\n{separator}")
        self.logger.info(f"  {title}")
        self.logger.info(f"{separator}")
    
    def subsection(self, title: str):
        """打印子章节标题"""
        separator = "-" * 40
        self.logger.info(f"\n{separator}")
        self.logger.info(f"  {title}")
        self.logger.info(f"{separator}")
    
    def test_pass(self, msg: str):
        """测试通过"""
        self.stats['tests_run'] += 1
        self.stats['tests_passed'] += 1
        if self.use_colors and sys.stdout.isatty():
            self.logger.info(f"{Colors.GREEN}✓ PASS{Colors.RESET} {msg}")
        else:
            self.logger.info(f"[PASS] {msg}")
    
    def test_fail(self, msg: str):
        """测试失败"""
        self.stats['tests_run'] += 1
        self.stats['tests_failed'] += 1
        if self.use_colors and sys.stdout.isatty():
            self.logger.error(f"{Colors.RED}✗ FAIL{Colors.RESET} {msg}")
        else:
            self.logger.error(f"[FAIL] {msg}")
    
    def test_skip(self, msg: str):
        """测试跳过"""
        self.stats['tests_run'] += 1
        self.stats['tests_skipped'] += 1
        if self.use_colors and sys.stdout.isatty():
            self.logger.warning(f"{Colors.YELLOW}⊘ SKIP{Colors.RESET} {msg}")
        else:
            self.logger.warning(f"[SKIP] {msg}")
    
    def test_warn(self, msg: str):
        """测试警告"""
        if self.use_colors and sys.stdout.isatty():
            self.logger.warning(f"{Colors.YELLOW}⚠ WARN{Colors.RESET} {msg}")
        else:
            self.logger.warning(f"[WARN] {msg}")
    
    def test_info(self, msg: str):
        """测试信息"""
        if self.use_colors and sys.stdout.isatty():
            self.logger.info(f"{Colors.CYAN}ℹ INFO{Colors.RESET} {msg}")
        else:
            self.logger.info(f"[INFO] {msg}")
    
    def print_summary(self):
        """打印测试汇总"""
        self.section("测试结果汇总")
        
        total = self.stats['tests_run']
        passed = self.stats['tests_passed']
        failed = self.stats['tests_failed']
        skipped = self.stats['tests_skipped']
        
        if self.use_colors and sys.stdout.isatty():
            self.logger.info(f"  通过: {Colors.GREEN}{passed}{Colors.RESET}")
            self.logger.info(f"  失败: {Colors.RED}{failed}{Colors.RESET}")
            self.logger.info(f"  跳过: {Colors.YELLOW}{skipped}{Colors.RESET}")
            self.logger.info(f"  总计: {total}")
            
            if total > 0:
                rate = passed / total * 100
                self.logger.info(f"  通过率: {rate:.1f}%")
            
            print()
            if failed == 0:
                self.logger.info(f"  {Colors.GREEN}{Colors.BOLD}🎉 所有测试通过！{Colors.RESET}")
            else:
                self.logger.error(f"  {Colors.RED}{Colors.BOLD}❌ 有 {failed} 个测试失败{Colors.RESET}")
        else:
            self.logger.info(f"  通过: {passed}")
            self.logger.info(f"  失败: {failed}")
            self.logger.info(f"  跳过: {skipped}")
            self.logger.info(f"  总计: {total}")
            
            if total > 0:
                rate = passed / total * 100
                self.logger.info(f"  通过率: {rate:.1f}%")
            
            print()
            if failed == 0:
                self.logger.info("  [SUCCESS] 所有测试通过！")
            else:
                self.logger.error(f"  [FAILED] 有 {failed} 个测试失败")
    
    def reset_stats(self):
        """重置统计"""
        self.stats = {
            'tests_run': 0,
            'tests_passed': 0,
            'tests_failed': 0,
            'tests_skipped': 0,
        }


def get_logger(name: str = "integration_test", log_dir: Optional[str] = None,
               log_level: int = logging.INFO, use_colors: bool = True) -> TestLogger:
    """获取测试日志管理器实例"""
    return TestLogger(name, log_dir, log_level, use_colors)
