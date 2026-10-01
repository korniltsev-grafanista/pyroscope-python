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

All rows above use `gil_only=true`. The table below uses `gil_only=false, include_idle=true`
on Linux aarch64 (OrbStack VM). "procfs overhead (before)" is the cost of the eliminated
`readdir`+stat path measured in isolation. "total ns/sample (after)" is the complete
per-sample cost with the new cpu-clock path. Estimated old total = after + procfs overhead.

| change | threads | procfs overhead ns (before) | total ns/sample (after) | est. old total ns |
|--------|--------:|----------------------------:|------------------------:|------------------:|
| perf: detect on-cpu threads with per-thread cpu clocks | 1 | 3936 | 1259 | 5195 |
| perf: detect on-cpu threads with per-thread cpu clocks | 10 | 28603 | 6287 | 34890 |
| perf: detect on-cpu threads with per-thread cpu clocks | 50 | 161947 | 33690 | 195637 |

The table below measures `bench_sample` with `gil_only=true` on Linux aarch64 (OrbStack VM)
at varying live-thread counts. Before: list walk proportional to thread count. After: direct
read of the GIL-owner thread state, flat in thread count.

| change | threads | ns/sample (before) | reads/sample (before) | ns/sample (after) | reads/sample (after) |
|--------|--------:|-------------------:|----------------------:|------------------:|---------------------:|
| perf: sample the gil owner without walking the thread list | 1 | 126.0 | 8.0 | 112.6 | 7.0 |
| perf: sample the gil owner without walking the thread list | 10 | 298.0 | 17.0 | 111.8 | 7.0 |
| perf: sample the gil owner without walking the thread list | 50 | 1045.0 | 57.0 | 120.0 | 7.0 |
