"""Micro-benchmark for the in-process stack sampler.

Usage:
    python scripts/bench/bench_unwind.py

Requires a wheel built with PYROSCOPE_BUILD_FEATURES=bench.
Reports ns/sample, reads/sample, bytes/sample at stack depths 10, 50, 200.
"""

import sys
import statistics

from pyroscope._native import bench_unwind


DEPTHS = [10, 50, 200]
WARMUP = 1
RUNS = 7
ITERATIONS = 200


def _recurse(depth: int, fn, *args):
    if depth <= 0:
        return fn(*args)
    return _recurse(depth - 1, fn, *args)


def measure(depth: int):
    for _ in range(WARMUP):
        _recurse(depth, bench_unwind, ITERATIONS)

    ns_samples = []
    reads_samples = []
    bytes_samples = []
    for _ in range(RUNS):
        ns, r, b = _recurse(depth, bench_unwind, ITERATIONS)
        ns_samples.append(ns)
        reads_samples.append(r)
        bytes_samples.append(b)

    return (
        statistics.median(ns_samples),
        statistics.median(reads_samples),
        statistics.median(bytes_samples),
    )


def main():
    print(f"{'depth':>8}  {'ns/sample':>12}  {'reads/sample':>14}  {'bytes/sample':>14}")
    print("-" * 56)
    for depth in DEPTHS:
        ns, r, b = measure(depth)
        print(f"{depth:>8}  {ns:>12.1f}  {r:>14.1f}  {b:>14.1f}")


if __name__ == "__main__":
    main()
