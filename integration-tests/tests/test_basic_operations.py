"""
基础操作测试
测试 ChronoDB 的基本数据写入和查询功能
"""
import pytest
import requests
import time


class TestBasicOperations:
    """基础操作测试类"""
    
    def test_health_check(self, chronodb_url, check_services):
        """测试健康检查"""
        response = requests.get(f"{chronodb_url}/-/healthy")
        assert response.status_code == 200, "健康检查失败"
    
    def test_single_write_and_query(self, chronodb_url, unique_metric_name, test_timestamp):
        """测试单条数据写入和查询"""
        metric_name = unique_metric_name
        test_data = f'{metric_name}{{job="test",instance="localhost"}} 42.5 {test_timestamp}'
        
        write_response = requests.post(
            f"{chronodb_url}/api/v1/write",
            data=test_data,
            headers={"Content-Type": "text/plain"}
        )
        assert write_response.status_code in [200, 204], f"写入失败: {write_response.text}"
        
        time.sleep(0.5)
        
        query_response = requests.get(
            f"{chronodb_url}/api/v1/query",
            params={"query": metric_name}
        )
        assert query_response.status_code == 200, f"查询失败: {query_response.text}"
        
        result = query_response.json()
        assert result.get("status") == "success", f"查询状态错误: {result}"
        
        data = result.get("data", {}).get("result", [])
        assert len(data) > 0, "查询结果为空"
        
        value = float(data[0]["value"][1])
        assert abs(value - 42.5) < 0.01, f"查询值不匹配: {value}"
    
    def test_batch_write(self, chronodb_url, unique_metric_name):
        """测试批量数据写入"""
        from utils.data_generator import DataGenerator
        
        generator = DataGenerator()
        lines = generator.generate_time_series(
            metric_name=unique_metric_name,
            labels={"job": "batch_test", "instance": "server-1"},
            start_time=int(time.time() * 1000) - 60000,
            end_time=int(time.time() * 1000),
            interval=1000
        )
        
        batch_data = "\n".join(lines)
        write_response = requests.post(
            f"{chronodb_url}/api/v1/write",
            data=batch_data,
            headers={"Content-Type": "text/plain"}
        )
        assert write_response.status_code in [200, 204], f"批量写入失败: {write_response.text}"
        
        time.sleep(1)
        
        query_response = requests.get(
            f"{chronodb_url}/api/v1/query",
            params={"query": unique_metric_name}
        )
        assert query_response.status_code == 200, f"查询失败: {query_response.text}"
        
        result = query_response.json()
        data = result.get("data", {}).get("result", [])
        assert len(data) > 0, "批量写入后查询结果为空"
    
    def test_labels_query(self, chronodb_url):
        """测试标签查询"""
        response = requests.get(f"{chronodb_url}/api/v1/labels")
        assert response.status_code == 200, f"标签查询失败: {response.text}"
        
        result = response.json()
        assert result.get("status") == "success", f"查询状态错误: {result}"
        
        labels = result.get("data", [])
        assert isinstance(labels, list), "标签列表格式错误"
    
    def test_query_range(self, chronodb_url, unique_metric_name):
        """测试时间范围查询"""
        from utils.data_generator import DataGenerator
        
        generator = DataGenerator()
        lines = generator.generate_time_series(
            metric_name=unique_metric_name,
            labels={"job": "range_test", "instance": "server-1"},
            start_time=int(time.time() * 1000) - 3600000,
            end_time=int(time.time() * 1000),
            interval=10000
        )
        
        batch_data = "\n".join(lines)
        requests.post(
            f"{chronodb_url}/api/v1/write",
            data=batch_data,
            headers={"Content-Type": "text/plain"}
        )
        
        time.sleep(1)
        
        end_ts = int(time.time())
        start_ts = end_ts - 3600
        
        response = requests.get(
            f"{chronodb_url}/api/v1/query_range",
            params={
                "query": unique_metric_name,
                "start": start_ts,
                "end": end_ts,
                "step": "15s"
            }
        )
        assert response.status_code == 200, f"时间范围查询失败: {response.text}"
        
        result = response.json()
        assert result.get("status") == "success", f"查询状态错误: {result}"
