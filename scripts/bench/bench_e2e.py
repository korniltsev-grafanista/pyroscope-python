"""End-to-end CPU cost benchmark for the sampler thread.

Usage:
    python scripts/bench/bench_e2e.py

Linux only -- uses /proc for per-thread CPU accounting.
Starts a throwaway HTTP sink, runs the Pyroscope agent at 100 Hz and 1000 Hz,
and reports sampler-thread utime+stime, process-wide CPU, and workload
throughput against an agent-off baseline.
"""

import sys
import os
import time
import threading
import http.server
import statistics
import subprocess

WALL_SECONDS = 10
DEPTH = 50

if sys.platform != "linux":
    print("skip: bench_e2e.py requires Linux for /proc/self/task accounting")
    sys.exit(0)

import pyroscope
from workload import run as workload_run


def _find_sampler_tid(name: str = "pyro-sampler") -> int | None:
    task_dir = "/proc/self/task"
    try:
        for tid in os.listdir(task_dir):
            comm_path = os.path.join(task_dir, tid, "comm")
            try:
                with open(comm_path) as f:
                    if f.read().strip() == name:
                        return int(tid)
            except OSError:
                pass
    except OSError:
        pass
    return None


def _thread_cpu_jiffies(tid: int) -> int:
    with open(f"/proc/self/task/{tid}/stat") as f:
        fields = f.read().split()
    return int(fields[13]) + int(fields[14])


def _process_cpu_jiffies() -> int:
    with open("/proc/self/stat") as f:
        fields = f.read().split()
    return int(fields[13]) + int(fields[14])


def _hz() -> int:
    try:
        return os.sysconf("SC_CLK_TCK")
    except (AttributeError, ValueError):
        return 100


class _SilentHandler(http.server.BaseHTTPRequestHandler):
    def do_POST(self):
        length = int(self.headers.get("Content-Length", 0))
        self.rfile.read(length)
        self.send_response(200)
        self.end_headers()

    def log_message(self, *_):
        pass


def _start_sink() -> tuple[http.server.HTTPServer, int]:
    server = http.server.HTTPServer(("127.0.0.1", 0), _SilentHandler)
    port = server.server_address[1]
    t = threading.Thread(target=server.serve_forever, daemon=True)
    t.start()
    return server, port


def _run_workload(seconds: float):
    deadline = time.monotonic() + seconds
    count = 0
    while time.monotonic() < deadline:
        workload_run(depth=DEPTH, iterations=100_000)
        count += 1
    return count


def _measure(sample_rate: int | None, sink_port: int) -> dict:
    if sample_rate is not None:
        pyroscope.configure(
            application_name="bench_e2e",
            server_address=f"http://127.0.0.1:{sink_port}",
            sample_rate=sample_rate,
        )
        time.sleep(0.5)
        tid = _find_sampler_tid()
    else:
        tid = None

    t0_proc = _process_cpu_jiffies()
    t0_thr = _thread_cpu_jiffies(tid) if tid else 0
    t0 = time.monotonic()

    count = _run_workload(WALL_SECONDS)

    t1 = time.monotonic()
    t1_proc = _process_cpu_jiffies()
    t1_thr = _thread_cpu_jiffies(tid) if tid else 0

    if sample_rate is not None:
        pyroscope.shutdown()

    hz = _hz()
    wall = t1 - t0
    proc_cpu_s = (t1_proc - t0_proc) / hz
    thr_cpu_s = (t1_thr - t0_thr) / hz if tid else 0.0

    return {
        "rate": sample_rate,
        "wall_s": wall,
        "workload_ops": count,
        "throughput": count / wall,
        "proc_cpu_s": proc_cpu_s,
        "proc_cpu_pct": 100.0 * proc_cpu_s / wall,
        "sampler_cpu_s": thr_cpu_s,
        "sampler_cpu_pct": 100.0 * thr_cpu_s / wall,
        "sampler_tid": tid,
    }


def main():
    server, port = _start_sink()

    baseline = _measure(None, port)
    r100 = _measure(100, port)
    r1000 = _measure(1000, port)

    server.shutdown()

    rows = [baseline, r100, r1000]
    labels = ["baseline (no agent)", "100 Hz", "1000 Hz"]

    print(f"\n{'scenario':<22}  {'throughput':>12}  {'proc CPU%':>10}  {'sampler CPU%':>13}  {'sampler tid':>12}")
    print("-" * 80)
    for label, row in zip(labels, rows):
        print(
            f"{label:<22}  {row['throughput']:>12.1f}  {row['proc_cpu_pct']:>10.1f}"
            f"  {row['sampler_cpu_pct']:>13.1f}  {str(row['sampler_tid'] or '-'):>12}"
        )
    print()


if __name__ == "__main__":
    main()
