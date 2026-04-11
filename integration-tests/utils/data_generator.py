"""
测试数据生成器
"""
import random
import time
from typing import Dict, List, Tuple


class DataGenerator:
    """测试数据生成器"""
    
    def __init__(self):
        self.now_ms = int(time.time() * 1000)
    
    def generate_time_series(
        self,
        metric_name: str,
        labels: Dict[str, str],
        start_time: int,
        end_time: int,
        interval: int = 1000,
        value_pattern: str = "random"
    ) -> List[str]:
        """生成时间序列数据"""
        lines = []
        label_str = ",".join([f'{k}="{v}"' for k, v in labels.items()])
        
        current_time = start_time
        while current_time <= end_time:
            if value_pattern == "random":
                value = round(random.uniform(0, 100), 2)
            elif value_pattern == "increment":
                value = (current_time - start_time) / interval
            elif value_pattern == "constant":
                value = 50.0
            else:
                value = round(random.uniform(0, 100), 2)
            
            line = f"{metric_name}{{{label_str}}} {value} {current_time}"
            lines.append(line)
            current_time += interval
        
        return lines
    
    def generate_batch_data(
        self,
        num_metrics: int,
        num_series: int,
        num_points: int,
        time_range: Tuple[int, int]
    ) -> List[str]:
        """批量生成测试数据"""
        all_lines = []
        start_time, end_time = time_range
        interval = (end_time - start_time) // num_points
        
        for i in range(num_metrics):
            metric_name = f"batch_metric_{i}"
            for j in range(num_series):
                labels = {
                    "job": f"job_{j % 10}",
                    "instance": f"instance_{j}",
                    "region": random.choice(["east", "west", "north", "south"])
                }
                lines = self.generate_time_series(
                    metric_name,
                    labels,
                    start_time,
                    end_time,
                    interval
                )
                all_lines.extend(lines)
        
        return all_lines
    
    def generate_realistic_data(
        self,
        scenario: str = "web_server"
    ) -> List[str]:
        """生成真实场景数据"""
        scenarios = {
            "web_server": self._generate_web_server_data,
            "database": self._generate_database_data,
            "container": self._generate_container_data
        }
        
        generator = scenarios.get(scenario, self._generate_web_server_data)
        return generator()
    
    def _generate_web_server_data(self) -> List[str]:
        """生成 Web 服务器场景数据"""
        metrics = ["http_requests_total", "http_request_duration_seconds", "http_response_size_bytes"]
        labels_list = [
            {"method": "GET", "status": "200"},
            {"method": "POST", "status": "201"},
            {"method": "GET", "status": "404"},
            {"method": "POST", "status": "500"}
        ]
        
        all_lines = []
        for metric in metrics:
            for labels in labels_list:
                lines = self.generate_time_series(
                    metric,
                    labels,
                    self.now_ms - 3600000,
                    self.now_ms,
                    1000
                )
                all_lines.extend(lines)
        
        return all_lines
    
    def _generate_database_data(self) -> List[str]:
        """生成数据库场景数据"""
        metrics = ["db_queries_total", "db_query_duration_seconds", "db_connections_active"]
        labels_list = [
            {"db": "primary", "query_type": "select"},
            {"db": "primary", "query_type": "insert"},
            {"db": "replica", "query_type": "select"}
        ]
        
        all_lines = []
        for metric in metrics:
            for labels in labels_list:
                lines = self.generate_time_series(
                    metric,
                    labels,
                    self.now_ms - 3600000,
                    self.now_ms,
                    1000
                )
                all_lines.extend(lines)
        
        return all_lines
    
    def _generate_container_data(self) -> List[str]:
        """生成容器场景数据"""
        metrics = ["container_cpu_usage_seconds_total", "container_memory_usage_bytes", "container_network_receive_bytes_total"]
        labels_list = [
            {"container": "app", "namespace": "default"},
            {"container": "db", "namespace": "default"},
            {"container": "cache", "namespace": "default"}
        ]
        
        all_lines = []
        for metric in metrics:
            for labels in labels_list:
                lines = self.generate_time_series(
                    metric,
                    labels,
                    self.now_ms - 3600000,
                    self.now_ms,
                    1000
                )
                all_lines.extend(lines)
        
        return all_lines
