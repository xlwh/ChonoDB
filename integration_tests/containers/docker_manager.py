#!/usr/bin/env python3
"""
Docker容器管理模块
管理Prometheus和ChronoDB容器的生命周期
"""

import os
import time
import json
import subprocess
import tempfile
import shutil
from typing import Dict, List, Optional, Tuple, Any
from pathlib import Path
from dataclasses import dataclass

import sys
sys.path.insert(0, str(Path(__file__).parent.parent))
from core.logger import get_logger
from core.config import get_config


@dataclass
class ContainerInfo:
    """容器信息"""
    name: str
    container_id: Optional[str] = None
    status: str = "unknown"
    port: int = 0
    host: str = "localhost"
    logs_path: Optional[str] = None
    data_dir: Optional[str] = None
    
    @property
    def url(self) -> str:
        return f"http://{self.host}:{self.port}"


class DockerContainerManager:
    """Docker容器管理器"""
    
    def __init__(self):
        self.logger = get_logger()
        self.config = get_config()
        self.containers: Dict[str, ContainerInfo] = {}
        self.network_name = self.config.container_config.network_name
        self._check_docker()
    
    def _check_docker(self):
        """检查Docker是否可用"""
        try:
            result = subprocess.run(
                ["docker", "version"],
                capture_output=True,
                text=True,
                timeout=10
            )
            if result.returncode != 0:
                raise RuntimeError("Docker 守护进程未运行")
        except FileNotFoundError:
            raise RuntimeError("Docker 未安装")
        except subprocess.TimeoutExpired:
            raise RuntimeError("Docker 检查超时")
    
    def _run_docker_command(self, cmd: List[str], timeout: int = 60) -> Tuple[int, str, str]:
        """运行Docker命令"""
        try:
            result = subprocess.run(
                ["docker"] + cmd,
                capture_output=True,
                text=True,
                timeout=timeout
            )
            return result.returncode, result.stdout, result.stderr
        except subprocess.TimeoutExpired:
            return -1, "", "命令超时"
        except Exception as e:
            return -1, "", str(e)
    
    def create_network(self):
        """创建Docker网络"""
        self.logger.info(f"创建Docker网络: {self.network_name}")
        
        # 检查网络是否已存在
        rc, stdout, stderr = self._run_docker_command(
            ["network", "ls", "--filter", f"name={self.network_name}", "--format", "{{.Name}}"]
        )
        
        if self.network_name in stdout:
            self.logger.info(f"网络 {self.network_name} 已存在")
            return True
        
        # 创建网络
        rc, stdout, stderr = self._run_docker_command(
            ["network", "create", self.network_name]
        )
        
        if rc == 0:
            self.logger.info(f"网络创建成功: {stdout.strip()}")
            return True
        else:
            self.logger.error(f"网络创建失败: {stderr}")
            return False
    
    def remove_network(self):
        """删除Docker网络"""
        self.logger.info(f"删除Docker网络: {self.network_name}")
        
        rc, stdout, stderr = self._run_docker_command(
            ["network", "rm", self.network_name]
        )
        
        if rc == 0:
            self.logger.info("网络删除成功")
            return True
        else:
            self.logger.warning(f"网络删除失败: {stderr}")
            return False
    
    def pull_image(self, image: str) -> bool:
        """拉取Docker镜像"""
        self.logger.info(f"拉取镜像: {image}")
        
        rc, stdout, stderr = self._run_docker_command(
            ["pull", image],
            timeout=300
        )
        
        if rc == 0:
            self.logger.info(f"镜像拉取成功: {image}")
            return True
        else:
            self.logger.error(f"镜像拉取失败: {stderr}")
            return False
    
    def start_prometheus(self, name: str = "prometheus-test", 
                        port: int = None,
                        config_content: str = None) -> Optional[ContainerInfo]:
        """启动Prometheus容器"""
        port = port or self.config.container_config.prometheus_port
        image = self.config.container_config.prometheus_image
        
        self.logger.info(f"启动Prometheus容器: {name} (端口: {port})")
        
        # 创建临时目录
        temp_dir = tempfile.mkdtemp(prefix=f"prometheus_{name}_")
        config_dir = os.path.join(temp_dir, "config")
        data_dir = os.path.join(temp_dir, "data")
        os.makedirs(config_dir, exist_ok=True)
        os.makedirs(data_dir, exist_ok=True)
        
        # 写入配置文件
        if config_content:
            config_path = os.path.join(config_dir, "prometheus.yml")
            with open(config_path, 'w') as f:
                f.write(config_content)
        else:
            # 使用默认配置
            config_path = self._create_default_prometheus_config(config_dir)
        
        # 启动容器
        cmd = [
            "run", "-d",
            "--name", name,
            "--network", self.network_name,
            "-p", f"{port}:9090",
            "-v", f"{config_path}:/etc/prometheus/prometheus.yml:ro",
            "-v", f"{data_dir}:/prometheus",
            "--restart", "unless-stopped",
            image,
            "--config.file=/etc/prometheus/prometheus.yml",
            "--storage.tsdb.path=/prometheus",
            "--web.enable-lifecycle"
        ]
        
        rc, stdout, stderr = self._run_docker_command(cmd, timeout=60)
        
        if rc != 0:
            self.logger.error(f"Prometheus容器启动失败: {stderr}")
            shutil.rmtree(temp_dir, ignore_errors=True)
            return None
        
        container_id = stdout.strip()
        
        # 等待服务就绪
        if not self._wait_for_healthy(name, timeout=60):
            self.logger.error("Prometheus服务启动超时")
            self.stop_container(name)
            shutil.rmtree(temp_dir, ignore_errors=True)
            return None
        
        info = ContainerInfo(
            name=name,
            container_id=container_id,
            status="running",
            port=port,
            host="localhost",
            logs_path=temp_dir,
            data_dir=data_dir
        )
        
        self.containers[name] = info
        self.logger.info(f"Prometheus容器启动成功: {container_id[:12]}")
        
        return info
    
    def start_chronodb(self, name: str = "chronodb-test",
                      port: int = None,
                      mode: str = "standalone",
                      config_content: str = None) -> Optional[ContainerInfo]:
        """启动ChronoDB容器"""
        port = port or self.config.container_config.chronodb_port
        image = self.config.container_config.chronodb_image
        
        self.logger.info(f"启动ChronoDB容器: {name} (模式: {mode}, 端口: {port})")
        
        # 创建临时目录
        temp_dir = tempfile.mkdtemp(prefix=f"chronodb_{name}_")
        data_dir = os.path.join(temp_dir, "data")
        config_dir = os.path.join(temp_dir, "config")
        os.makedirs(data_dir, exist_ok=True)
        os.makedirs(config_dir, exist_ok=True)
        
        # 写入配置文件
        if config_content:
            config_path = os.path.join(config_dir, "chronodb.yaml")
            with open(config_path, 'w') as f:
                f.write(config_content)
        else:
            config_path = self._create_default_chronodb_config(config_dir, mode)
        
        # 启动容器 - 使用本地已构建的镜像或从Dockerfile构建
        # 首先尝试使用本地镜像
        cmd = [
            "run", "-d",
            "--name", name,
            "--network", self.network_name,
            "-p", f"{port}:9090",
            "-v", f"{config_path}:/etc/chronodb/chronodb.yaml:ro",
            "-v", f"{data_dir}:/var/lib/chronodb",
            "-e", "RUST_LOG=info",
            image
        ]
        
        rc, stdout, stderr = self._run_docker_command(cmd, timeout=60)
        
        if rc != 0:
            # 尝试从本地Dockerfile构建
            self.logger.info("尝试从本地Dockerfile构建ChronoDB镜像...")
            if self._build_chronodb_image():
                rc, stdout, stderr = self._run_docker_command(cmd, timeout=60)
            
            if rc != 0:
                self.logger.error(f"ChronoDB容器启动失败: {stderr}")
                shutil.rmtree(temp_dir, ignore_errors=True)
                return None
        
        container_id = stdout.strip()
        
        # 等待服务就绪
        if not self._wait_for_healthy(name, timeout=60):
            self.logger.error("ChronoDB服务启动超时")
            self.stop_container(name)
            shutil.rmtree(temp_dir, ignore_errors=True)
            return None
        
        info = ContainerInfo(
            name=name,
            container_id=container_id,
            status="running",
            port=port,
            host="localhost",
            logs_path=temp_dir,
            data_dir=data_dir
        )
        
        self.containers[name] = info
        self.logger.info(f"ChronoDB容器启动成功: {container_id[:12]}")
        
        return info
    
    def _build_chronodb_image(self) -> bool:
        """从本地Dockerfile构建ChronoDB镜像"""
        project_root = Path(__file__).parent.parent.parent
        dockerfile_path = project_root / "Dockerfile"
        
        if not dockerfile_path.exists():
            self.logger.error(f"Dockerfile不存在: {dockerfile_path}")
            return False
        
        self.logger.info("构建ChronoDB镜像...")
        
        rc, stdout, stderr = self._run_docker_command(
            ["build", "-t", self.config.container_config.chronodb_image, "."],
            timeout=600
        )
        
        if rc == 0:
            self.logger.info("ChronoDB镜像构建成功")
            return True
        else:
            self.logger.error(f"ChronoDB镜像构建失败: {stderr}")
            return False
    
    def stop_container(self, name: str) -> bool:
        """停止并删除容器"""
        self.logger.info(f"停止容器: {name}")
        
        # 停止容器
        rc, stdout, stderr = self._run_docker_command(
            ["stop", "-t", "10", name],
            timeout=30
        )
        
        # 删除容器
        rc, stdout, stderr = self._run_docker_command(
            ["rm", "-v", name],
            timeout=30
        )
        
        # 清理临时目录
        if name in self.containers:
            info = self.containers[name]
            if info.logs_path and os.path.exists(info.logs_path):
                shutil.rmtree(info.logs_path, ignore_errors=True)
            del self.containers[name]
        
        self.logger.info(f"容器已停止: {name}")
        return True
    
    def stop_all_containers(self):
        """停止所有容器"""
        self.logger.info("停止所有容器")
        
        for name in list(self.containers.keys()):
            self.stop_container(name)
    
    def restart_container(self, name: str) -> bool:
        """重启容器"""
        self.logger.info(f"重启容器: {name}")
        
        rc, stdout, stderr = self._run_docker_command(
            ["restart", name],
            timeout=60
        )
        
        if rc == 0:
            # 等待服务就绪
            if self._wait_for_healthy(name, timeout=60):
                self.logger.info(f"容器重启成功: {name}")
                return True
        
        self.logger.error(f"容器重启失败: {stderr}")
        return False
    
    def kill_container(self, name: str) -> bool:
        """强制停止容器（模拟故障）"""
        self.logger.info(f"强制停止容器: {name}")
        
        rc, stdout, stderr = self._run_docker_command(
            ["kill", name],
            timeout=10
        )
        
        if rc == 0:
            if name in self.containers:
                self.containers[name].status = "killed"
            return True
        
        return False
    
    def pause_container(self, name: str) -> bool:
        """暂停容器"""
        self.logger.info(f"暂停容器: {name}")
        
        rc, stdout, stderr = self._run_docker_command(
            ["pause", name],
            timeout=10
        )
        
        if rc == 0:
            if name in self.containers:
                self.containers[name].status = "paused"
            return True
        
        return False
    
    def unpause_container(self, name: str) -> bool:
        """恢复容器"""
        self.logger.info(f"恢复容器: {name}")
        
        rc, stdout, stderr = self._run_docker_command(
            ["unpause", name],
            timeout=10
        )
        
        if rc == 0:
            if name in self.containers:
                self.containers[name].status = "running"
            return True
        
        return False
    
    def get_container_logs(self, name: str, tail: int = 100) -> str:
        """获取容器日志"""
        rc, stdout, stderr = self._run_docker_command(
            ["logs", "--tail", str(tail), name],
            timeout=30
        )
        
        if rc == 0:
            return stdout
        return stderr
    
    def get_container_stats(self, name: str) -> Dict[str, Any]:
        """获取容器统计信息"""
        rc, stdout, stderr = self._run_docker_command(
            ["stats", "--no-stream", "--format", "json", name],
            timeout=30
        )
        
        if rc == 0:
            try:
                return json.loads(stdout)
            except json.JSONDecodeError:
                pass
        
        return {}
    
    def is_container_running(self, name: str) -> bool:
        """检查容器是否运行中"""
        rc, stdout, stderr = self._run_docker_command(
            ["inspect", "-f", "{{.State.Running}}", name],
            timeout=10
        )
        
        return rc == 0 and stdout.strip() == "true"
    
    def _wait_for_healthy(self, name: str, timeout: int = 60) -> bool:
        """等待容器健康"""
        start_time = time.time()
        
        while time.time() - start_time < timeout:
            # 检查容器是否运行
            if not self.is_container_running(name):
                time.sleep(1)
                continue
            
            # 检查健康端点
            if name in self.containers:
                info = self.containers[name]
                import requests
                try:
                    response = requests.get(
                        f"{info.url}/-/healthy",
                        timeout=2
                    )
                    if response.status_code == 200:
                        return True
                except Exception:
                    pass
            
            time.sleep(1)
        
        return False
    
    def _create_default_prometheus_config(self, config_dir: str) -> str:
        """创建默认Prometheus配置"""
        config_content = """
global:
  scrape_interval: 15s
  evaluation_interval: 15s

scrape_configs:
  - job_name: 'prometheus'
    static_configs:
      - targets: ['localhost:9090']
"""
        config_path = os.path.join(config_dir, "prometheus.yml")
        with open(config_path, 'w') as f:
            f.write(config_content)
        
        return config_path
    
    def _create_default_chronodb_config(self, config_dir: str, mode: str) -> str:
        """创建默认ChronoDB配置"""
        config_content = f"""
listen_address: "0.0.0.0"
port: 9090
data_dir: "/var/lib/chronodb"

storage:
  mode: "{mode}"
  backend: "local"
  local_path: "/var/lib/chronodb/data"

query:
  max_concurrent: 100
  timeout: 120
  max_samples: 50000000
  enable_vectorized: true
  enable_parallel: true

memory:
  memstore_size: "1GB"
  wal_size: "256MB"
  max_memory_usage: "80%"

log:
  level: "info"
  format: "text"
"""
        config_path = os.path.join(config_dir, "chronodb.yaml")
        with open(config_path, 'w') as f:
            f.write(config_content)
        
        return config_path
    
    def cleanup(self):
        """清理所有资源"""
        self.logger.info("清理Docker资源")
        self.stop_all_containers()
        self.remove_network()


# 便捷函数
def create_container_manager() -> DockerContainerManager:
    """创建容器管理器"""
    return DockerContainerManager()
