"""
pytest 配置和 fixtures
"""
import pytest
import requests
import time
from typing import Dict


@pytest.fixture(scope="session")
def chronodb_url():
    """ChronoDB 服务 URL"""
    return "http://localhost:9090"


@pytest.fixture(scope="session")
def prometheus_url():
    """Prometheus 服务 URL"""
    return "http://localhost:9092"


@pytest.fixture(scope="session")
def monitoring_prometheus_url():
    """监控 Prometheus 服务 URL"""
    return "http://localhost:9093"


@pytest.fixture(scope="session")
def check_services(chronodb_url, prometheus_url, monitoring_prometheus_url):
    """检查所有服务是否就绪"""
    services = {
        "ChronoDB": chronodb_url,
        "Prometheus": prometheus_url,
        "Monitoring Prometheus": monitoring_prometheus_url
    }
    
    for name, url in services.items():
        max_retries = 10
        for i in range(max_retries):
            try:
                response = requests.get(f"{url}/-/healthy", timeout=2)
                if response.status_code == 200:
                    print(f"✓ {name} 服务就绪")
                    break
            except Exception as e:
                if i == max_retries - 1:
                    pytest.fail(f"{name} 服务未就绪: {e}")
                time.sleep(2)


@pytest.fixture
def unique_metric_name():
    """生成唯一的指标名称"""
    import uuid
    return f"test_metric_{uuid.uuid4().hex[:8]}"


@pytest.fixture
def test_timestamp():
    """生成测试时间戳（毫秒）"""
    return int(time.time() * 1000)
