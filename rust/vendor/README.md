# Vendored crates

## py-spy

- Upstream: https://github.com/grafana/pyroscope-py-spy
- Pinned SHA: `e9858c037e796327391708f43d454ec611594ecc`
- Stripped: `images/`, `.github/`, `ci/`, `tests/`, `.cargo/`, `CODEOWNERS`,
  `.pre-commit-config.yaml`, `pyproject.toml`, `setup.cfg`, `uv.lock`,
  `generate_bindings.py`, `.gitignore`, `CHANGELOG.md`, `AGENTS.md`,
  `SECURITY.md`, `examples/`, `Cargo.lock`
- Also removed: `[dev-dependencies]` entry for `py-spy-testdata`, and the
  `#[cfg(test)]` module in `src/cython.rs` (referenced the dropped
  `ci/testdata/cython_test.c`)
- remoteprocess dependency changed to `path = "../remoteprocess"`

To re-sync: check out the upstream repo at the desired SHA, copy `src/`,
`Cargo.toml`, `build.rs`, `LICENSE`, `README.md` here, then reapply the
edits above.

## remoteprocess

- Upstream: https://github.com/grafana/pyroscope-remoteprocess
- Pinned SHA: `579e513a5584adc55b9c88e3d5212bb41c328315`
- Stripped: `CODEOWNERS`, `examples/`

To re-sync: check out the upstream repo at the desired SHA, copy `src/`,
`Cargo.toml`, `build.rs`, `LICENSE`, `README.md` here.
