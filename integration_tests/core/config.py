#!/usr/bin/env python3
"""
集成测试配置管理模块
"""

import os
import yaml
from dataclasses import dataclass, field
from typing import Dict, List, Optional, Any
from pathlib import Path


@dataclass
class ContainerConfig:
    """容器配置"""
    prometheus_image: str = "prom/prometheus:latest"
    chronodb_image: str = "chronodb:latest"
    network_name: str = "chronodb-integration-test"
    prometheus_port: int = 9090
    chronodb_port: int = 9091
    
    # 数据规模配置
    small_scale: Dict[str, Any] = field(default_factory=lambda: {
        "metrics": 10,
        "series_per_metric": 10,
        "samples_per_series": 100,
        "time_range_hours": 1
    })
    medium_scale: Dict[str, Any] = field(default_factory=lambda: {
        "metrics": 50,
        "series_per_metric": 50,
        "samples_per_series": 1000,
        "time_range_hours": 6
    })
    large_scale: Dict[str, Any] = field(default_factory=lambda: {
        "metrics": 100,
        "series_per_metric": 100,
        "samples_per_series": 10000,
        "time_range_hours": 24
    })


@dataclass
class TestConfig:
    """测试配置"""
    # 测试模式
    test_standalone: bool = True
    test_distributed: bool = True
    
    # 测试数据规模
    test_scales: List[str] = field(default_factory=lambda: ["small", "medium"])
    
    # 故障注入配置
    enable_fault_injection: bool = True
    fo_probability: float = 0.1
    restart_probability: float = 0.05
    
    # 查询测试配置
    query_time_ranges: List[str] = field(default_factory=lambda: ["1m", "5m", "1h", "6h", "1d"])
    query_steps: List[str] = field(default_factory=lambda: ["1s", "15s", "1m", "5m"])
    
    # 结果对比容差
    value_tolerance: float = 0.01  # 1% 容差
    timestamp_tolerance_ms: int = 1000  # 1秒容差
    
    # 超时配置
    write_timeout: int = 30
    query_timeout: int = 60
    container_start_timeout: int = 60
    
    # 报告配置
    report_dir: str = "./integration_test_reports"
    save_raw_results: bool = True


@dataclass
class PromQLOperators:
    """PromQL算子配置"""
    
    # 聚合算子
    aggregations: List[str] = field(default_factory=lambda: [
        "sum", "avg", "min", "max", "count", "stddev", "stdvar",
        "topk(5)", "bottomk(5)", "quantile(0.95)"
    ])
    
    # 范围向量函数
    range_functions: List[str] = field(default_factory=lambda: [
        "rate", "irate", "increase", "delta", "idelta",
        "changes", "resets", "avg_over_time", "min_over_time",
        "max_over_time", "sum_over_time", "count_over_time",
        "quantile_over_time(0.95,", "stddev_over_time", "stdvar_over_time"
    ])
    
    # 数学函数
    math_functions: List[str] = field(default_factory=lambda: [
        "abs", "ceil", "floor", "round", "clamp", "clamp_max", "clamp_min",
        "exp", "ln", "log2", "log10", "sqrt"
    ])
    
    # 三角函数
    trig_functions: List[str] = field(default_factory=lambda: [
        "sin", "cos", "tan", "asin", "acos", "atan"
    ])
    
    # 时间函数
    time_functions: List[str] = field(default_factory=lambda: [
        "time", "timestamp", "day_of_month", "day_of_week",
        "days_in_month", "hour", "minute", "month", "year"
    ])
    
    # 标签操作函数
    label_functions: List[str] = field(default_factory=lambda: [
        "label_replace", "label_join"
    ])
    
    # 二元运算符
    binary_operators: List[str] = field(default_factory=lambda: [
        "+", "-", "*", "/", "%", "^",
        "==", "!=", ">", "<", ">=", "<=",
        "and", "or", "unless"
    ])
    
    # 集合运算符
    set_operators: List[str] = field(default_factory=lambda: [
        "and", "or", "unless"
    ])


class ConfigManager:
    """配置管理器"""
    
    def __init__(self, config_path: Optional[str] = None):
        self.config_path = config_path or self._get_default_config_path()
        self.container_config = ContainerConfig()
        self.test_config = TestConfig()
        self.promql_operators = PromQLOperators()
        self._load_config()
    
    def _get_default_config_path(self) -> str:
        """获取默认配置文件路径"""
        script_dir = Path(__file__).parent.parent
        return str(script_dir / "integration_test_config.yaml")
    
    def _load_config(self):
        """加载配置文件"""
        if os.path.exists(self.config_path):
            try:
                with open(self.config_path, 'r') as f:
                    config = yaml.safe_load(f)
                
                if config:
                    # 加载容器配置
                    if 'container' in config:
                        for key, value in config['container'].items():
                            if hasattr(self.container_config, key):
                                setattr(self.container_config, key, value)
                    
                    # 加载测试配置
                    if 'test' in config:
                        for key, value in config['test'].items():
                            if hasattr(self.test_config, key):
                                setattr(self.test_config, key, value)
                    
                    # 加载PromQL算子配置
                    if 'promql_operators' in config:
                        for key, value in config['promql_operators'].items():
                            if hasattr(self.promql_operators, key):
                                setattr(self.promql_operators, key, value)
            except Exception as e:
                print(f"警告: 加载配置文件失败: {e}, 使用默认配置")
    
    def save_config(self):
        """保存配置到文件"""
        config = {
            'container': self.container_config.__dict__,
            'test': self.test_config.__dict__,
            'promql_operators': self.promql_operators.__dict__
        }
        
        os.makedirs(os.path.dirname(self.config_path), exist_ok=True)
        with open(self.config_path, 'w') as f:
            yaml.dump(config, f, default_flow_style=False)
    
    def get_scale_config(self, scale: str) -> Dict[str, Any]:
        """获取指定规模的配置"""
        if scale == "small":
            return self.container_config.small_scale
        elif scale == "medium":
            return self.container_config.medium_scale
        elif scale == "large":
            return self.container_config.large_scale
        else:
            return self.container_config.small_scale


# 全局配置实例
_config_instance: Optional[ConfigManager] = None


def get_config(config_path: Optional[str] = None) -> ConfigManager:
    """获取全局配置实例"""
    global _config_instance
    if _config_instance is None or config_path is not None:
        _config_instance = ConfigManager(config_path)
    return _config_instance
