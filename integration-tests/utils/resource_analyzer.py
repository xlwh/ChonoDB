"""
资源使用分析器
"""
import requests
from typing import Dict, List


class ResourceAnalyzer:
    """资源使用分析器"""
    
    def __init__(self, monitoring_prometheus_url: str):
        self.prometheus_url = monitoring_prometheus_url
    
    def query_cpu_usage(
        self,
        node: str,
        start_time: int,
        end_time: int
    ) -> List[Dict]:
        """查询 CPU 使用率"""
        query = f'cpu_usage_percent{{node="{node}"}}'
        return self._query_range(query, start_time, end_time)
    
    def query_memory_usage(
        self,
        node: str,
        start_time: int,
        end_time: int
    ) -> List[Dict]:
        """查询内存使用率"""
        query = f'memory_usage_percent{{node="{node}"}}'
        return self._query_range(query, start_time, end_time)
    
    def query_disk_io(
        self,
        node: str,
        start_time: int,
        end_time: int
    ) -> Dict:
        """查询磁盘 I/O"""
        read_query = f'disk_read_bytes_total{{node="{node}"}}'
        write_query = f'disk_written_bytes_total{{node="{node}"}}'
        return {
            'read': self._query_range(read_query, start_time, end_time),
            'write': self._query_range(write_query, start_time, end_time)
        }
    
    def query_network_traffic(
        self,
        node: str,
        start_time: int,
        end_time: int
    ) -> Dict:
        """查询网络流量"""
        receive_query = f'network_receive_bytes_total{{node="{node}"}}'
        transmit_query = f'network_transmit_bytes_total{{node="{node}"}}'
        return {
            'receive': self._query_range(receive_query, start_time, end_time),
            'transmit': self._query_range(transmit_query, start_time, end_time)
        }
    
    def compare_resource_usage(
        self,
        chronodb_node: str,
        prometheus_node: str,
        start_time: int,
        end_time: int
    ) -> Dict:
        """对比 ChronoDB 和 Prometheus 的资源使用"""
        return {
            'cpu': {
                'chronodb': self.query_cpu_usage(chronodb_node, start_time, end_time),
                'prometheus': self.query_cpu_usage(prometheus_node, start_time, end_time)
            },
            'memory': {
                'chronodb': self.query_memory_usage(chronodb_node, start_time, end_time),
                'prometheus': self.query_memory_usage(prometheus_node, start_time, end_time)
            },
            'disk_io': {
                'chronodb': self.query_disk_io(chronodb_node, start_time, end_time),
                'prometheus': self.query_disk_io(prometheus_node, start_time, end_time)
            },
            'network': {
                'chronodb': self.query_network_traffic(chronodb_node, start_time, end_time),
                'prometheus': self.query_network_traffic(prometheus_node, start_time, end_time)
            }
        }
    
    def identify_bottlenecks(
        self,
        resource_data: Dict,
        thresholds: Dict = None
    ) -> List[Dict]:
        """识别资源瓶颈"""
        if thresholds is None:
            thresholds = {
                'cpu_high': 80.0,
                'memory_high': 85.0,
                'disk_io_high': 100 * 1024 * 1024,
                'network_high': 50 * 1024 * 1024
            }
        
        bottlenecks = []
        
        for resource_type, data in resource_data.items():
            if resource_type == 'cpu':
                for node, values in data.items():
                    if values:
                        max_usage = max(v['value'] for v in values)
                        if max_usage > thresholds['cpu_high']:
                            bottlenecks.append({
                                'type': 'cpu',
                                'node': node,
                                'value': max_usage,
                                'threshold': thresholds['cpu_high'],
                                'severity': 'high' if max_usage > 90 else 'medium'
                            })
            
            elif resource_type == 'memory':
                for node, values in data.items():
                    if values:
                        max_usage = max(v['value'] for v in values)
                        if max_usage > thresholds['memory_high']:
                            bottlenecks.append({
                                'type': 'memory',
                                'node': node,
                                'value': max_usage,
                                'threshold': thresholds['memory_high'],
                                'severity': 'high' if max_usage > 95 else 'medium'
                            })
        
        return bottlenecks
    
    def _query_range(
        self,
        query: str,
        start_time: int,
        end_time: int
    ) -> List[Dict]:
        """执行范围查询"""
        url = f"{self.prometheus_url}/api/v1/query_range"
        params = {
            'query': query,
            'start': start_time,
            'end': end_time,
            'step': '15s'
        }
        try:
            response = requests.get(url, params=params, timeout=10)
            if response.status_code == 200:
                result = response.json()
                return result.get('data', {}).get('result', [])
        except Exception as e:
            print(f"查询失败: {e}")
        return []
