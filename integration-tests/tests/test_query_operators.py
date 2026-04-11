"""
查询算子测试
测试 ChronoDB 的 PromQL 查询算子
"""
import pytest
import requests
import time
from utils.data_generator import DataGenerator


class TestQueryOperators:
    """查询算子测试类"""
    
    @pytest.fixture(autouse=True)
    def setup_data(self, chronodb_url, unique_metric_name):
        """设置测试数据"""
        self.chronodb_url = chronodb_url
        self.metric_name = unique_metric_name
        
        generator = DataGenerator()
        for i in range(3):
            lines = generator.generate_time_series(
                metric_name=self.metric_name,
                labels={"job": f"job_{i}", "instance": f"instance_{i}"},
                start_time=int(time.time() * 1000) - 60000,
                end_time=int(time.time() * 1000),
                interval=1000
            )
            batch_data = "\n".join(lines)
            requests.post(
                f"{self.chronodb_url}/api/v1/write",
                data=batch_data,
                headers={"Content-Type": "text/plain"}
            )
        
        time.sleep(1)
    
    def test_sum_aggregation(self):
        """测试 sum 聚合"""
        response = requests.get(
            f"{self.chronodb_url}/api/v1/query",
            params={"query": f"sum({self.metric_name})"}
        )
        assert response.status_code == 200
        result = response.json()
        assert result.get("status") == "success"
    
    def test_avg_aggregation(self):
        """测试 avg 聚合"""
        response = requests.get(
            f"{self.chronodb_url}/api/v1/query",
            params={"query": f"avg({self.metric_name})"}
        )
        assert response.status_code == 200
        result = response.json()
        assert result.get("status") == "success"
    
    def test_min_aggregation(self):
        """测试 min 聚合"""
        response = requests.get(
            f"{self.chronodb_url}/api/v1/query",
            params={"query": f"min({self.metric_name})"}
        )
        assert response.status_code == 200
        result = response.json()
        assert result.get("status") == "success"
    
    def test_max_aggregation(self):
        """测试 max 聚合"""
        response = requests.get(
            f"{self.chronodb_url}/api/v1/query",
            params={"query": f"max({self.metric_name})"}
        )
        assert response.status_code == 200
        result = response.json()
        assert result.get("status") == "success"
    
    def test_count_aggregation(self):
        """测试 count 聚合"""
        response = requests.get(
            f"{self.chronodb_url}/api/v1/query",
            params={"query": f"count({self.metric_name})"}
        )
        assert response.status_code == 200
        result = response.json()
        assert result.get("status") == "success"
    
    def test_arithmetic_operators(self):
        """测试算术运算符"""
        operators = [
            f"{self.metric_name} + 10",
            f"{self.metric_name} - 10",
            f"{self.metric_name} * 2",
            f"{self.metric_name} / 2",
        ]
        
        for expr in operators:
            response = requests.get(
                f"{self.chronodb_url}/api/v1/query",
                params={"query": expr}
            )
            assert response.status_code == 200, f"算术运算失败: {expr}"
    
    def test_comparison_operators(self):
        """测试比较运算符"""
        operators = [
            f"{self.metric_name} > 50",
            f"{self.metric_name} < 80",
            f"{self.metric_name} >= 30",
            f"{self.metric_name} <= 90",
        ]
        
        for expr in operators:
            response = requests.get(
                f"{self.chronodb_url}/api/v1/query",
                params={"query": expr}
            )
            assert response.status_code == 200, f"比较运算失败: {expr}"
    
    def test_label_filter(self):
        """测试标签过滤"""
        response = requests.get(
            f"{self.chronodb_url}/api/v1/query",
            params={"query": f'{self.metric_name}{{job="job_0"}}'}
        )
        assert response.status_code == 200
        result = response.json()
        assert result.get("status") == "success"
