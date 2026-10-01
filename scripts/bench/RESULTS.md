# Benchmark results per change

BASELINE.md is the fixed reference point. This file tracks per-change deltas.
Rows are identified by commit subject, since a commit cannot record its own
hash.

| change | depth | ns/sample | reads/sample |
|--------|------:|----------:|-------------:|
| baseline | 10 | 31023.0 | 118.0 |
| baseline | 50 | 109915.7 | 438.0 |
| baseline | 200 | 411483.0 | 1638.0 |
| perf: read structs without heap allocation | 10 | 25744.6 | 118.0 |
| perf: read structs without heap allocation | 50 | 108883.6 | 438.0 |
| perf: read structs without heap allocation | 200 | 392267.9 | 1638.0 |
| perf: read own memory with guarded memcpy instead of a syscall | 10 | 2375.8 | 118.0 |
| perf: read own memory with guarded memcpy instead of a syscall | 50 | 10557.7 | 438.0 |
| perf: read own memory with guarded memcpy instead of a syscall | 200 | 27556.3 | 1638.0 |
| perf: cache resolved frames by code object | 10 | 1187.5 | 34.0 |
| perf: cache resolved frames by code object | 50 | 4563.1 | 114.0 |
| perf: cache resolved frames by code object | 200 | 16972.6 | 414.0 |

All rows above use `bench_unwind` (`gil_only=true`) at the stated stack depth on Linux
aarch64 (OrbStack VM).

The table below uses `bench_sample` with `gil_only=false, include_idle=true` on Linux
aarch64 (OrbStack VM) at stack depth 10. Worker threads spin on a tight loop so they are
on-CPU during sampling. The timed loop runs with the GIL released so all threads are
visible and active. "procfs overhead (before)" is the cost of the eliminated `readdir`+stat
path measured in isolation before the cpu-clock change; those prior numbers were collected
with the GIL held, which parked the workers and made the sampler see at most one active
thread -- they measured a degenerate case and are superseded by the corrected rows below.

| change | threads | ns/sample | reads/sample |
|--------|--------:|----------:|-------------:|
| fix: key the frame cache on code object identity, not just its address | 1 | 28187.6 | 22.1 |
| fix: key the frame cache on code object identity, not just its address | 10 | 91222.9 | 133.6 |
| fix: key the frame cache on code object identity, not just its address | 50 | 205054.9 | 785.3 |

The table below measures `bench_sample` with `gil_only=true` on Linux aarch64 (OrbStack VM)
at varying live-thread counts; stack depth during measurement was shallow (bench function
called without deep recursion). Before: list walk proportional to thread count. After: direct
read of the GIL-owner thread state, flat in thread count. These numbers are not affected by
the GIL-release change: `gil_only=true` keeps the GIL held during the timed loop.

| change | threads | ns/sample (before) | reads/sample (before) | ns/sample (after) | reads/sample (after) |
|--------|--------:|-------------------:|----------------------:|------------------:|---------------------:|
| perf: sample the gil owner without walking the thread list | 1 | 126.0 | 8.0 | 112.6 | 7.0 |
| perf: sample the gil owner without walking the thread list | 10 | 298.0 | 17.0 | 111.8 | 7.0 |
| perf: sample the gil owner without walking the thread list | 50 | 1045.0 | 57.0 | 120.0 | 7.0 |
