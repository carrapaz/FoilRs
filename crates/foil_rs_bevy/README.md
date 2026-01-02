# `foil_rs_bevy`

Bevy UI frontend for FoilRs: an interactive airfoil playground for exploring NACA 4‑digit sections, flow field visualization, Cp(x), and simple α-sweep polars with CSV export.

This crate depends on Bevy and is intended to be used as an application crate.

## Run

From the workspace root:

```bash
cargo run -p foil_rs_bevy
```

## Development (faster iteration)

Enable Bevy dynamic linking:

```bash
cargo run -p foil_rs_bevy --features dynamic_linking
```

## Core library

The solver/geometry code lives in the `foil_rs` crate.

## License

MIT. See the repository `LICENSE` file.
