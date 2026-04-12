#!/usr/bin/env python3
"""
测试数据生成器
生成各种类型的指标数据用于测试
"""

import random
import time
import math
from typing import List, Dict, Any, Optional, Tuple, Generator
from dataclasses import dataclass, field
from enum import Enum
from datetime import datetime, timedelta

import sys
from pathlib import Path
sys.path.insert(0, str(Path(__file__).parent.parent))
from core.logger import get_logger


class MetricType(Enum):
    """指标类型"""
    GAUGE = "gauge"           # 可增可减的瞬时值
    COUNTER = "counter"       # 单调递增的计数器
    HISTOGRAM = "histogram"   # 直方图
    SUMMARY = "summary"       # 摘要


@dataclass
class MetricSeries:
    """指标序列"""
    name: str
    metric_type: MetricType
    labels: Dict[str, str] = field(default_factory=dict)
    samples: List[Tuple[int, float]] = field(default_factory=list)  # (timestamp_ms, value)
    
    def to_prometheus_format(self) -> str:
        """转换为Prometheus文本格式"""
        lines = []
        
        # 构建标签字符串
        label_str = ",".join([f'{k}="{v}"' for k, v in self.labels.items()])
        if label_str:
            metric_str = f"{self.name}{{{label_str}}}"
        else:
            metric_str = self.name
        
        # 添加样本
        for ts, value in self.samples:
            lines.append(f"{metric_str} {value} {ts}")
        
        return "\n".join(lines)
    
    def to_remote_write_format(self) -> Dict[str, Any]:
        """转换为Remote Write格式"""
        return {
            "labels": {
                "__name__": self.name,
                **self.labels
            },
            "samples": [
                {"timestamp": ts, "value": value}
                for ts, value in self.samples
            ]
        }


class MetricGenerator:
    """指标数据生成器"""
    
    def __init__(self, seed: Optional[int] = None):
        self.logger = get_logger()
        if seed is not None:
            random.seed(seed)
        
        # 默认标签值池
        self.label_pools = {
            "job": ["webserver", "database", "cache", "api-gateway", "worker", "queue"],
            "instance": [f"server-{i}" for i in range(1, 11)],
            "region": ["us-east-1", "us-west-2", "eu-west-1", "ap-southeast-1", "cn-north-1"],
            "env": ["prod", "staging", "dev"],
            "team": ["platform", "backend", "frontend", "data", "sre"],
        }
    
    def generate_gauge_series(self, name: str, num_samples: int,
                              start_time_ms: int, interval_ms: int = 1000,
                              min_value: float = 0, max_value: float = 100,
                              labels: Optional[Dict[str, str]] = None,
                              pattern: str = "random") -> MetricSeries:
        """
        生成Gauge类型指标序列
        
        Args:
            name: 指标名称
            num_samples: 样本数量
            start_time_ms: 起始时间戳(毫秒)
            interval_ms: 采样间隔(毫秒)
            min_value: 最小值
            max_value: 最大值
            labels: 标签
            pattern: 数据模式 (random, sine, linear, spike)
        """
        samples = []
        
        for i in range(num_samples):
            ts = start_time_ms + i * interval_ms
            
            if pattern == "random":
                value = random.uniform(min_value, max_value)
            elif pattern == "sine":
                # 正弦波模式
                amplitude = (max_value - min_value) / 2
                offset = (max_value + min_value) / 2
                value = offset + amplitude * math.sin(2 * math.pi * i / num_samples)
            elif pattern == "linear":
                # 线性增长
                value = min_value + (max_value - min_value) * i / num_samples
            elif pattern == "spike":
                # 随机尖峰
                base = (max_value + min_value) / 2
                if random.random() < 0.05:  # 5%概率出现尖峰
                    value = max_value * random.uniform(0.8, 1.0)
                else:
                    value = base + random.uniform(-10, 10)
            else:
                value = random.uniform(min_value, max_value)
            
            samples.append((ts, round(value, 6)))
        
        return MetricSeries(
            name=name,
            metric_type=MetricType.GAUGE,
            labels=labels or {},
            samples=samples
        )
    
    def generate_counter_series(self, name: str, num_samples: int,
                                start_time_ms: int, interval_ms: int = 1000,
                                start_value: float = 0,
                                increment_min: float = 0,
                                increment_max: float = 10,
                                labels: Optional[Dict[str, str]] = None,
                                reset_prob: float = 0.0) -> MetricSeries:
        """
        生成Counter类型指标序列
        
        Args:
            name: 指标名称
            num_samples: 样本数量
            start_time_ms: 起始时间戳(毫秒)
            interval_ms: 采样间隔(毫秒)
            start_value: 起始值
            increment_min: 最小增量
            increment_max: 最大增量
            labels: 标签
            reset_prob: 重置概率（模拟进程重启）
        """
        samples = []
        current_value = start_value
        
        for i in range(num_samples):
            ts = start_time_ms + i * interval_ms
            
            # 随机重置（模拟进程重启）
            if random.random() < reset_prob:
                current_value = 0
            
            # 增加计数
            increment = random.uniform(increment_min, increment_max)
            current_value += increment
            
            samples.append((ts, round(current_value, 6)))
        
        return MetricSeries(
            name=name,
            metric_type=MetricType.COUNTER,
            labels=labels or {},
            samples=samples
        )
    
    def generate_histogram_series(self, name: str, num_samples: int,
                                  start_time_ms: int, interval_ms: int = 1000,
                                  buckets: Optional[List[float]] = None,
                                  labels: Optional[Dict[str, str]] = None) -> List[MetricSeries]:
        """
        生成Histogram类型指标序列
        
        Returns:
            包含_count, _sum, _bucket的序列列表
        """
        if buckets is None:
            buckets = [0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1, 2.5, 5, 10]
        
        count_samples = []
        sum_samples = []
        bucket_samples = {f"{name}_bucket": {le: [] for le in buckets}}
        bucket_samples[f"{name}_bucket"]["+Inf"] = []
        
        total_count = 0
        total_sum = 0.0
        bucket_counts = {le: 0 for le in buckets}
        bucket_counts["+Inf"] = 0
        
        for i in range(num_samples):
            ts = start_time_ms + i * interval_ms
            
            # 生成观测值
            # 使用指数分布模拟延迟
            value = random.expovariate(1.0)  # 均值为1的指数分布
            
            total_count += 1
            total_sum += value
            
            # 更新bucket计数
            for le in buckets:
                if value <= le:
                    bucket_counts[le] += 1
            bucket_counts["+Inf"] += 1
            
            count_samples.append((ts, float(total_count)))
            sum_samples.append((ts, round(total_sum, 6)))
            
            for le in buckets + ["+Inf"]:
                bucket_samples[f"{name}_bucket"][le].append((ts, float(bucket_counts[le])))
        
        series_list = []
        
        # _count序列
        series_list.append(MetricSeries(
            name=f"{name}_count",
            metric_type=MetricType.GAUGE,
            labels=labels or {},
            samples=count_samples
        ))
        
        # _sum序列
        series_list.append(MetricSeries(
            name=f"{name}_sum",
            metric_type=MetricType.GAUGE,
            labels=labels or {},
            samples=sum_samples
        ))
        
        # _bucket序列
        for le in buckets + ["+Inf"]:
            bucket_labels = (labels or {}).copy()
            bucket_labels["le"] = str(le)
            series_list.append(MetricSeries(
                name=f"{name}_bucket",
                metric_type=MetricType.GAUGE,
                labels=bucket_labels,
                samples=bucket_samples[f"{name}_bucket"][le]
            ))
        
        return series_list
    
    def generate_test_data_set(self, scale_config: Dict[str, Any],
                               start_time_ms: Optional[int] = None) -> List[MetricSeries]:
        """
        生成完整的测试数据集
        
        Args:
            scale_config: 规模配置 (metrics, series_per_metric, samples_per_series, time_range_hours)
            start_time_ms: 起始时间戳(毫秒)，默认当前时间前推time_range_hours
        
        Returns:
            指标序列列表
        """
        metrics = scale_config.get("metrics", 10)
        series_per_metric = scale_config.get("series_per_metric", 10)
        samples_per_series = scale_config.get("samples_per_series", 100)
        time_range_hours = scale_config.get("time_range_hours", 1)
        
        if start_time_ms is None:
            end_time_ms = int(time.time() * 1000)
            start_time_ms = end_time_ms - time_range_hours * 3600 * 1000
        
        interval_ms = (time_range_hours * 3600 * 1000) // samples_per_series
        
        all_series = []
        
        # 生成不同类型的指标
        metric_types = [
            ("cpu_usage", MetricType.GAUGE),
            ("memory_usage", MetricType.GAUGE),
            ("disk_io", MetricType.COUNTER),
            ("network_bytes", MetricType.COUNTER),
            ("request_duration", MetricType.HISTOGRAM),
            ("request_count", MetricType.COUNTER),
            ("error_rate", MetricType.GAUGE),
            ("queue_length", MetricType.GAUGE),
        ]
        
        for metric_idx in range(metrics):
            metric_name, metric_type = metric_types[metric_idx % len(metric_types)]
            metric_name = f"{metric_name}_{metric_idx // len(metric_types)}"
            
            for series_idx in range(series_per_metric):
                # 生成标签组合
                labels = self._generate_labels(series_idx)
                
                if metric_type == MetricType.GAUGE:
                    pattern = random.choice(["random", "sine", "linear", "spike"])
                    series = self.generate_gauge_series(
                        name=metric_name,
                        num_samples=samples_per_series,
                        start_time_ms=start_time_ms,
                        interval_ms=interval_ms,
                        min_value=0,
                        max_value=100,
                        labels=labels,
                        pattern=pattern
                    )
                    all_series.append(series)
                
                elif metric_type == MetricType.COUNTER:
                    series = self.generate_counter_series(
                        name=metric_name,
                        num_samples=samples_per_series,
                        start_time_ms=start_time_ms,
                        interval_ms=interval_ms,
                        start_value=random.randint(0, 1000),
                        increment_min=0,
                        increment_max=10,
                        labels=labels,
                        reset_prob=0.001  # 0.1%概率重置
                    )
                    all_series.append(series)
                
                elif metric_type == MetricType.HISTOGRAM:
                    series_list = self.generate_histogram_series(
                        name=metric_name,
                        num_samples=samples_per_series,
                        start_time_ms=start_time_ms,
                        interval_ms=interval_ms,
                        labels=labels
                    )
                    all_series.extend(series_list)
        
        self.logger.info(f"生成了 {len(all_series)} 个时间序列，总计 {sum(len(s.samples) for s in all_series)} 个样本")
        
        return all_series
    
    def _generate_labels(self, series_idx: int) -> Dict[str, str]:
        """生成标签组合"""
        labels = {}
        
        # 根据series_idx生成不同的标签组合
        for label_name, label_values in self.label_pools.items():
            idx = (series_idx + hash(label_name)) % len(label_values)
            labels[label_name] = label_values[idx]
        
        return labels
    
    def generate_batch_data(self, series_list: List[MetricSeries],
                           batch_size: int = 1000) -> Generator[str, None, None]:
        """
        分批生成Prometheus格式的数据
        
        Args:
            series_list: 指标序列列表
            batch_size: 每批样本数量
        
        Yields:
            Prometheus格式的数据批次
        """
        all_samples = []
        
        for series in series_list:
            for ts, value in series.samples:
                label_str = ",".join([f'{k}="{v}"' for k, v in series.labels.items()])
                if label_str:
                    metric_str = f"{series.name}{{{label_str}}}"
                else:
                    metric_str = series.name
                all_samples.append(f"{metric_str} {value} {ts}")
        
        # 按批次生成
        for i in range(0, len(all_samples), batch_size):
            batch = all_samples[i:i+batch_size]
            yield "\n".join(batch)
    
    def generate_comparison_data(self, num_metrics: int = 10,
                                 num_samples: int = 100) -> Tuple[List[MetricSeries], List[MetricSeries]]:
        """
        生成用于对比测试的相同数据集
        
        Returns:
            (prometheus_data, chronodb_data) - 两份相同的数据
        """
        scale_config = {
            "metrics": num_metrics,
            "series_per_metric": 1,
            "samples_per_series": num_samples,
            "time_range_hours": 1
        }
        
        # 使用固定种子确保两份数据相同
        random_state = random.getstate()
        
        data1 = self.generate_test_data_set(scale_config)
        
        random.setstate(random_state)
        data2 = self.generate_test_data_set(scale_config)
        
        return data1, data2


class DataWriter:
    """数据写入器"""
    
    def __init__(self, base_url: str):
        self.base_url = base_url
        self.logger = get_logger()
    
    def write_series(self, series: MetricSeries, timeout: int = 30) -> bool:
        """写入单个序列"""
        import requests
        
        data = series.to_prometheus_format()
        url = f"{self.base_url}/api/v1/write"
        
        try:
            response = requests.post(
                url,
                data=data,
                headers={"Content-Type": "text/plain"},
                timeout=timeout
            )
            return response.status_code in [200, 204]
        except Exception as e:
            self.logger.error(f"写入失败: {e}")
            return False
    
    def write_batch(self, series_list: List[MetricSeries],
                   batch_size: int = 1000, timeout: int = 30) -> Tuple[int, int]:
        """
        批量写入序列
        
        Returns:
            (成功数, 失败数)
        """
        import requests
        
        generator = MetricGenerator()
        success_count = 0
        fail_count = 0
        
        for batch_data in generator.generate_batch_data(series_list, batch_size):
            url = f"{self.base_url}/api/v1/write"
            
            try:
                response = requests.post(
                    url,
                    data=batch_data,
                    headers={"Content-Type": "text/plain"},
                    timeout=timeout
                )
                if response.status_code in [200, 204]:
                    success_count += 1
                else:
                    fail_count += 1
                    self.logger.warning(f"写入失败: {response.status_code}")
            except Exception as e:
                fail_count += 1
                self.logger.error(f"写入异常: {e}")
        
        return success_count, fail_count


# 便捷函数
def create_metric_generator(seed: Optional[int] = None) -> MetricGenerator:
    """创建指标生成器"""
    return MetricGenerator(seed)


def create_data_writer(base_url: str) -> DataWriter:
    """创建数据写入器"""
    return DataWriter(base_url)
