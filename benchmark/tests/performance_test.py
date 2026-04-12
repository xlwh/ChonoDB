import time
import sys
import os
import statistics

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from lib.chronodb_client import ChronoDBClient
from lib.prometheus_client import PrometheusClient
from lib.data_generator import DataGenerator, SMALL, MEDIUM, LARGE
from lib.report import ReportCollector, TestResult, ComparisonResult


TIME_RANGES = [
    ("1h", 3600),
    ("6h", 21600),
    ("24h", 86400),
    ("7d", 604800),
]

QUERY_TEMPLATES = [
    ("basic_selector", "cpu_usage_percent"),
    ("label_filter", 'cpu_usage_percent{job="webserver"}'),
    ("sum_agg", "sum(cpu_usage_percent)"),
    ("avg_agg", "avg(cpu_usage_percent)"),
    ("max_agg", "max(cpu_usage_percent)"),
    ("sum_by_job", "sum(cpu_usage_percent) by (job)"),
    ("avg_by_region", "avg(cpu_usage_percent) by (region)"),
    ("gt_filter", "cpu_usage_percent > 50"),
    ("rate_func", "rate(cpu_usage_percent[5m])"),
    ("sum_rate", "sum(rate(cpu_usage_percent[5m]))"),
]


class PerformanceTest:
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
        self._data_loaded = False

    def _add_result(self, name: str, passed: bool, duration_ms: float, detail: str = "", metrics: dict = None):
        self.reporter.add_result(TestResult(
            name=name, category="performance", passed=passed,
            duration_ms=duration_ms, detail=detail, metrics=metrics or {},
        ))

    def load_data(self, scale):
        print(f"  [Performance] Loading {scale.name} data ({scale.total_samples} samples)...")
        start = time.time()

        batches = self.generator.generate_write_batches(scale, batch_size=500)

        c_written = 0
        p_written = 0
        for batch in batches:
            c_ok, _ = self.chronodb.write_text(batch)
            if c_ok:
                c_written += len(batch)
            p_ok, _ = self.prometheus.write_text(batch)
            if p_ok:
                p_written += len(batch)

        time.sleep(3)
        duration = (time.time() - start) * 1000

        self._add_result(
            f"data_load_{scale.name}", c_written > 0 and p_written > 0, duration,
            detail=f"chronodb={c_written}, prometheus={p_written}",
            metrics={"chronodb_written": c_written, "prometheus_written": p_written},
        )
        self._data_loaded = True
        print(f"  [Performance] Data loaded: chronodb={c_written}, prometheus={p_written}")

    def test_write_throughput(self, scale):
        print(f"  [Performance] Testing write throughput ({scale.name})...")
        start = time.time()

        ts_ms = int(time.time() * 1000) + 86400000
        lines = []
        for i in range(scale.series_count):
            val = 50.0 + i * 0.1
            ts = ts_ms + i * 15000
            lines.append(f'write_perf_{{job="perf",instance="n{i}"}} {val} {ts}')

        batch_size = 500
        c_latencies = []
        p_latencies = []

        for i in range(0, len(lines), batch_size):
            batch = lines[i:i + batch_size]

            t0 = time.time()
            c_ok, _ = self.chronodb.write_text(batch)
            c_lat = (time.time() - t0) * 1000
            c_latencies.append(c_lat)

            t0 = time.time()
            p_ok, _ = self.prometheus.write_text(batch)
            p_lat = (time.time() - t0) * 1000
            p_latencies.append(p_lat)

        duration = (time.time() - start) * 1000

        c_avg = statistics.mean(c_latencies) if c_latencies else 0
        p_avg = statistics.mean(p_latencies) if p_latencies else 0
        c_p99 = sorted(c_latencies)[int(len(c_latencies) * 0.99)] if c_latencies else 0
        p_p99 = sorted(p_latencies)[int(len(p_latencies) * 0.99)] if p_latencies else 0

        speedup = p_avg / c_avg if c_avg > 0 else 0

        self._add_result(
            f"write_throughput_{scale.name}", True, duration,
            detail=f"chronodb_avg={c_avg:.1f}ms, prometheus_avg={p_avg:.1f}ms, speedup={speedup:.2f}x",
            metrics={
                "chronodb_avg_ms": round(c_avg, 2),
                "prometheus_avg_ms": round(p_avg, 2),
                "chronodb_p99_ms": round(c_p99, 2),
                "prometheus_p99_ms": round(p_p99, 2),
                "speedup": round(speedup, 2),
            },
        )

    def test_query_latency_comparison(self, scale):
        print(f"  [Performance] Testing query latency ({scale.name})...")
        now_ts = int(time.time())

        for query_name, expr in QUERY_TEMPLATES:
            c_latencies = []
            p_latencies = []
            c_success = False
            p_success = False
            c_result_count = 0
            p_result_count = 0

            for _ in range(5):
                t0 = time.time()
                c_ok, c_data = self.chronodb.query(expr, ts=now_ts)
                c_lat = (time.time() - t0) * 1000
                c_latencies.append(c_lat)
                if c_ok:
                    c_success = True
                    c_result_count = len(c_data.get("data", {}).get("result", []))

                t0 = time.time()
                p_ok, p_data = self.prometheus.query(expr, ts=now_ts)
                p_lat = (time.time() - t0) * 1000
                p_latencies.append(p_lat)
                if p_ok:
                    p_success = True
                    p_result_count = len(p_data.get("data", {}).get("result", []))

            c_avg = statistics.mean(c_latencies) if c_latencies else 0
            p_avg = statistics.mean(p_latencies) if p_latencies else 0
            c_p99 = sorted(c_latencies)[int(len(c_latencies) * 0.99)] if len(c_latencies) > 1 else c_avg
            p_p99 = sorted(p_latencies)[int(len(p_latencies) * 0.99)] if len(p_latencies) > 1 else p_avg

            data_consistent = c_success and p_success and c_result_count == p_result_count

            comp = ComparisonResult(
                query_name=f"{query_name}_{scale.name}",
                chronodb_latency_ms=round(c_avg, 2),
                prometheus_latency_ms=round(p_avg, 2),
                chronodb_success=c_success,
                prometheus_success=p_success,
                chronodb_result_count=c_result_count,
                prometheus_result_count=p_result_count,
                data_consistent=data_consistent,
            )
            self.reporter.add_comparison(comp)

            self._add_result(
                f"query_latency_{query_name}_{scale.name}",
                c_success and p_success,
                c_avg,
                detail=f"chronodb={c_avg:.1f}ms, prometheus={p_avg:.1f}ms, speedup={comp.speedup:.2f}x",
                metrics={
                    "chronodb_avg_ms": round(c_avg, 2),
                    "prometheus_avg_ms": round(p_avg, 2),
                    "chronodb_p99_ms": round(c_p99, 2),
                    "prometheus_p99_ms": round(p_p99, 2),
                    "speedup": round(comp.speedup, 2),
                    "data_consistent": data_consistent,
                },
            )

    def test_query_range_performance(self, scale):
        print(f"  [Performance] Testing query_range performance ({scale.name})...")
        now_ts = int(time.time())

        for range_name, range_seconds in TIME_RANGES:
            start_ts = now_ts - range_seconds
            step = "15s" if range_seconds <= 21600 else "1m"

            c_latencies = []
            p_latencies = []

            for _ in range(3):
                t0 = time.time()
                c_ok, c_data = self.chronodb.query_range("cpu_usage_percent", start_ts, now_ts, step)
                c_lat = (time.time() - t0) * 1000
                c_latencies.append(c_lat)

                t0 = time.time()
                p_ok, p_data = self.prometheus.query_range("cpu_usage_percent", start_ts, now_ts, step)
                p_lat = (time.time() - t0) * 1000
                p_latencies.append(p_lat)

            c_avg = statistics.mean(c_latencies) if c_latencies else 0
            p_avg = statistics.mean(p_latencies) if p_latencies else 0
            speedup = p_avg / c_avg if c_avg > 0 else 0

            c_samples = 0
            p_samples = 0
            if c_ok:
                c_results = c_data.get("data", {}).get("result", [])
                c_samples = sum(len(r.get("values", [])) for r in c_results)
            if p_ok:
                p_results = p_data.get("data", {}).get("result", [])
                p_samples = sum(len(r.get("values", [])) for r in p_results)

            self._add_result(
                f"query_range_{range_name}_{scale.name}",
                c_ok and p_ok,
                c_avg,
                detail=f"chronodb={c_avg:.1f}ms({c_samples}samples), prometheus={p_avg:.1f}ms({p_samples}samples), speedup={speedup:.2f}x",
                metrics={
                    "chronodb_avg_ms": round(c_avg, 2),
                    "prometheus_avg_ms": round(p_avg, 2),
                    "chronodb_samples": c_samples,
                    "prometheus_samples": p_samples,
                    "speedup": round(speedup, 2),
                },
            )

    def test_concurrent_query_performance(self, scale):
        print(f"  [Performance] Testing concurrent queries ({scale.name})...")
        import concurrent.futures

        queries = [
            "cpu_usage_percent",
            "memory_usage_bytes",
            'cpu_usage_percent{job="webserver"}',
            "sum(cpu_usage_percent)",
            "avg(memory_usage_bytes)",
            "max(cpu_usage_percent) by (job)",
            "sum(rate(cpu_usage_percent[5m]))",
        ] * 5

        now_ts = int(time.time())

        def run_chronodb_query(expr):
            t0 = time.time()
            ok, _ = self.chronodb.query(expr, ts=now_ts)
            return (time.time() - t0) * 1000, ok

        def run_prometheus_query(expr):
            t0 = time.time()
            ok, _ = self.prometheus.query(expr, ts=now_ts)
            return (time.time() - t0) * 1000, ok

        start = time.time()
        with concurrent.futures.ThreadPoolExecutor(max_workers=10) as executor:
            c_futures = [executor.submit(run_chronodb_query, q) for q in queries]
            c_results = [f.result() for f in concurrent.futures.as_completed(c_futures)]

        c_latencies = [r[0] for r in c_results]
        c_success_rate = sum(1 for r in c_results if r[1]) / len(c_results) * 100

        start = time.time()
        with concurrent.futures.ThreadPoolExecutor(max_workers=10) as executor:
            p_futures = [executor.submit(run_prometheus_query, q) for q in queries]
            p_results = [f.result() for f in concurrent.futures.as_completed(p_futures)]

        p_latencies = [r[0] for r in p_results]
        p_success_rate = sum(1 for r in p_results if r[1]) / len(p_results) * 100

        c_avg = statistics.mean(c_latencies) if c_latencies else 0
        p_avg = statistics.mean(p_latencies) if p_latencies else 0
        speedup = p_avg / c_avg if c_avg > 0 else 0

        self._add_result(
            f"concurrent_query_{scale.name}",
            c_success_rate >= 80 and p_success_rate >= 80,
            c_avg,
            detail=f"chronodb: avg={c_avg:.1f}ms, success={c_success_rate:.0f}%; prometheus: avg={p_avg:.1f}ms, success={p_success_rate:.0f}%",
            metrics={
                "chronodb_avg_ms": round(c_avg, 2),
                "prometheus_avg_ms": round(p_avg, 2),
                "chronodb_success_rate": round(c_success_rate, 1),
                "prometheus_success_rate": round(p_success_rate, 1),
                "speedup": round(speedup, 2),
            },
        )

    def run_all(self, scales=None):
        if scales is None:
            scales = [SMALL, MEDIUM]

        print("\n  [Performance Tests] Starting...")
        for scale in scales:
            self.load_data(scale)
            self.test_write_throughput(scale)
            self.test_query_latency_comparison(scale)
            self.test_query_range_performance(scale)
            self.test_concurrent_query_performance(scale)
        print("  [Performance Tests] Completed")
