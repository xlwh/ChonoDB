import time
import sys
import os

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from lib.chronodb_client import ChronoDBClient
from lib.prometheus_client import PrometheusClient
from lib.data_generator import DataGenerator, SMALL
from lib.report import ReportCollector, TestResult
from lib.docker_manager import DockerManager


CLUSTER_NODES = ["bench-chronodb-node1", "bench-chronodb-node2", "bench-chronodb-node3"]
NODE_PORTS = {
    "bench-chronodb-node1": 19090,
    "bench-chronodb-node2": 19093,
    "bench-chronodb-node3": 19095,
}


class FailoverTest:
    def __init__(
        self,
        chronodb: ChronoDBClient,
        prometheus: PrometheusClient,
        reporter: ReportCollector,
        docker_mgr: DockerManager,
    ):
        self.chronodb = chronodb
        self.prometheus = prometheus
        self.reporter = reporter
        self.docker = docker_mgr
        self.generator = DataGenerator(seed=99)

    def _add_result(self, name: str, passed: bool, duration_ms: float, detail: str = "", metrics: dict = None):
        self.reporter.add_result(TestResult(
            name=name, category="failover", passed=passed,
            duration_ms=duration_ms, detail=detail, metrics=metrics or {},
        ))

    def _get_node_client(self, node_name: str) -> ChronoDBClient:
        port = NODE_PORTS.get(node_name, 19090)
        return ChronoDBClient(base_url=f"http://localhost:{port}")

    def _write_test_data(self, count: int = 500) -> int:
        ts_ms = int(time.time() * 1000) - 300000
        lines = []
        for i in range(count):
            ts = ts_ms + i * 1000
            val = round(50.0 + i * 0.1, 4)
            job = f"fo-job-{i % 5}"
            instance = f"fo-inst-{i % 10}"
            lines.append(f'fo_test_metric{{job="{job}",instance="{instance}"}} {val} {ts}')

        ok, _ = self.chronodb.write_text(lines)
        return len(lines) if ok else 0

    def _verify_query_works(self, expr: str = "fo_test_metric") -> bool:
        ok, data = self.chronodb.query(expr)
        if not ok:
            return False
        results = data.get("data", {}).get("result", [])
        return len(results) > 0

    def test_single_node_stop_and_restart(self):
        print("  [FO] Test: Single node stop and restart...")
        start = time.time()

        written = self._write_test_data(500)
        time.sleep(2)

        ok_before = self._verify_query_works()

        target_node = CLUSTER_NODES[1]
        print(f"  [FO] Stopping node: {target_node}")
        stopped = self.docker.stop_container(target_node)
        if not stopped:
            duration = (time.time() - start) * 1000
            self._add_result("fo_single_node_stop_restart", False, duration,
                             detail=f"failed to stop {target_node}")
            return

        time.sleep(5)

        ok_during = self._verify_query_works()
        print(f"  [FO] Query during node down: {'OK' if ok_during else 'FAILED'}")

        print(f"  [FO] Restarting node: {target_node}")
        restarted = self.docker.start_container(target_node)
        if not restarted:
            duration = (time.time() - start) * 1000
            self._add_result("fo_single_node_stop_restart", False, duration,
                             detail=f"failed to restart {target_node}")
            return

        time.sleep(10)

        node_client = self._get_node_client(target_node)
        node_healthy = node_client.wait_ready(max_retries=30, interval=2.0)

        ok_after = self._verify_query_works()

        duration = (time.time() - start) * 1000
        passed = ok_before and ok_after and node_healthy
        self._add_result("fo_single_node_stop_restart", passed, duration,
                         detail=f"before={ok_before}, during={ok_during}, after={ok_after}, node_healthy={node_healthy}",
                         metrics={"query_before": ok_before, "query_during": ok_during,
                                  "query_after": ok_after, "node_healthy": node_healthy})

    def test_node_pause_and_unpause(self):
        print("  [FO] Test: Node pause and unpause...")
        start = time.time()

        written = self._write_test_data(300)
        time.sleep(2)

        ok_before = self._verify_query_works()

        target_node = CLUSTER_NODES[2]
        print(f"  [FO] Pausing node: {target_node}")
        paused = self.docker.pause_container(target_node)

        time.sleep(5)

        ok_during = self._verify_query_works()

        print(f"  [FO] Unpausing node: {target_node}")
        unpaused = self.docker.unpause_container(target_node)

        time.sleep(10)

        ok_after = self._verify_query_works()

        duration = (time.time() - start) * 1000
        passed = ok_before and ok_after
        self._add_result("fo_node_pause_unpause", passed, duration,
                         detail=f"before={ok_before}, during={ok_during}, after={ok_after}",
                         metrics={"query_before": ok_before, "query_during": ok_during, "query_after": ok_after})

    def test_kill_and_restart_node(self):
        print("  [FO] Test: Kill and restart node (hard failure)...")
        start = time.time()

        written = self._write_test_data(500)
        time.sleep(2)

        ok_before = self._verify_query_works()

        target_node = CLUSTER_NODES[1]
        print(f"  [FO] Killing node: {target_node}")
        killed = self.docker.kill_container(target_node)

        time.sleep(5)

        ok_during = self._verify_query_works()

        print(f"  [FO] Restarting killed node: {target_node}")
        restarted = self.docker.start_container(target_node)

        time.sleep(15)

        node_client = self._get_node_client(target_node)
        node_healthy = node_client.wait_ready(max_retries=30, interval=2.0)

        ok_after = self._verify_query_works()

        duration = (time.time() - start) * 1000
        passed = ok_before and ok_after and node_healthy
        self._add_result("fo_kill_and_restart", passed, duration,
                         detail=f"before={ok_before}, during={ok_during}, after={ok_after}, node_healthy={node_healthy}",
                         metrics={"query_before": ok_before, "query_during": ok_during,
                                  "query_after": ok_after, "node_healthy": node_healthy})

    def test_write_during_failure(self):
        print("  [FO] Test: Write during node failure...")
        start = time.time()

        written_before = self._write_test_data(200)
        time.sleep(2)

        target_node = CLUSTER_NODES[2]
        self.docker.stop_container(target_node)
        time.sleep(3)

        ts_ms = int(time.time() * 1000)
        lines = []
        for i in range(200):
            ts = ts_ms + i * 1000
            val = round(100.0 + i * 0.5, 4)
            lines.append(f'fo_during_fail{{job="fo-test",idx="{i}"}} {val} {ts}')

        ok_write, write_msg = self.chronodb.write_text(lines)
        write_count = len(lines) if ok_write else 0

        time.sleep(2)

        self.docker.start_container(target_node)
        time.sleep(10)

        node_client = self._get_node_client(target_node)
        node_healthy = node_client.wait_ready(max_retries=30, interval=2.0)

        ok_after = self._verify_query_works("fo_during_fail")

        duration = (time.time() - start) * 1000
        passed = ok_write and ok_after and node_healthy
        self._add_result("fo_write_during_failure", passed, duration,
                         detail=f"write_ok={ok_write}, query_after={ok_after}, node_healthy={node_healthy}",
                         metrics={"write_ok": ok_write, "query_after": ok_after, "node_healthy": node_healthy})

    def test_two_nodes_failure(self):
        print("  [FO] Test: Two nodes failure (minority available)...")
        start = time.time()

        written = self._write_test_data(300)
        time.sleep(2)

        ok_before = self._verify_query_works()

        node1 = CLUSTER_NODES[1]
        node2 = CLUSTER_NODES[2]
        print(f"  [FO] Stopping nodes: {node1}, {node2}")
        self.docker.stop_container(node1)
        self.docker.stop_container(node2)

        time.sleep(5)

        ok_during = self._verify_query_works()

        print(f"  [FO] Restarting nodes: {node1}, {node2}")
        self.docker.start_container(node1)
        self.docker.start_container(node2)

        time.sleep(15)

        c1 = self._get_node_client(node1)
        c2 = self._get_node_client(node2)
        n1_healthy = c1.wait_ready(max_retries=30, interval=2.0)
        n2_healthy = c2.wait_ready(max_retries=30, interval=2.0)

        ok_after = self._verify_query_works()

        duration = (time.time() - start) * 1000
        passed = ok_before and ok_after and n1_healthy and n2_healthy
        self._add_result("fo_two_nodes_failure", passed, duration,
                         detail=f"before={ok_before}, during={ok_during}, after={ok_after}, "
                                f"n1_healthy={n1_healthy}, n2_healthy={n2_healthy}",
                         metrics={"query_before": ok_before, "query_during": ok_during,
                                  "query_after": ok_after})

    def test_data_consistency_after_recovery(self):
        print("  [FO] Test: Data consistency after recovery...")
        start = time.time()

        ts_ms = int(time.time() * 1000) - 600000
        known_lines = []
        known_values = {}
        for i in range(50):
            ts = ts_ms + i * 10000
            val = round(200.0 + i * 1.5, 4)
            key = f"consistency_{i}"
            known_values[key] = val
            known_lines.append(f'fo_consistency{{job="fo-test",idx="{i}"}} {val} {ts}')

        ok, _ = self.chronodb.write_text(known_lines)
        time.sleep(2)

        ok_before, data_before = self.chronodb.query("fo_consistency")
        results_before = data_before.get("data", {}).get("result", []) if ok_before else []

        target_node = CLUSTER_NODES[1]
        self.docker.stop_container(target_node)
        time.sleep(5)
        self.docker.start_container(target_node)

        node_client = self._get_node_client(target_node)
        node_client.wait_ready(max_retries=30, interval=2.0)

        time.sleep(5)

        ok_after, data_after = self.chronodb.query("fo_consistency")
        results_after = data_after.get("data", {}).get("result", []) if ok_after else []

        consistent = ok_before and ok_after and len(results_after) >= len(results_before) * 0.8

        duration = (time.time() - start) * 1000
        self._add_result("fo_data_consistency_after_recovery", consistent, duration,
                         detail=f"before_series={len(results_before)}, after_series={len(results_after)}",
                         metrics={"series_before": len(results_before), "series_after": len(results_after)})

    def test_repeated_failures(self):
        print("  [FO] Test: Repeated node failures...")
        start = time.time()

        written = self._write_test_data(500)
        time.sleep(2)

        ok_before = self._verify_query_works()

        target_node = CLUSTER_NODES[2]
        success_count = 0
        total_rounds = 3

        for round_idx in range(total_rounds):
            self.docker.stop_container(target_node)
            time.sleep(3)
            self.docker.start_container(target_node)
            time.sleep(10)

            node_client = self._get_node_client(target_node)
            node_healthy = node_client.wait_ready(max_retries=20, interval=2.0)

            ok = self._verify_query_works()
            if ok and node_healthy:
                success_count += 1

        duration = (time.time() - start) * 1000
        passed = ok_before and success_count >= 2
        self._add_result("fo_repeated_failures", passed, duration,
                         detail=f"success_rounds={success_count}/{total_rounds}",
                         metrics={"success_rounds": success_count, "total_rounds": total_rounds})

    def run_all(self):
        print("\n  [Failover Tests] Starting...")
        self.test_single_node_stop_and_restart()
        self.test_node_pause_and_unpause()
        self.test_kill_and_restart_node()
        self.test_write_during_failure()
        self.test_two_nodes_failure()
        self.test_data_consistency_after_recovery()
        self.test_repeated_failures()
        print("  [Failover Tests] Completed")
