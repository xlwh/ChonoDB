#!/usr/bin/env python3
"""
ChronoDB Benchmark - One-click test runner
Supports standalone and cluster mode testing with Prometheus comparison.

Usage:
    python run_benchmark.py --mode standalone [--scale small|medium|large] [--skip-fo] [--skip-perf] [--skip-func]
    python run_benchmark.py --mode cluster [--scale small|medium|large] [--skip-fo] [--skip-perf] [--skip-func]
    python run_benchmark.py --mode all [--scale small|medium|large]
"""

import argparse
import os
import sys
import time
import subprocess
from datetime import datetime

BENCH_DIR = os.path.dirname(os.path.abspath(__file__))
PROJECT_DIR = os.path.dirname(BENCH_DIR)
REPORT_DIR = os.path.join(BENCH_DIR, "reports")

sys.path.insert(0, BENCH_DIR)

from lib.chronodb_client import ChronoDBClient
from lib.prometheus_client import PrometheusClient
from lib.docker_manager import DockerManager
from lib.report import ReportCollector
from lib.data_generator import SMALL, MEDIUM, LARGE, DATA_SCALES

from tests.functional_test import FunctionalTest
from tests.performance_test import PerformanceTest
from tests.fo_test import FailoverTest


def ensure_dependencies():
    try:
        import requests
        import tabulate
        import yaml
    except ImportError:
        print("Installing dependencies...")
        subprocess.check_call([
            sys.executable, "-m", "pip", "install", "-r",
            os.path.join(BENCH_DIR, "requirements.txt"),
            "--break-system-packages",
        ])


def ensure_docker():
    result = subprocess.run(["docker", "--version"], capture_output=True, text=True)
    if result.returncode != 0:
        print("ERROR: Docker is not installed or not in PATH")
        sys.exit(1)

    result = subprocess.run(["docker", "compose", "version"], capture_output=True, text=True)
    if result.returncode != 0:
        print("ERROR: Docker Compose V2 is not available")
        sys.exit(1)


def run_standalone(args, reporter: ReportCollector):
    print("\n" + "=" * 70)
    print("  STANDALONE MODE BENCHMARK")
    print("=" * 70)

    compose_file = os.path.join(BENCH_DIR, "docker-compose-standalone.yml")
    docker = DockerManager(compose_file, project_name="bench-standalone")

    reporter.start_phase("standalone_setup")

    print("\n[1/5] Building Docker images...")
    if not docker.build():
        print("ERROR: Docker build failed")
        return False

    print("\n[2/5] Starting containers...")
    if not docker.start():
        print("ERROR: Docker start failed")
        return False

    chronodb = ChronoDBClient(base_url="http://localhost:19090")
    prometheus = PrometheusClient(base_url="http://localhost:19092")

    print("  Waiting for ChronoDB to be ready...")
    if not chronodb.wait_ready(max_retries=60, interval=3.0):
        print("ERROR: ChronoDB failed to start")
        logs = docker.container_logs("bench-chronodb", tail=50)
        print(f"  ChronoDB logs:\n{logs}")
        docker.stop()
        return False

    print("  Waiting for Prometheus to be ready...")
    if not prometheus.wait_ready(max_retries=60, interval=3.0):
        print("ERROR: Prometheus failed to start")
        docker.stop()
        return False

    print("  All services ready!")
    reporter.end_phase("standalone_setup")

    scale = DATA_SCALES.get(args.scale, SMALL)
    success = True

    try:
        if not args.skip_func:
            reporter.start_phase("standalone_functional")
            func_test = FunctionalTest(chronodb, prometheus, reporter)
            func_test.run_all()
            reporter.end_phase("standalone_functional")

        if not args.skip_perf:
            reporter.start_phase("standalone_performance")
            perf_test = PerformanceTest(chronodb, prometheus, reporter)
            scales = [scale]
            if args.scale == "all":
                scales = [SMALL, MEDIUM, LARGE]
            perf_test.run_all(scales=scales)
            reporter.end_phase("standalone_performance")

    except Exception as e:
        print(f"  ERROR during standalone tests: {e}")
        import traceback
        traceback.print_exc()
        success = False

    finally:
        print("\n[5/5] Stopping containers...")
        reporter.start_phase("standalone_teardown")
        docker.stop()
        reporter.end_phase("standalone_teardown")

    return success


def run_cluster(args, reporter: ReportCollector):
    print("\n" + "=" * 70)
    print("  CLUSTER MODE BENCHMARK")
    print("=" * 70)

    compose_file = os.path.join(BENCH_DIR, "docker-compose-cluster.yml")
    docker = DockerManager(compose_file, project_name="bench-cluster")

    reporter.start_phase("cluster_setup")

    print("\n[1/5] Building Docker images...")
    if not docker.build():
        print("ERROR: Docker build failed")
        return False

    print("\n[2/5] Starting containers...")
    if not docker.start():
        print("ERROR: Docker start failed")
        return False

    chronodb = ChronoDBClient(base_url="http://localhost:19090")
    prometheus = PrometheusClient(base_url="http://localhost:19092")

    print("  Waiting for etcd to be ready...")
    time.sleep(10)

    print("  Waiting for ChronoDB node1 to be ready...")
    if not chronodb.wait_ready(max_retries=90, interval=3.0):
        print("ERROR: ChronoDB node1 failed to start")
        logs = docker.container_logs("bench-chronodb-node1", tail=50)
        print(f"  Node1 logs:\n{logs}")
        docker.stop()
        return False

    print("  Waiting for Prometheus to be ready...")
    if not prometheus.wait_ready(max_retries=60, interval=3.0):
        print("ERROR: Prometheus failed to start")
        docker.stop()
        return False

    print("  All services ready!")
    reporter.end_phase("cluster_setup")

    scale = DATA_SCALES.get(args.scale, SMALL)
    success = True

    try:
        if not args.skip_func:
            reporter.start_phase("cluster_functional")
            func_test = FunctionalTest(chronodb, prometheus, reporter)
            func_test.run_all()
            reporter.end_phase("cluster_functional")

        if not args.skip_perf:
            reporter.start_phase("cluster_performance")
            perf_test = PerformanceTest(chronodb, prometheus, reporter)
            perf_test.run_all(scales=[scale])
            reporter.end_phase("cluster_performance")

        if not args.skip_fo:
            reporter.start_phase("cluster_failover")
            fo_test = FailoverTest(chronodb, prometheus, reporter, docker)
            fo_test.run_all()
            reporter.end_phase("cluster_failover")

    except Exception as e:
        print(f"  ERROR during cluster tests: {e}")
        import traceback
        traceback.print_exc()
        success = False

    finally:
        print("\n[5/5] Stopping containers...")
        reporter.start_phase("cluster_teardown")
        docker.stop()
        reporter.end_phase("cluster_teardown")

    return success


def main():
    parser = argparse.ArgumentParser(description="ChronoDB Benchmark Runner")
    parser.add_argument(
        "--mode", choices=["standalone", "cluster", "all"], default="all",
        help="Test mode: standalone, cluster, or all (default: all)",
    )
    parser.add_argument(
        "--scale", choices=["small", "medium", "large", "all"], default="small",
        help="Data scale for testing (default: small)",
    )
    parser.add_argument("--skip-func", action="store_true", help="Skip functional tests")
    parser.add_argument("--skip-perf", action="store_true", help="Skip performance tests")
    parser.add_argument("--skip-fo", action="store_true", help="Skip failover tests")
    parser.add_argument("--no-build", action="store_true", help="Skip Docker build (use existing images)")
    args = parser.parse_args()

    ensure_dependencies()
    ensure_docker()

    os.makedirs(REPORT_DIR, exist_ok=True)

    reporter = ReportCollector()
    all_success = True

    print(f"\n{'#' * 70}")
    print(f"  ChronoDB Benchmark Suite")
    print(f"  Mode: {args.mode}")
    print(f"  Scale: {args.scale}")
    print(f"  Time: {datetime.now().strftime('%Y-%m-%d %H:%M:%S')}")
    print(f"{'#' * 70}")

    if args.mode in ["standalone", "all"]:
        ok = run_standalone(args, reporter)
        all_success = all_success and ok

    if args.mode in ["cluster", "all"]:
        ok = run_cluster(args, reporter)
        all_success = all_success and ok

    reporter.print_summary()

    timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
    report_path = os.path.join(REPORT_DIR, f"benchmark_{timestamp}.json")
    reporter.save_json(report_path)
    print(f"\n  Report saved to: {report_path}")

    if not all_success:
        print("\n  ⚠ Some tests failed. Please review the results above.")
    else:
        print("\n  ✅ All tests passed!")

    return 0 if all_success else 1


if __name__ == "__main__":
    sys.exit(main())
