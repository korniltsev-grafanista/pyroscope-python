use std::time::Instant;

use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use pyo3::wrap_pyfunction;

#[pyfunction]
pub fn bench_unwind(_py: Python<'_>, iterations: u64) -> PyResult<(f64, f64, f64)> {
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
        include_idle: false,
        include_thread_ids: true,
        include_thread_names: false,
        subprocesses: false,
        gil_only: true,
        lineno: py_spy::config::LineNo::LastInstruction,
        ..py_spy::Config::default()
    };

    let mut spy =
        py_spy::PythonSpy::new(pid, &config).map_err(|e| PyRuntimeError::new_err(e.to_string()))?;

    let _ = spy.get_stack_traces();

    py_spy::counters::take();

    let start = Instant::now();
    for _ in 0..iterations {
        let _ = spy.get_stack_traces();
    }
    let elapsed = start.elapsed();

    let (reads, bytes) = py_spy::counters::take();

    let ns = elapsed.as_nanos() as f64 / iterations as f64;
    let r = reads as f64 / iterations as f64;
    let b = bytes as f64 / iterations as f64;

    Ok((ns, r, b))
}

#[pyfunction]
pub fn bench_sample(
    _py: Python<'_>,
    iterations: u64,
    gil_only: bool,
    include_idle: bool,
) -> PyResult<(f64, f64, f64)> {
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

    let mut spy =
        py_spy::PythonSpy::new(pid, &config).map_err(|e| PyRuntimeError::new_err(e.to_string()))?;

    let _ = spy.get_stack_traces();

    py_spy::counters::take();

    let start = Instant::now();
    for _ in 0..iterations {
        let _ = spy.get_stack_traces();
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
