"""Reusable workload: recurse to a given depth, then spin the CPU.

Usage:
    from workload import run
    run(depth=50, iterations=10_000_000)
"""


def _inner(n: int) -> int:
    total = 0
    for i in range(n):
        total += i
    return total


def _recurse(depth: int, inner_n: int) -> int:
    if depth <= 0:
        return _inner(inner_n)
    return _recurse(depth - 1, inner_n)


def run(depth: int = 50, iterations: int = 5_000_000) -> int:
    return _recurse(depth, iterations)
