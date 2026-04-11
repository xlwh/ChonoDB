"""
Docker 环境管理工具
"""
import subprocess
import time
from typing import Optional, List


class DockerManager:
    """Docker 环境管理器"""
    
    def __init__(self, compose_file: str = "docker/docker-compose.test.yml"):
        self.compose_file = compose_file
    
    def start_services(self, services: Optional[List[str]] = None):
        """启动服务"""
        cmd = ["docker-compose", "-f", self.compose_file, "up", "-d"]
        if services:
            cmd.extend(services)
        
        result = subprocess.run(cmd, capture_output=True, text=True)
        if result.returncode != 0:
            raise Exception(f"启动服务失败: {result.stderr}")
        
        return True
    
    def stop_services(self, services: Optional[List[str]] = None):
        """停止服务"""
        cmd = ["docker-compose", "-f", self.compose_file, "stop"]
        if services:
            cmd.extend(services)
        
        result = subprocess.run(cmd, capture_output=True, text=True)
        if result.returncode != 0:
            raise Exception(f"停止服务失败: {result.stderr}")
        
        return True
    
    def restart_service(self, service: str):
        """重启服务"""
        cmd = ["docker-compose", "-f", self.compose_file, "restart", service]
        
        result = subprocess.run(cmd, capture_output=True, text=True)
        if result.returncode != 0:
            raise Exception(f"重启服务失败: {result.stderr}")
        
        return True
    
    def get_service_logs(self, service: str, lines: int = 100) -> str:
        """获取服务日志"""
        cmd = ["docker-compose", "-f", self.compose_file, "logs", "--tail", str(lines), service]
        
        result = subprocess.run(cmd, capture_output=True, text=True)
        return result.stdout
    
    def check_service_health(self, service: str, max_retries: int = 30) -> bool:
        """检查服务健康状态"""
        cmd = ["docker-compose", "-f", self.compose_file, "ps", service]
        
        for i in range(max_retries):
            result = subprocess.run(cmd, capture_output=True, text=True)
            if "healthy" in result.stdout or "running" in result.stdout:
                return True
            time.sleep(2)
        
        return False
    
    def cleanup(self):
        """清理环境"""
        cmd = ["docker-compose", "-f", self.compose_file, "down", "-v"]
        result = subprocess.run(cmd, capture_output=True, text=True)
        
        if result.returncode != 0:
            raise Exception(f"清理环境失败: {result.stderr}")
        
        return True
