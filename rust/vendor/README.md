# Vendored crates

## py-spy

- Upstream: https://github.com/grafana/pyroscope-py-spy
- Pinned SHA: `e9858c037e796327391708f43d454ec611594ecc`
- Stripped (initial vendor): `images/`, `.github/`, `ci/`, `tests/`, `.cargo/`, `CODEOWNERS`,
  `.pre-commit-config.yaml`, `pyproject.toml`, `setup.cfg`, `uv.lock`,
  `generate_bindings.py`, `.gitignore`, `CHANGELOG.md`, `AGENTS.md`,
  `SECURITY.md`, `examples/`, `Cargo.lock`
- Also removed (initial vendor): `[dev-dependencies]` entry for `py-spy-testdata`, and the
  `#[cfg(test)]` module in `src/cython.rs` (referenced the dropped
  `ci/testdata/cython_test.c`)
- remoteprocess dependency changed to `path = "../remoteprocess"`
- Stripped (in-process trim): CLI binary (`src/main.rs`), renderer modules
  (`src/flamegraph.rs`, `src/speedscope.rs`, `src/chrometrace.rs`,
  `src/console_viewer.rs`), dump helpers (`src/dump.rs`, `src/coredump.rs`),
  native-unwind support (`src/native_stack_trace.rs`, `src/cython.rs`);
  removed `[[bin]]` section, `cli`/`unwind` features, and deps
  `clap`, `clap_complete`, `console`, `ctrlc`, `indicatif`, `inferno`,
  `env_logger`, `cpp_demangle`, `chrono`, `lru`, `tempfile`, `serde_json`,
  `termios`; removed all `#[cfg(feature = "cli")]` and
  `#[cfg(feature = "unwind")]` blocks from surviving files

To re-sync: check out the upstream repo at the desired SHA, copy `src/`,
`Cargo.toml`, `build.rs`, `LICENSE`, `README.md` here, then reapply all
the edits above.

## remoteprocess

- Upstream: https://github.com/grafana/pyroscope-remoteprocess
- Pinned SHA: `579e513a5584adc55b9c88e3d5212bb41c328315`
- Stripped (initial vendor): `CODEOWNERS`, `examples/`
- Stripped (in-process trim): `src/linux/libunwind/` directory and its
  `mod` declaration; `unwinder()` and `symbolicator()` entry points in
  `src/linux/mod.rs`; all `#[cfg(use_libunwind)]` blocks in `src/lib.rs`
  and `src/linux/mod.rs`; link logic for libunwind/lzma removed from
  `build.rs`; `unwind` feature removed from `Cargo.toml`

To re-sync: check out the upstream repo at the desired SHA, copy `src/`,
`Cargo.toml`, `build.rs`, `LICENSE`, `README.md` here, then reapply all
the edits above.

## kindasafe / kindasafe_init

These crates are not vendored; they are pulled from git at build time.

- Source: https://github.com/grafana/pyroscope-kinda-safe
- Pinned rev: `a2c5388ce896940424890519f5f9faab08394c83`
  (branch `perf/aarch64-slice-blocks`, draft PR
  [grafana/pyroscope-kinda-safe#41](https://github.com/grafana/pyroscope-kinda-safe/pull/41))
- Both crates **must** come from the same git source and rev. A mixed pin
  (one from git, one from crates.io) builds two copies of `kindasafe`; the
  signal handler matches crash-point PCs from one copy while reads execute
  code in the other. There is no compile-time signal for this, but the
  init path in `rust/src/lib.rs` probes the real read path through
  `remoteprocess::Process::read` on a `PROT_NONE` page and disables the
  fast path if it does not return `ReadMemFault`, catching this mismatch
  at startup.
- TODO: move pin to a released version once the PR merges.
- Known limitation: we assume this process owns SIGSEGV and SIGBUS. Any
  handler installed after ours (CPython `faulthandler`, a crash reporter,
  pytest's default setup) breaks fault recovery. Detection and chaining are
  left as a follow-up.
