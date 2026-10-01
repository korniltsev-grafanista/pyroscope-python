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
