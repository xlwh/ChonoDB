import random
import time
from typing import List, Dict, Tuple, Optional
from dataclasses import dataclass, field


@dataclass
class DataScale:
    name: str
    series_count: int
    samples_per_series: int
    label_combinations: int

    @property
    def total_samples(self) -> int:
        return self.series_count * self.samples_per_series


SMALL = DataScale("small", series_count=100, samples_per_series=100, label_combinations=10)
MEDIUM = DataScale("medium", series_count=1000, samples_per_series=500, label_combinations=50)
LARGE = DataScale("large", series_count=10000, samples_per_series=200, label_combinations=200)

DATA_SCALES = {"small": SMALL, "medium": MEDIUM, "large": LARGE}

METRIC_NAMES = [
    "cpu_usage_percent",
    "memory_usage_bytes",
    "disk_read_bytes",
    "disk_write_bytes",
    "network_rx_bytes",
    "network_tx_bytes",
    "http_requests_total",
    "http_request_duration_seconds",
    "process_cpu_seconds_total",
    "process_resident_memory_bytes",
]

JOBS = ["webserver", "database", "cache", "api-gateway", "load-balancer", "monitoring", "logging", "queue"]
REGIONS = ["us-east-1", "us-west-2", "eu-west-1", "ap-southeast-1", "ap-northeast-1"]
ENVIRONMENTS = ["production", "staging", "development"]
INSTANCES_PER_JOB = 5


class DataGenerator:
    def __init__(self, seed: int = 42):
        self.rng = random.Random(seed)
        self.series_counter = 0

    def generate_series_labels(self, scale: DataScale) -> List[Dict[str, str]]:
        all_series = []
        for metric in METRIC_NAMES:
            for job in JOBS:
                for env in ENVIRONMENTS:
                    for region in REGIONS:
                        for i in range(INSTANCES_PER_JOB):
                            labels = {
                                "__name__": metric,
                                "job": job,
                                "environment": env,
                                "region": region,
                                "instance": f"{job}-{i}",
                            }
                            all_series.append(labels)
                            if len(all_series) >= scale.series_count:
                                return all_series[: scale.series_count]
        return all_series[: scale.series_count]

    def generate_write_lines(
        self,
        scale: DataScale,
        base_ts_ms: Optional[int] = None,
        interval_ms: int = 15000,
    ) -> List[str]:
        if base_ts_ms is None:
            base_ts_ms = int(time.time() * 1000)

        series_list = self.generate_series_labels(scale)
        lines = []

        for series in series_list:
            metric_name = series.pop("__name__")
            label_str = ",".join(f'{k}="{v}"' for k, v in sorted(series.items()))
            series["__name__"] = metric_name

            for j in range(scale.samples_per_series):
                ts = base_ts_ms - (scale.samples_per_series - j) * interval_ms
                value = self._generate_value(metric_name)
                lines.append(f"{metric_name}{{{label_str}}} {value} {ts}")

        return lines

    def generate_write_batches(
        self,
        scale: DataScale,
        batch_size: int = 500,
        base_ts_ms: Optional[int] = None,
        interval_ms: int = 15000,
    ) -> List[List[str]]:
        all_lines = self.generate_write_lines(scale, base_ts_ms, interval_ms)
        batches = []
        for i in range(0, len(all_lines), batch_size):
            batches.append(all_lines[i : i + batch_size])
        return batches

    def _generate_value(self, metric_name: str) -> float:
        if "percent" in metric_name:
            return round(self.rng.uniform(0, 100), 4)
        elif "bytes" in metric_name:
            return round(self.rng.uniform(0, 1073741824), 2)
        elif "total" in metric_name:
            return round(self.rng.uniform(0, 1000000), 2)
        elif "duration" in metric_name:
            return round(self.rng.uniform(0, 10), 6)
        else:
            return round(self.rng.uniform(0, 1000), 4)

    def generate_known_data(
        self,
        series_count: int = 10,
        samples_per_series: int = 100,
        base_ts_ms: Optional[int] = None,
        interval_ms: int = 15000,
    ) -> Tuple[List[str], Dict[str, List[Tuple[int, float]]]]:
        if base_ts_ms is None:
            base_ts_ms = int(time.time() * 1000)

        lines = []
        known_data = {}

        for i in range(series_count):
            metric_name = "known_test_metric"
            job = f"test-job-{i % 3}"
            instance = f"test-instance-{i}"
            labels = {"__name__": metric_name, "job": job, "instance": instance}
            label_str = ",".join(f'{k}="{v}"' for k, v in sorted(labels.items()) if k != "__name__")
            key = f"{metric_name}{{{label_str}}}"

            samples = []
            for j in range(samples_per_series):
                ts = base_ts_ms - (samples_per_series - j) * interval_ms
                value = round(50.0 + i * 10.0 + j * 0.1, 4)
                lines.append(f"{metric_name}{{{label_str}}} {value} {ts}")
                samples.append((ts, value))

            known_data[key] = samples

        return lines, known_data
