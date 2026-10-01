# Sampler benchmark baseline

## Environment

- Machine: OrbStack ubuntu (aarch64)
- OS: Ubuntu 24.04
- CPython: 3.12.3
- Commit: (filled in after measurement)

## Micro-benchmark (`bench_unwind.py`)

The micro-benchmark samples a frozen stack from the GIL-holding thread, so it
is an A/B instrument rather than a model of production overhead.

| depth | ns/sample | reads/sample | bytes/sample |
|------:|----------:|-------------:|-------------:|
|    10 |           |              |              |
|    50 |           |              |              |
|   200 |           |              |              |

## End-to-end (`bench_e2e.py`)

| scenario           | throughput | proc CPU% | sampler CPU% |
|:-------------------|-----------:|----------:|-------------:|
| baseline (no agent)|            |           |              |
| 100 Hz             |            |           |              |
| 1000 Hz            |            |           |              |
