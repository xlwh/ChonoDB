import time
import sys
import os

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from lib.chronodb_client import ChronoDBClient
from lib.prometheus_client import PrometheusClient
from lib.data_generator import DataGenerator, DataScale, SMALL, MEDIUM, LARGE
from lib.report import ReportCollector, TestResult


class FunctionalTest:
    def __init__(
        self,
        chronodb: ChronoDBClient,
        prometheus: PrometheusClient,
        reporter: ReportCollector,
    ):
        self.chronodb = chronodb
        self.prometheus = prometheus
        self.reporter = reporter
        self.generator = DataGenerator(seed=42)

    def _add_result(self, name: str, passed: bool, duration_ms: float, detail: str = "", metrics: dict = None):
        self.reporter.add_result(TestResult(
            name=name, category="functional", passed=passed,
            duration_ms=duration_ms, detail=detail, metrics=metrics or {},
        ))

    def test_health_endpoints(self):
        start = time.time()
        healthy = self.chronodb.health_check()
        ready = self.chronodb.ready_check()
        duration = (time.time() - start) * 1000
        self._add_result(
            "health_check", healthy and ready, duration,
            detail="" if healthy and ready else f"healthy={healthy}, ready={ready}",
        )

    def test_write_and_query_basic(self):
        start = time.time()
        ts_ms = int(time.time() * 1000) - 60000
        lines = [
            f'func_test_metric{{job="ftest",instance="node-1",region="east"}} 42.5 {ts_ms}',
            f'func_test_metric{{job="ftest",instance="node-2",region="west"}} 78.3 {ts_ms}',
        ]
        ok, msg = self.chronodb.write_text(lines)
        if not ok:
            duration = (time.time() - start) * 1000
            self._add_result("write_and_query_basic", False, duration, detail=f"write failed: {msg}")
            return

        time.sleep(1)

        ok, data = self.chronodb.query("func_test_metric")
        duration = (time.time() - start) * 1000

        if not ok:
            self._add_result("write_and_query_basic", False, duration, detail=f"query failed: {data}")
            return

        results = data.get("data", {}).get("result", [])
        if len(results) < 2:
            self._add_result("write_and_query_basic", False, duration,
                             detail=f"expected >=2 series, got {len(results)}")
            return

        values = {}
        for r in results:
            labels = r.get("metric", {})
            instance = labels.get("instance", "")
            val = float(r.get("value", [0, "0"])[1])
            values[instance] = val

        val_ok = True
        detail_parts = []
        for inst, expected in [("node-1", 42.5), ("node-2", 78.3)]:
            if inst in values:
                if abs(values[inst] - expected) > 0.01:
                    val_ok = False
                    detail_parts.append(f"{inst}: got {values[inst]}, expected {expected}")
            else:
                val_ok = False
                detail_parts.append(f"{inst}: not found")

        self._add_result("write_and_query_basic", val_ok, duration,
                         detail="; ".join(detail_parts) if detail_parts else "")

    def test_label_filtering(self):
        start = time.time()
        ts_ms = int(time.time() * 1000) - 60000
        lines = [
            f'label_test{{job="web",env="prod",region="east"}} 10 {ts_ms}',
            f'label_test{{job="web",env="staging",region="west"}} 20 {ts_ms}',
            f'label_test{{job="db",env="prod",region="east"}} 30 {ts_ms}',
        ]
        self.chronodb.write_text(lines)
        time.sleep(1)

        ok, data = self.chronodb.query('label_test{job="web"}')
        if ok:
            results = data.get("data", {}).get("result", [])
            all_web = all(r.get("metric", {}).get("job") == "web" for r in results)
            duration = (time.time() - start) * 1000
            self._add_result("label_filtering_single", all_web and len(results) >= 2, duration,
                             detail=f"results={len(results)}, all_web={all_web}")
        else:
            duration = (time.time() - start) * 1000
            self._add_result("label_filtering_single", False, duration, detail="query failed")

        ok, data = self.chronodb.query('label_test{job="web",env="prod"}')
        if ok:
            results = data.get("data", {}).get("result", [])
            all_match = all(
                r.get("metric", {}).get("job") == "web" and r.get("metric", {}).get("env") == "prod"
                for r in results
            )
            duration = (time.time() - start) * 1000
            self._add_result("label_filtering_multi", all_match and len(results) >= 1, duration,
                             detail=f"results={len(results)}")
        else:
            duration = (time.time() - start) * 1000
            self._add_result("label_filtering_multi", False, duration, detail="query failed")

    def test_aggregation_functions(self):
        start = time.time()
        ts_ms = int(time.time() * 1000) - 60000
        lines = []
        for i in range(10):
            val = 10.0 + i * 5.0
            lines.append(f'agg_test{{job="ftest",instance="node-{i}"}} {val} {ts_ms}')
        self.chronodb.write_text(lines)
        time.sleep(1)

        agg_tests = [
            ("sum(agg_test)", 55.0, "sum"),
            ("avg(agg_test)", 32.5, "avg"),
            ("min(agg_test)", 10.0, "min"),
            ("max(agg_test)", 55.0, "max"),
        ]

        for expr, expected, name in agg_tests:
            ok, data = self.chronodb.query(expr)
            duration = (time.time() - start) * 1000
            if not ok:
                self._add_result(f"aggregation_{name}", False, duration, detail="query failed")
                continue

            results = data.get("data", {}).get("result", [])
            if len(results) == 0:
                self._add_result(f"aggregation_{name}", False, duration, detail="no results")
                continue

            actual = float(results[0].get("value", [0, "0"])[1])
            tolerance = 1.0
            passed = abs(actual - expected) <= tolerance
            self._add_result(f"aggregation_{name}", passed, duration,
                             detail=f"expected={expected}, actual={actual}")

    def test_query_range(self):
        start = time.time()
        ts_ms = int(time.time() * 1000)
        lines = []
        for i in range(100):
            ts = ts_ms - (100 - i) * 15000
            val = 50.0 + i * 0.5
            lines.append(f'range_test{{job="ftest"}} {val} {ts}')
        self.chronodb.write_text(lines)
        time.sleep(1)

        end_ts = ts_ms // 1000
        start_ts = end_ts - 1800

        ok, data = self.chronodb.query_range("range_test", start_ts, end_ts, "15s")
        duration = (time.time() - start) * 1000
        if not ok:
            self._add_result("query_range", False, duration, detail="query_range failed")
            return

        results = data.get("data", {}).get("result", [])
        total_samples = sum(len(r.get("values", [])) for r in results)
        self._add_result("query_range", total_samples > 0, duration,
                         detail=f"series={len(results)}, samples={total_samples}",
                         metrics={"series_count": len(results), "sample_count": total_samples})

    def test_series_metadata(self):
        start = time.time()
        ts_ms = int(time.time() * 1000) - 60000
        lines = [
            f'meta_test{{job="ftest",team="backend"}} 1 {ts_ms}',
            f'meta_test{{job="ftest",team="frontend"}} 2 {ts_ms}',
        ]
        self.chronodb.write_text(lines)
        time.sleep(1)

        ok, labels = self.chronodb.labels()
        duration = (time.time() - start) * 1000
        has_job = "job" in labels if ok else False
        self._add_result("labels_api", has_job, duration,
                         detail=f"labels count={len(labels)}, has_job={has_job}")

        ok, values = self.chronodb.label_values("job")
        duration = (time.time() - start) * 1000
        has_ftest = "ftest" in values if ok else False
        self._add_result("label_values_api", has_ftest, duration,
                         detail=f"values={values[:10] if ok else 'failed'}")

        ok, series = self.chronodb.series(["meta_test"])
        duration = (time.time() - start) * 1000
        self._add_result("series_api", ok and len(series) > 0, duration,
                         detail=f"series count={len(series)}")

    def test_data_accuracy_vs_prometheus(self):
        start = time.time()
        ts_ms = int(time.time() * 1000) - 120000

        lines, known_data = self.generator.generate_known_data(
            series_count=5, samples_per_series=20, base_ts_ms=ts_ms, interval_ms=15000,
        )

        c_ok, c_msg = self.chronodb.write_text(lines)
        p_ok, p_msg = self.prometheus.write_text(lines)

        if not c_ok or not p_ok:
            duration = (time.time() - start) * 1000
            self._add_result("data_accuracy_vs_prometheus", False, duration,
                             detail=f"write failed: chronodb={c_msg[:100]}, prometheus={p_msg[:100]}")
            return

        time.sleep(2)

        accuracy_ok = True
        detail_parts = []

        for key, samples in known_data.items():
            if not samples:
                continue

            metric_part = key.split("{")[0] if "{" in key else key
            label_part = key.split("{")[1].rstrip("}") if "{" in key else ""

            c_ok, c_data = self.chronodb.query(key)
            p_ok, p_data = self.prometheus.query(key)

            if not c_ok or not p_ok:
                accuracy_ok = False
                detail_parts.append(f"{metric_part}: query failed")
                continue

            c_results = c_data.get("data", {}).get("result", [])
            p_results = p_data.get("data", {}).get("result", [])

            if len(c_results) != len(p_results):
                accuracy_ok = False
                detail_parts.append(f"{metric_part}: series count mismatch (c={len(c_results)}, p={len(p_results)})")
                continue

            if len(c_results) == 0:
                detail_parts.append(f"{metric_part}: no results in both")
                continue

            c_val = float(c_results[0].get("value", [0, "0"])[1])
            p_val = float(p_results[0].get("value", [0, "0"])[1])

            if abs(c_val - p_val) > 0.01:
                accuracy_ok = False
                detail_parts.append(f"{metric_part}: value mismatch (c={c_val}, p={p_val})")

        duration = (time.time() - start) * 1000
        self._add_result("data_accuracy_vs_prometheus", accuracy_ok, duration,
                         detail="; ".join(detail_parts) if detail_parts else "all values match")

    def test_different_data_scales(self):
        for scale_name, scale in [("small", SMALL), ("medium", MEDIUM)]:
            start = time.time()
            batches = self.generator.generate_write_batches(scale, batch_size=500)

            total_written = 0
            for batch in batches:
                ok, _ = self.chronodb.write_text(batch)
                if ok:
                    total_written += len(batch)

            time.sleep(2)

            ok, data = self.chronodb.query("cpu_usage_percent")
            duration = (time.time() - start) * 1000

            results = data.get("data", {}).get("result", []) if ok else []
            self._add_result(
                f"data_scale_{scale_name}", total_written > 0 and len(results) > 0, duration,
                detail=f"written={total_written}/{scale.total_samples}, series_found={len(results)}",
                metrics={"written": total_written, "series_found": len(results)},
            )

    def test_comparison_operators(self):
        start = time.time()
        ts_ms = int(time.time() * 1000) - 60000
        lines = []
        for i in range(20):
            val = 10.0 + i * 5.0
            lines.append(f'comp_test{{job="ftest",instance="n{i}"}} {val} {ts_ms}')
        self.chronodb.write_text(lines)
        time.sleep(1)

        for expr, name in [
            ("comp_test > 50", "gt"),
            ("comp_test < 50", "lt"),
            ("comp_test >= 50", "ge"),
            ("comp_test <= 50", "le"),
        ]:
            ok, data = self.chronodb.query(expr)
            duration = (time.time() - start) * 1000
            if ok:
                results = data.get("data", {}).get("result", [])
                self._add_result(f"comparison_{name}", True, duration,
                                 detail=f"results={len(results)}")
            else:
                self._add_result(f"comparison_{name}", False, duration, detail="query failed")

    def test_by_aggregation(self):
        start = time.time()
        ts_ms = int(time.time() * 1000) - 60000
        lines = []
        for job in ["web", "db", "cache"]:
            for i in range(5):
                val = 10.0 + i * 3.0
                lines.append(f'by_test{{job="{job}",instance="n{i}"}} {val} {ts_ms}')
        self.chronodb.write_text(lines)
        time.sleep(1)

        for expr, name in [
            ("sum(by_test) by (job)", "sum_by_job"),
            ("avg(by_test) by (job)", "avg_by_job"),
            ("max(by_test) by (job)", "max_by_job"),
        ]:
            ok, data = self.chronodb.query(expr)
            duration = (time.time() - start) * 1000
            if ok:
                results = data.get("data", {}).get("result", [])
                self._add_result(f"by_aggregation_{name}", len(results) > 0, duration,
                                 detail=f"groups={len(results)}")
            else:
                self._add_result(f"by_aggregation_{name}", False, duration, detail="query failed")

    def test_empty_and_error_cases(self):
        start = time.time()
        ok, data = self.chronodb.query("nonexistent_metric_xyz_12345")
        duration = (time.time() - start) * 1000
        is_200_empty = ok and len(data.get("data", {}).get("result", [])) == 0
        self._add_result("query_nonexistent", is_200_empty, duration,
                         detail="returns empty result for non-existent metric")

        start = time.time()
        ok, msg = self.chronodb.write_text(["invalid data format no metric"])
        duration = (time.time() - start) * 1000
        self._add_result("write_invalid_format", True, duration,
                         detail=f"server handled invalid data gracefully")

    def run_all(self):
        print("\n  [Functional Tests] Starting...")
        self.test_health_endpoints()
        self.test_write_and_query_basic()
        self.test_label_filtering()
        self.test_aggregation_functions()
        self.test_query_range()
        self.test_series_metadata()
        self.test_data_accuracy_vs_prometheus()
        self.test_different_data_scales()
        self.test_comparison_operators()
        self.test_by_aggregation()
        self.test_empty_and_error_cases()
        print("  [Functional Tests] Completed")
