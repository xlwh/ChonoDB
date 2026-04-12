import subprocess
import time
import os
import sys
from typing import Optional, List


class DockerManager:
    def __init__(self, compose_file: str, project_name: str = "bench"):
        self.compose_file = os.path.abspath(compose_file)
        self.project_name = project_name
        self._docker_cmd = self._detect_docker_cmd()

    def _detect_docker_cmd(self) -> List[str]:
        result = subprocess.run(
            ["docker", "info"], capture_output=True, text=True, timeout=10,
        )
        if result.returncode == 0:
            return ["docker"]
        result = subprocess.run(
            ["sudo", "docker", "info"], capture_output=True, text=True, timeout=10,
        )
        if result.returncode == 0:
            return ["sudo", "docker"]
        return ["docker"]

    def _run_compose(self, args: List[str], capture: bool = True, timeout: int = 1800) -> subprocess.CompletedProcess:
        cmd = self._docker_cmd + [
            "compose",
            "-f", self.compose_file,
            "-p", self.project_name,
        ] + args
        result = subprocess.run(
            cmd,
            capture_output=capture,
            text=True,
            timeout=timeout,
        )
        return result

    def build(self) -> bool:
        print("  Building Docker images...")
        result = self._run_compose(["build", "--no-cache"], timeout=3600)
        if result.returncode != 0:
            print(f"  Build failed: {result.stderr[-500:]}")
            return False
        print("  Build completed")
        return True

    def start(self) -> bool:
        print("  Starting containers...")
        result = self._run_compose(["up", "-d", "--remove-orphans"])
        if result.returncode != 0:
            print(f"  Start failed: {result.stderr[-500:]}")
            return False
        print("  Containers started")
        return True

    def stop(self) -> bool:
        print("  Stopping containers...")
        result = self._run_compose(["down", "-v", "--remove-orphans"])
        if result.returncode != 0:
            print(f"  Stop failed: {result.stderr[-500:]}")
            return False
        print("  Containers stopped and volumes removed")
        return True

    def pause_container(self, container_name: str) -> bool:
        result = subprocess.run(
            self._docker_cmd + ["pause", container_name],
            capture_output=True, text=True, timeout=30,
        )
        return result.returncode == 0

    def unpause_container(self, container_name: str) -> bool:
        result = subprocess.run(
            self._docker_cmd + ["unpause", container_name],
            capture_output=True, text=True, timeout=30,
        )
        return result.returncode == 0

    def stop_container(self, container_name: str) -> bool:
        result = subprocess.run(
            self._docker_cmd + ["stop", container_name],
            capture_output=True, text=True, timeout=30,
        )
        return result.returncode == 0

    def start_container(self, container_name: str) -> bool:
        result = subprocess.run(
            self._docker_cmd + ["start", container_name],
            capture_output=True, text=True, timeout=30,
        )
        return result.returncode == 0

    def kill_container(self, container_name: str) -> bool:
        result = subprocess.run(
            self._docker_cmd + ["kill", container_name],
            capture_output=True, text=True, timeout=30,
        )
        return result.returncode == 0

    def restart_container(self, container_name: str) -> bool:
        result = subprocess.run(
            self._docker_cmd + ["restart", container_name],
            capture_output=True, text=True, timeout=60,
        )
        return result.returncode == 0

    def container_logs(self, container_name: str, tail: int = 50) -> str:
        result = subprocess.run(
            self._docker_cmd + ["logs", "--tail", str(tail), container_name],
            capture_output=True, text=True, timeout=30,
        )
        return result.stdout + result.stderr

    def container_is_running(self, container_name: str) -> bool:
        result = subprocess.run(
            self._docker_cmd + ["inspect", "-f", "{{.State.Running}}", container_name],
            capture_output=True, text=True, timeout=10,
        )
        return result.stdout.strip() == "true"

    def wait_container_healthy(self, container_name: str, max_retries: int = 60, interval: float = 3.0) -> bool:
        for i in range(max_retries):
            result = subprocess.run(
                self._docker_cmd + ["inspect", "-f", "{{.State.Health.Status}}", container_name],
                capture_output=True, text=True, timeout=10,
            )
            status = result.stdout.strip()
            if status == "healthy":
                return True
            if status == "unhealthy":
                time.sleep(interval)
                continue
            if self.container_is_running(container_name):
                time.sleep(interval)
                continue
            return False
        return False
