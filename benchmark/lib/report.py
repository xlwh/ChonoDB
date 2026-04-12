import json
import os
import time
from datetime import datetime
from typing import Dict, List, Any, Optional
from dataclasses import dataclass, field
from tabulate import tabulate


@dataclass
class TestResult:
    name: str
    category: str
    passed: bool
    duration_ms: float = 0.0
    detail: str = ""
    metrics: Dict[str, Any] = field(default_factory=dict)


@dataclass
class ComparisonResult:
    query_name: str
    chronodb_latency_ms: float
    prometheus_latency_ms: float
    chronodb_success: bool
    prometheus_success: bool
    chronodb_result_count: int
    prometheus_result_count: int
    data_consistent: bool
    speedup: float = 0.0


class ReportCollector:
    def __init__(self):
        self.results: List[TestResult] = []
        self.comparisons: List[ComparisonResult] = []
        self.start_time = time.time()
        self.phase_timings: Dict[str, float] = {}

    def add_result(self, result: TestResult):
        self.results.append(result)

    def add_comparison(self, comp: ComparisonResult):
        if comp.prometheus_latency_ms > 0:
            comp.speedup = comp.prometheus_latency_ms / comp.chronodb_latency_ms if comp.chronodb_latency_ms > 0 else 0
        self.comparisons.append(comp)

    def start_phase(self, name: str):
        self.phase_timings[name] = time.time()

    def end_phase(self, name: str):
        if name in self.phase_timings:
            self.phase_timings[name] = time.time() - self.phase_timings[name]

    def summary(self) -> Dict[str, Any]:
        total = len(self.results)
        passed = sum(1 for r in self.results if r.passed)
        failed = total - passed
        by_category: Dict[str, Dict] = {}
        for r in self.results:
            if r.category not in by_category:
                by_category[r.category] = {"passed": 0, "failed": 0, "total": 0}
            by_category[r.category]["total"] += 1
            if r.passed:
                by_category[r.category]["passed"] += 1
            else:
                by_category[r.category]["failed"] += 1

        return {
            "timestamp": datetime.now().isoformat(),
            "total_duration_s": round(time.time() - self.start_time, 2),
            "total_tests": total,
            "passed": passed,
            "failed": failed,
            "pass_rate": f"{passed/total*100:.1f}%" if total > 0 else "N/A",
            "by_category": by_category,
            "phase_timings": {k: round(v, 2) for k, v in self.phase_timings.items()},
        }

    def print_summary(self):
        s = self.summary()
        print("\n" + "=" * 70)
        print("  TEST REPORT SUMMARY")
        print("=" * 70)
        print(f"  Timestamp:       {s['timestamp']}")
        print(f"  Total Duration:  {s['total_duration_s']}s")
        print(f"  Total Tests:     {s['total_tests']}")
        print(f"  Passed:          {s['passed']}")
        print(f"  Failed:          {s['failed']}")
        print(f"  Pass Rate:       {s['pass_rate']}")

        if s["by_category"]:
            print("\n  Results by Category:")
            table = []
            for cat, stats in s["by_category"].items():
                rate = f"{stats['passed']/stats['total']*100:.0f}%" if stats['total'] > 0 else "N/A"
                table.append([cat, stats['total'], stats['passed'], stats['failed'], rate])
            print(tabulate(table, headers=["Category", "Total", "Passed", "Failed", "Rate"], tablefmt="simple"))

        if self.comparisons:
            print("\n  Performance Comparison (ChronoDB vs Prometheus):")
            table = []
            for c in self.comparisons:
                speedup_str = f"{c.speedup:.2f}x" if c.speedup > 0 else "N/A"
                consistency = "✓" if c.data_consistent else "✗"
                table.append([
                    c.query_name,
                    f"{c.chronodb_latency_ms:.1f}ms",
                    f"{c.prometheus_latency_ms:.1f}ms",
                    speedup_str,
                    consistency,
                ])
            print(tabulate(table, headers=["Query", "ChronoDB", "Prometheus", "Speedup", "Consistent"], tablefmt="simple"))

            avg_speedup = 0
            valid_comps = [c for c in self.comparisons if c.speedup > 0]
            if valid_comps:
                avg_speedup = sum(c.speedup for c in valid_comps) / len(valid_comps)
            print(f"\n  Average Speedup: {avg_speedup:.2f}x")

        if s["phase_timings"]:
            print("\n  Phase Timings:")
            for phase, duration in s["phase_timings"].items():
                print(f"    {phase}: {duration}s")

        failed_results = [r for r in self.results if not r.passed]
        if failed_results:
            print("\n  Failed Tests:")
            for r in failed_results:
                print(f"    ✗ [{r.category}] {r.name}: {r.detail}")

        print("=" * 70)

    def save_json(self, filepath: str):
        data = {
            "summary": self.summary(),
            "results": [
                {
                    "name": r.name,
                    "category": r.category,
                    "passed": r.passed,
                    "duration_ms": r.duration_ms,
                    "detail": r.detail,
                    "metrics": r.metrics,
                }
                for r in self.results
            ],
            "comparisons": [
                {
                    "query_name": c.query_name,
                    "chronodb_latency_ms": c.chronodb_latency_ms,
                    "prometheus_latency_ms": c.prometheus_latency_ms,
                    "chronodb_success": c.chronodb_success,
                    "prometheus_success": c.prometheus_success,
                    "chronodb_result_count": c.chronodb_result_count,
                    "prometheus_result_count": c.prometheus_result_count,
                    "data_consistent": c.data_consistent,
                    "speedup": c.speedup,
                }
                for c in self.comparisons
            ],
        }
        os.makedirs(os.path.dirname(filepath) if os.path.dirname(filepath) else ".", exist_ok=True)
        with open(filepath, "w") as f:
            json.dump(data, f, indent=2, ensure_ascii=False)
