# Sampler benchmark baseline

## Environment

- Machine: OrbStack ubuntu (aarch64)
- OS: Ubuntu 24.04
- CPython: 3.12.3
- Commit: 6d4eb34 (test: add sampler benchmark harness)

The micro-benchmark samples a frozen stack from the GIL-holding thread, so it
is an A/B instrument rather than a model of production overhead.

## Micro-benchmark (`bench_unwind.py`)

Median of 7 runs, 200 iterations each, single warmup run discarded.

| depth | ns/sample | reads/sample | bytes/sample |
|------:|----------:|-------------:|-------------:|
|    10 |   31023.0 |        118.0 |       8769.0 |
|    50 |  109915.7 |        438.0 |      30529.0 |
|   200 |  411483.0 |       1638.0 |     112129.0 |

## End-to-end (`bench_e2e.py`)

10 s wall-clock run, depth=50, workload ops = recursive CPU-bound iterations.

| scenario            | throughput (ops/s) | proc CPU% | sampler CPU% |
|:--------------------|-------------------:|----------:|-------------:|
| baseline (no agent) |              687.5 |     100.0 |          0.0 |
| 100 Hz              |              649.2 |     102.2 |          1.9 |
| 1000 Hz             |              662.7 |     113.5 |         12.1 |
