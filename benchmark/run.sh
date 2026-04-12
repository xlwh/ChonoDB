#!/bin/bash
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

echo "============================================================"
echo "  ChronoDB Benchmark - One-Click Runner"
echo "============================================================"

cd "$SCRIPT_DIR"

python3 run_benchmark.py "$@"
