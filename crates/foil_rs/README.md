# `foil_rs`

Core library for FoilRs: NACA 4‑digit airfoil geometry + a lightweight vortex-panel flow solver, Cp sampling, simple polar sweeps, and a small boundary-layer estimate for `CDp`.

This crate has **no Bevy dependency** and is intended to be usable in headless/batch workflows.

## Usage

Add the dependency:

```toml
[dependencies]
foil_rs = "0.1"
```

## Examples

- Headless single solve:
  - `cargo run -p foil_rs --example headless -- 2412 4.0`
- Export a polar sweep as CSV:
  - `cargo run -p foil_rs --example export_polar_csv --release -- 2412`
- Export multi-polars as CSV:
  - `cargo run -p foil_rs --example export_multi_polars_csv --release -- 2412 "0.5,1.0,2.0" "0.0,0.1"`
- Benchmark harness:
  - `cargo run -p foil_rs --example bench_headless --release -- 2412 4.0`

## Status

`0.1.x` is a “preview” series: the API may change, and numerical results are intended for visualization and trend exploration (not as a drop-in replacement for XFoil).

## License

MIT. See the repository `LICENSE` file.
