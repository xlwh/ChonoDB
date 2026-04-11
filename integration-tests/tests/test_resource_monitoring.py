"""
资源使用监控测试
测试过程中监控各节点的资源使用情况
"""
import pytest
import time
from utils.resource_analyzer import ResourceAnalyzer


class TestResourceMonitoring:
    """资源使用监控测试类"""
    
    @pytest.fixture
    def resource_analyzer(self, monitoring_prometheus_url):
        """创建资源分析器"""
        return ResourceAnalyzer(monitoring_prometheus_url)
    
    def test_cpu_usage_monitoring(self, resource_analyzer):
        """测试 CPU 使用监控"""
        end_time = int(time.time())
        start_time = end_time - 300
        
        cpu_data = resource_analyzer.query_cpu_usage(
            node="chronodb",
            start_time=start_time,
            end_time=end_time
        )
        
        assert isinstance(cpu_data, list), "CPU 数据格式错误"
    
    def test_memory_usage_monitoring(self, resource_analyzer):
        """测试内存使用监控"""
        end_time = int(time.time())
        start_time = end_time - 300
        
        memory_data = resource_analyzer.query_memory_usage(
            node="chronodb",
            start_time=start_time,
            end_time=end_time
        )
        
        assert isinstance(memory_data, list), "内存数据格式错误"
    
    def test_resource_comparison(self, resource_analyzer):
        """测试资源使用对比"""
        end_time = int(time.time())
        start_time = end_time - 300
        
        comparison = resource_analyzer.compare_resource_usage(
            chronodb_node="chronodb",
            prometheus_node="prometheus",
            start_time=start_time,
            end_time=end_time
        )
        
        assert "cpu" in comparison, "缺少 CPU 对比数据"
        assert "memory" in comparison, "缺少内存对比数据"
        assert "disk_io" in comparison, "缺少磁盘 I/O 对比数据"
        assert "network" in comparison, "缺少网络对比数据"
    
    def test_bottleneck_identification(self, resource_analyzer):
        """测试资源瓶颈识别"""
        end_time = int(time.time())
        start_time = end_time - 300
        
        resource_data = resource_analyzer.compare_resource_usage(
            chronodb_node="chronodb",
            prometheus_node="prometheus",
            start_time=start_time,
            end_time=end_time
        )
        
        bottlenecks = resource_analyzer.identify_bottlenecks(resource_data)
        
        assert isinstance(bottlenecks, list), "瓶颈识别结果格式错误"
        
        for bottleneck in bottlenecks:
            assert "type" in bottleneck, "瓶颈类型缺失"
            assert "node" in bottleneck, "节点信息缺失"
            assert "value" in bottleneck, "数值缺失"
            assert "threshold" in bottleneck, "阈值缺失"
            assert "severity" in bottleneck, "严重程度缺失"
