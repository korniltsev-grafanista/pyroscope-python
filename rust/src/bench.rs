use std::time::Instant;

use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use pyo3::wrap_pyfunction;

fn make_spy(gil_only: bool, include_idle: bool) -> PyResult<py_spy::PythonSpy> {
    #[cfg(all(
        any(target_os = "linux", target_os = "macos"),
        any(target_arch = "x86_64", target_arch = "aarch64")
    ))]
    {
        if kindasafe_init::init().is_ok() && kindasafe_init::sanity_check().is_ok() {
            remoteprocess::enable_kindasafe();
        }
    }

    let pid: py_spy::Pid = std::process::id()
        .try_into()
        .map_err(|e: std::num::TryFromIntError| PyRuntimeError::new_err(e.to_string()))?;

    let config = py_spy::Config {
        blocking: py_spy::config::LockingStrategy::NonBlocking,
        native: false,
        pid: Some(pid),
        sampling_rate: 100,
        include_idle,
        include_thread_ids: true,
        include_thread_names: false,
        subprocesses: false,
        gil_only,
        lineno: py_spy::config::LineNo::LastInstruction,
        ..py_spy::Config::default()
    };

    py_spy::PythonSpy::new(pid, &config).map_err(|e| PyRuntimeError::new_err(e.to_string()))
}

#[pyfunction]
pub fn bench_unwind(py: Python<'_>, iterations: u64) -> PyResult<(f64, f64, f64)> {
    bench_sample(py, iterations, true, false)
}

#[pyfunction]
pub fn bench_sample(
    py: Python<'_>,
    iterations: u64,
    gil_only: bool,
    include_idle: bool,
) -> PyResult<(f64, f64, f64)> {
    let mut spy = make_spy(gil_only, include_idle)?;

    let traces = spy
        .get_stack_traces()
        .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
    let max_frames = traces.iter().map(|t| t.frames.len()).max().unwrap_or(0);
    if traces.is_empty() || max_frames == 0 {
        return Err(PyRuntimeError::new_err(format!(
            "sampler returned implausible result: {} traces, {} max frames",
            traces.len(),
            max_frames
        )));
    }

    py_spy::counters::take();

    let start = Instant::now();
    if gil_only {
        for _ in 0..iterations {
            let _ = spy.get_stack_traces();
        }
    } else {
        py.detach(|| {
            for _ in 0..iterations {
                let _ = spy.get_stack_traces();
            }
        });
    }
    let elapsed = start.elapsed();

    let (reads, bytes) = py_spy::counters::take();

    let ns = elapsed.as_nanos() as f64 / iterations as f64;
    let r = reads as f64 / iterations as f64;
    let b = bytes as f64 / iterations as f64;

    Ok((ns, r, b))
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(bench_unwind, m)?)?;
    m.add_function(wrap_pyfunction!(bench_sample, m)?)
}
