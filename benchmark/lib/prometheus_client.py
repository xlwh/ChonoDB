import requests
import time
from typing import Optional, Dict, List, Tuple


class PrometheusClient:
    def __init__(self, base_url: str = "http://localhost:19092", timeout: int = 60):
        self.base_url = base_url.rstrip("/")
        self.timeout = timeout

    def health_check(self) -> bool:
        try:
            resp = requests.get(f"{self.base_url}/-/healthy", timeout=5)
            return resp.status_code == 200
        except Exception:
            return False

    def ready_check(self) -> bool:
        try:
            resp = requests.get(f"{self.base_url}/-/ready", timeout=5)
            return resp.status_code == 200
        except Exception:
            return False

    def wait_ready(self, max_retries: int = 60, interval: float = 2.0) -> bool:
        for i in range(max_retries):
            if self.health_check() and self.ready_check():
                return True
            time.sleep(interval)
        return False

    def write_text(self, lines: List[str]) -> Tuple[bool, str]:
        data = "\n".join(lines)
        try:
            resp = requests.post(
                f"{self.base_url}/api/v1/write",
                data=data,
                timeout=self.timeout,
                headers={"Content-Type": "text/plain"},
            )
            return resp.status_code in [200, 204], resp.text[:500]
        except Exception as e:
            return False, str(e)

    def query(self, expr: str, ts: Optional[float] = None) -> Tuple[bool, Dict]:
        params = {"query": expr}
        if ts is not None:
            params["time"] = ts
        try:
            resp = requests.get(
                f"{self.base_url}/api/v1/query", params=params, timeout=self.timeout
            )
            if resp.status_code == 200:
                data = resp.json()
                return data.get("status") == "success", data
            return False, {"error": f"HTTP {resp.status_code}: {resp.text[:300]}"}
        except Exception as e:
            return False, {"error": str(e)}

    def query_range(
        self,
        expr: str,
        start: float,
        end: float,
        step: str = "15s",
    ) -> Tuple[bool, Dict]:
        params = {"query": expr, "start": start, "end": end, "step": step}
        try:
            resp = requests.get(
                f"{self.base_url}/api/v1/query_range",
                params=params,
                timeout=self.timeout,
            )
            if resp.status_code == 200:
                data = resp.json()
                return data.get("status") == "success", data
            return False, {"error": f"HTTP {resp.status_code}: {resp.text[:300]}"}
        except Exception as e:
            return False, {"error": str(e)}

    def labels(self) -> Tuple[bool, List[str]]:
        try:
            resp = requests.get(
                f"{self.base_url}/api/v1/labels", timeout=self.timeout
            )
            if resp.status_code == 200:
                data = resp.json()
                return True, data.get("data", [])
            return False, []
        except Exception:
            return False, []

    def label_values(self, label_name: str) -> Tuple[bool, List[str]]:
        try:
            resp = requests.get(
                f"{self.base_url}/api/v1/label/{label_name}/values",
                timeout=self.timeout,
            )
            if resp.status_code == 200:
                data = resp.json()
                return True, data.get("data", [])
            return False, []
        except Exception:
            return False, []

    def series(self, match: List[str]) -> Tuple[bool, List[Dict]]:
        params = [("match[]", m) for m in match]
        try:
            resp = requests.get(
                f"{self.base_url}/api/v1/series", params=params, timeout=self.timeout
            )
            if resp.status_code == 200:
                data = resp.json()
                return True, data.get("data", [])
            return False, []
        except Exception:
            return False, []
