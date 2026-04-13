# FoilRs Codebase Audit — 2026-04-13

Full audit of the FoilRs project: architecture, solver fidelity, test
coverage, Bevy viewer capabilities, and integration readiness.

---

## Architecture

| Crate | Source lines | Test lines | Purpose |
|---|---|---|---|
| `foil_rs` | ~1,965 | 297 (17 tests) | Core solver — panel method, boundary layer, polar sweep |
| `foil_rs_bevy` | ~5,000 | 0 | Bevy viewer — 4 visualization modes, UI controls |

Clean separation. Core solver has zero Bevy dependency — usable as a
standalone library via `foil_rs = "0.x"` from crates.io.

- **Rust:** 1.92.0 (pinned in rust-toolchain.toml)
- **Edition:** 2024
- **Bevy:** 0.18.0 (with experimental Feathers theme)
- **License:** MIT
- **External math deps:** glam only (linear algebra hand-rolled)

---

## Core Solver: `foil_rs`

### Module Structure

```
src/
  lib.rs                          (14 lines)  Public API re-exports
  math.rs                          (1 line)   Re-exports glam::Vec2
  airfoil/
    mod.rs                         (6 lines)  Geometry exports
    naca.rs                      (193 lines)  NACA 4-digit geometry generation
  solvers/
    mod.rs                         (7 lines)  Solver re-exports
    panel/
      mod.rs                     (940 lines)  Panel method, LU solver, Cp calculation
      panels.rs                   (47 lines)  Panel struct, panel building
      geometry.rs                  (4 lines)  Re-exports airfoil geometry
      tests.rs                    (88 lines)  8 unit tests
    boundary_layer.rs            (196 lines)  Integral BL estimate, transition/separation
    polar.rs                     (473 lines)  Polar sweep, multi-threading, CSV export
  state/
    mod.rs                       (114 lines)  NacaParams, FlowSettings, reference helpers
examples/
  headless.rs                     (60 lines)  Single-point solve
  export_polar_csv.rs            (201 lines)  Batch polar sweep → CSV
  export_multi_polars_csv.rs     (168 lines)  Multiple Re/M curves → CSV
  bench_headless.rs              (311 lines)  Performance benchmarking
tests/
  aero_tests.rs                  (209 lines)  9 integration tests
```

### Panel Method

**Type:** Constant-strength vortex panel method with line source + line vortex
singularities.

**Implementation** (`solvers/panel/mod.rs`, 940 lines):
- Collocation points offset 1e-4 inward from panel midpoints
- Kutta condition: identifies TE panels via tangent direction and x-coordinate,
  enforces zero circulation difference at trailing edge
- LU factorization with partial pivoting (`lu_factorize`, lines 682-727),
  forward/back substitution (`lu_solve`, lines 729-769)
- `PanelLuSystem`: caches factorization for repeated solves across alpha —
  geometry/matrix reused, only RHS changes per alpha point

**Cp calculation** (lines 420-450):
- Samples off-surface perpendicular to airfoil at SURFACE_SAMPLE_EPS = 1e-4
- Clamped to [-3.0, 2.0] for numerical stability
- Uses `induced_velocity_from_solution()` for perturbation field

**Fallback** (lines 847-937):
- Cheap analytic model (freestream + bound quarter-chord vortex)
- Triggered when main solver fails (singular geometry) or in Approx mode
- Sets `used_fallback: true` on results

**Velocity singularities** (lines 771-817):
- `line_source_velocity()`: tangential + normal from line source (atan2/log)
- `line_vortex_velocity()`: circulation-induced field (atan2/log)

### Boundary Layer

**Method:** Integral estimate (Thwaites-like).

**File:** `solvers/boundary_layer.rs` (196 lines)

- **Input:** `PanelSolution` (Cp distribution) + `BoundaryLayerInputs` (Re, M,
  viscous flag, trip point)
- **Output:** `BoundaryLayerResult` (cd_profile, transition locations,
  separation locations, stall flag)
- **Transition:** Free (e^N heuristic: Crit = 1.174 * (1 + 22400/Re_x)) or
  forced at user-specified x
- **Separation:** lambda = theta^2 * dU_e/ds / nu < -0.09
- **Skin friction:** Laminar C_f = 0.664/sqrt(Re_x), turbulent
  C_f = 0.455/(log10(Re_x))^2.58. Zero in separated regions.
- **Stall heuristic:** `probable_stall` when separation in 20-95% chord range
- **Compressibility:** Prandtl-Glauert beta = sqrt(1 - M^2), clamped [0.05, 1.0]

**Key limitation:** This is a post-hoc estimate, not a coupled
viscous-inviscid iteration. The inviscid Cp drives the BL, but the BL
displacement thickness does not feed back into the panel solution. Real XFoil
iterates until the viscous and inviscid solutions converge.

### Polar Sweep

**File:** `solvers/polar.rs` (473 lines)

- `compute_polar_sweep()`: single-threaded baseline
- `compute_polar_sweep_with_system()`: reuses LU factorization
- `compute_polar_sweep_parallel_with_system()`: multi-threaded via
  `std::thread::scope()` (no external threading crate)
- Deterministic alpha sampling via integer steps (avoids float drift)
- Thread count auto-detected, clamped to sample count

### Airfoil Geometry

**Supported:** NACA 4-digit only.

- `build_naca_body_geometry_sharp_te()`: sharp TE for panel methods (preferred)
- `build_naca_body_geometry()`: rounded TE (visual)
- `camber_line(m, p, x)`, `camber_slope(m, p, x)`, `thickness_distribution(t, x)`
- Cosine spacing for LE clustering
- `NacaParams::from_naca4(code) -> Option<Self>`: validates 4 ASCII digits

**Not supported:**
- NACA 5-digit (23012, 23015, etc.)
- Arbitrary coordinate import (.dat files)
- Spline fitting / normalization for noisy geometry

### Error Handling

- No `unwrap()` in core solver (verified)
- Singular LU factorization returns `Option::None`, triggers fallback
- Cp clamped to prevent NaN propagation
- Geometric failures (bad panel counts) caught with early return + approx
  coefficients

### Public API

```rust
// Geometry
pub fn build_naca_body_geometry_sharp_te(params: &NacaParams) -> Vec<Vec2>
pub fn camber_line(m: f32, p: f32, x: f32) -> f32
pub fn camber_slope(m: f32, p: f32, x: f32) -> f32
pub fn thickness_distribution(t: f32, x: f32) -> f32

// State
pub struct NacaParams { m_digit, p_digit, t_digits, num_points }
pub struct FlowSettings { alpha_deg, reynolds, mach, viscous, free_transition }
pub fn NacaParams::from_naca4(code: &str) -> Option<Self>

// Panel solver
pub fn compute_panel_solution(params: &NacaParams, alpha_deg: f32) -> PanelSolution
pub struct PanelLuSystem { /* cached factorization */ }
impl PanelLuSystem {
    pub fn new(params: &NacaParams) -> Option<Self>
    pub fn solve_flow(&self, alpha_deg: f32) -> Option<PanelFlow>
    pub fn panel_solution(&self, params: &NacaParams, alpha_deg: f32) -> PanelSolution
}

// Polar sweep
pub fn compute_polar_sweep(...) -> Vec<PolarRow>
pub fn compute_polar_sweep_parallel_with_system_mode(...) -> PolarSweepResult

// Boundary layer
pub fn estimate_boundary_layer(solution: &PanelSolution, inputs: &BoundaryLayerInputs)
    -> Option<BoundaryLayerResult>

// Results
pub struct PanelSolution { x, cp_upper, cp_lower, upper_coords, lower_coords }
impl PanelSolution { pub fn cl(&self) -> f32; pub fn cm_c4(&self) -> f32 }
pub struct PolarRow { alpha_deg, cl, cm_c4, cd_profile, probable_stall }
pub struct BoundaryLayerResult { cd_profile, transition_upper/lower, separation_upper/lower, probable_stall }
```

---

## Bevy Viewer: `foil_rs_bevy`

### Visualization Modes

1. **Field view** (`views/field_view.rs`, 283 lines) — Velocity vectors
   (arrows at grid points) + streamlines + airfoil silhouette. Responds to
   alpha, Re, Mach.

2. **Cp(x) view** (`views/cp_view.rs`, 212 lines) — Upper/lower Cp curves,
   color-matched to airfoil surface segments. Axis labels dynamically refreshed.

3. **Polars view** (`views/polar_view.rs`, 308 lines) — CL (green), CM (blue),
   CDp (red) vs alpha. Stall markers. Panel vs Approx mode toggle. Configurable
   sweep range, step, thread count.

4. **Panels view** (`views/panel_view.rs`, 93 lines) — Panel midpoints and
   normals. Useful for debugging geometry convergence.

### Controls

- **NACA digits:** m, p, t via sliders or typed input
- **Alpha:** slider or typed (-10 to +20 deg)
- **Reynolds:** slider (0.5M to 5M)
- **Mach:** slider (0.0 to 0.3)
- **Viscosity:** on/off toggle
- **Transition:** free (e^N) vs forced (trip at x=0.05)
- **Input mode:** slider-only or type-only
- **Theme:** colorful or XFoil monochrome

### Export

- CSV polar export to `exports/polar_<naca>_Re<>e6_M<>.csv`
- Auto-named, includes all sweep parameters in header
- No .dat coordinate export
- No .polar.json export

### Caching

`VizCache` struct caches geometry, LU factorization, and visualization
primitives keyed by composite hashes of (NACA params, flow state, view mode).
Cache invalidates automatically when any key component changes.

### CLI Arguments

**None.** The Bevy viewer has no command-line arguments. Starts with hardcoded
defaults: NACA 2412, alpha=4 deg, Re=1e6, M=0.1.

The headless examples in the core crate accept CLI args for scripted use.

---

## Test Coverage

### Unit Tests (8, `solvers/panel/tests.rs`)

1. `cl_sign_matches_alpha` — CL sign consistency at +/-4 deg
2. `cl_scales_with_alpha` — CL magnitude grows with alpha
3. `cl_snapshot_alpha0_print` — debug output
4. `pg_scaling_increases_induced_velocity_with_mach` — PG scaling at M=0.6
5. `odd_even_panel_counts_should_match` — 160 vs 161 points invariance
6. (3 more assertions embedded in above)

### Integration Tests (9, `tests/aero_tests.rs`)

1. `reference_values_match_xfoil` — NACA 2412 @ 0 deg: CL=0.2554, CM=-0.0557
2. `panel_solver_reports_lift_positive_at_positive_alpha` — CL(4) > 0.05
3. `panel_solver_reports_lift_negative_at_negative_alpha` — CL(-4) < -0.3
4. `naca2412_at_alpha0_matches_reference_coarsely` — CL within +/-0.15
5. `boundary_layer_returns_profile_drag` — CDp > 0.0005 at Re=1M
6. `cp_upper_is_more_negative_near_le_at_positive_alpha` — suction peak
7. `cp_lower_is_more_negative_near_le_at_negative_alpha` — antisymmetry
8. `naca0012_alpha0_has_near_zero_lift` — symmetric foil CL ~ 0
9. `naca0012_lift_is_approximately_antisymmetric_inviscid` — CL(4) + CL(-4) ~ 0

### Assessment

| Area | Coverage | Notes |
|---|---|---|
| CL sign/symmetry | Good | Tested at multiple alpha, symmetric + cambered |
| XFoil reference | 1 point only | NACA 2412 @ 0 deg — need 10+ points |
| Boundary layer | Basic | CDp exists, no stall at mild alpha |
| Polar sweep | Not tested | No convergence or multi-point validation |
| High alpha | Not tested | No tests above ~4 deg |
| Edge-case geometry | Not tested | Unusual NACA codes, thin/thick extremes |
| Bevy viewer | Manual only | No automated UI tests |

---

## Performance Characteristics

| Operation | Estimated time | Notes |
|---|---|---|
| Single solve (160 panels, 1 alpha) | ~1-10 ms | LU solve dominates (O(n^3)) |
| 51-point polar (0.5 deg steps) | ~500 ms single-thread | ~100-150 ms with 4 threads |
| Memory per solve | ~640 KB | 160x160 float matrix + pivots |
| Polar sweep memory | ~20 KB | 51 PolarRow structs |

Multi-threading speedup is near-linear up to thread count.

---

## Code Quality

### Strengths

- No `unwrap()` in core solver — graceful fallback throughout
- Clean library/viewer separation — core is a standalone library
- Allocation-free BL integration in sweeps (performance-conscious)
- Cached LU factorization (PanelLuSystem) avoids redundant work
- Consistent formatting (cargo fmt + git pre-commit hook)
- No unsafe code in solver
- Doc comments on public functions

### Weaknesses

- Complex math (Biot-Savart, LU decomposition) lacks derivation references
  in comments
- No architecture doc explaining coordinate frames or sign conventions
- Cp sign handling in panel/mod.rs (lines 455-458) requires careful reading
- TODO.md notes a potential double-rotation bug in field view (not confirmed)

---

## What's Missing for Aircraft Builder Integration

These are ordered by priority for the integration described in
`plan-foilrs-integration.md`:

### Blockers (required before Phase 1)

| # | Gap | Impact |
|---|---|---|
| 1 | **NACA 5-digit support** | Presets use 23012, 23015 — Phase 1 only works on 4-digit airfoils without this |
| 2 | **Arbitrary coordinate support in public API** | The solver internally works with `Vec<Vec2>`, but the public API only exposes `NacaParams` entry points. Need `PanelLuSystem::from_coordinates(coords: &[Vec2])` |
| 3 | **Multi-point XFoil validation** | Before trusting polars for flight model input, need 5-10 reference points across different airfoils, Re, and alpha ranges |

### Required for Custom Airfoil Workflow

| # | Gap | Impact |
|---|---|---|
| 4 | **.dat file import** | Standard Selig/Lednicer format — needed for `AirfoilSource::Coordinates` path |
| 5 | **CLI args on foil_rs_bevy** | `--import <path>` and `--export <path>` flags for process-spawn integration |
| 6 | **.polar.json export** | `SectionPolar` JSON format for pre-computed polars |
| 7 | **.dat coordinate export** | Standard Selig format for mesh generation |

### Desirable for Production Quality

| # | Gap | Impact |
|---|---|---|
| 8 | Viscous-inviscid coupling iteration | Stall prediction accuracy — currently post-hoc BL only |
| 9 | Separation bubble model | Thin airfoils at low Re (common in UAV/model aircraft) |
| 10 | Transonic drag rise | Only Prandtl-Glauert — no shock-BL interaction |

---

## Scripts and Assets

**Scripts:**
- `scripts/setup-git-hooks.sh` (5 lines) — enables pre-commit cargo fmt
- `scripts/release-check.sh` (62 lines) — fmt, clippy, test, optional
  publish/build. Supports `--publish-dry-run`, `--release-binary`,
  `--allow-dirty`

**Assets:**
- `assets/views_img/` — 5 PNG screenshots for README (Field, Cp, Polars,
  Panels, Headless)

---

## Coding Standards: `cargo qa`, `cargo cov`, `cargo val`

FoilRs should adopt the same three-command workflow used in the aircraft
builder project. These are cargo aliases defined in `.cargo/config.toml` that
give a single command for quality checks, coverage, and solver validation.

### `.cargo/config.toml`

```toml
[alias]
qa  = "run -p foil_rs --example qa_check"
cov = "llvm-cov --workspace --all-targets --html --open -- --test-threads=1"
val = "run -p foil_rs --example validate_reference --release"
```

### `cargo qa` — Quality Assurance Gate

A single command that runs all quality checks in sequence, reports pass/fail,
and exits non-zero on any failure. Suitable for CI and pre-push hooks.

**Checks (4 steps):**

1. **`cargo fmt --all --check`** — formatting. Zero tolerance.
2. **`cargo clippy --workspace --lib -- -D clippy::unwrap_used -D clippy::todo`**
   — strict lint on library code. Deny `unwrap()` and `todo!()` in solver.
   Test code and examples are allowed more latitude.
3. **`cargo test --workspace`** — all tests must pass.
4. **`cargo test --workspace -- --ignored`** — run ignored/slow tests
   (validation suite, if gated behind `#[ignore]`).

**Implementation:** `examples/qa_check.rs` (or a build script that shells out
to cargo subcommands, same pattern as `release-check.sh` but in Rust for
cross-platform support).

**Exit code:** 0 = all passed, 1 = at least one failure. Summary printed to
stderr.

### `cargo cov` — Code Coverage

Generates an HTML coverage report using `cargo-llvm-cov`.

**Prerequisites:**
```sh
cargo install cargo-llvm-cov
rustup component add llvm-tools-preview
```

**What it covers:**
- `foil_rs` crate (core solver) — this is the primary target
- `foil_rs_bevy` is excluded from coverage (UI code, tested manually)

**Target:** Aim for >80% line coverage on the core solver. Current estimate
is ~60% based on test count vs source lines. The main gaps are:
- `polar.rs` multi-threading paths
- `boundary_layer.rs` separation and stall branches
- Fallback paths in `panel/mod.rs`

### `cargo val` — Solver Validation Against Reference Data

Runs the FoilRs solver against a curated database of reference airfoil data
(from XFoil, published NACA reports, and wind tunnel measurements) and
generates a comparison report.

This is the most important of the three commands. Without validation against
known-good data, the solver's output can't be trusted for downstream use.

#### Reference Database: `foil_validation_data/`

A data crate (or `data/` directory with `include_str!`) containing reference
polars and single-point coefficients for well-studied airfoils:

```
foil_validation_data/
  data/
    source_index.json              ← bibliography of data sources
    airfoils/
      naca_0012.json               ← symmetric baseline
      naca_2412.json               ← cambered GA airfoil
      naca_23012.json              ← 5-digit, C172 wing section
      naca_4412.json               ← high-camber GA
      naca_0009.json               ← thin symmetric (tail sections)
      naca_63_215.json             ← laminar-flow 6-series (stretch goal)
      clark_y.json                 ← vintage GA airfoil (stretch goal)
  src/
    lib.rs                         ← include_str!, typed structs, accessors
```

#### Reference Data Per Airfoil

Each JSON file contains reference data at multiple conditions:

```json
{
  "meta": {
    "id": "naca_2412",
    "label": "NACA 2412",
    "family": "4-digit",
    "thickness_pct": 12,
    "max_camber_pct": 2,
    "camber_position_pct": 40
  },
  "sources": ["abbott_1959", "xfoil_6.99"],
  "reference_points": [
    {
      "id": "alpha0_re1m",
      "alpha_deg": 0.0,
      "reynolds": 1000000,
      "mach": 0.0,
      "cl": 0.2554,
      "cm_c4": -0.0557,
      "cd": 0.0068,
      "source": "xfoil_6.99",
      "notes": "Ncrit=9, 160 panels, converged"
    },
    {
      "id": "alpha4_re3m",
      "alpha_deg": 4.0,
      "reynolds": 3000000,
      "mach": 0.0,
      "cl": 0.6350,
      "cm_c4": -0.0601,
      "cd": 0.0059,
      "source": "xfoil_6.99",
      "notes": null
    },
    {
      "id": "alpha8_re3m",
      "alpha_deg": 8.0,
      "reynolds": 3000000,
      "mach": 0.0,
      "cl": 1.0100,
      "cm_c4": -0.0638,
      "cd": 0.0073,
      "source": "xfoil_6.99",
      "notes": null
    },
    {
      "id": "clmax_re3m",
      "alpha_deg": 16.0,
      "reynolds": 3000000,
      "mach": 0.0,
      "cl": 1.5200,
      "cm_c4": -0.0490,
      "cd": 0.0210,
      "source": "abbott_1959",
      "notes": "Approximate CL_max from Abbott & von Doenhoff Fig. 4.11"
    }
  ],
  "polar_curves": [
    {
      "id": "polar_re3m_inviscid",
      "reynolds": 3000000,
      "mach": 0.0,
      "viscous": false,
      "source": "xfoil_6.99",
      "alpha_deg": [-8, -4, 0, 4, 8, 12],
      "cl":       [-0.62, -0.19, 0.26, 0.64, 1.01, 1.35],
      "cm_c4":    [-0.055, -0.055, -0.056, -0.060, -0.064, -0.068]
    }
  ]
}
```

#### Comparison Metrics and Tolerances

| Metric | Tolerance (inviscid) | Tolerance (viscous) | Notes |
|---|---|---|---|
| CL at given alpha | 5% or 0.02 absolute | 10% or 0.05 absolute | Whichever is larger |
| CM_c/4 at given alpha | 10% or 0.005 absolute | 15% or 0.01 absolute | CM is inherently noisier |
| CD_profile | N/A (inviscid has no drag) | 20% or 0.002 absolute | BL estimate is approximate |
| CL_alpha (slope) | 5% | 8% | Finite-difference from polar |
| CL_max | N/A | 15% | Stall prediction is heuristic |
| Alpha_stall | N/A | 2 deg absolute | Stall prediction is heuristic |

Tolerances are deliberately loose in the early stages. As the solver matures
and coupling iterations are added, tighten them.

#### Validation Runner

The `validate_reference` example:

1. Loads all reference datasets from `foil_validation_data`
2. For each reference point: runs the FoilRs solver at the same
   (airfoil, alpha, Re, M, viscous) condition
3. Compares solver output against reference values
4. Generates a report (text to stdout, optional HTML)

```
$ cargo val

FoilRs Validation Report — 2026-04-13
======================================

NACA 0012 (4 reference points)
  alpha=0  Re=3M  inviscid  CL: 0.0001 vs 0.0000 (ref)  PASS (Δ=0.0001)
  alpha=4  Re=3M  inviscid  CL: 0.4512 vs 0.4418 (ref)  PASS (Δ=2.1%)
  alpha=0  Re=3M  viscous   CD: 0.0071 vs 0.0061 (ref)  PASS (Δ=16%)
  alpha=12 Re=3M  viscous   CL: 1.1200 vs 1.0900 (ref)  PASS (Δ=2.8%)

NACA 2412 (6 reference points)
  alpha=0  Re=1M  inviscid  CL: 0.2554 vs 0.2554 (ref)  PASS (Δ=0.0%)
  alpha=4  Re=3M  inviscid  CL: 0.6280 vs 0.6350 (ref)  PASS (Δ=1.1%)
  ...

Summary: 18/20 within tolerance (90%)
  2 outside tolerance:
    naca_4412 alpha=14 Re=1M viscous CL: 1.48 vs 1.31 (ref) — FAIL (Δ=13%)
    naca_0012 alpha=14 Re=1M viscous CD: 0.018 vs 0.013 (ref) — FAIL (Δ=38%)
```

#### Where to Get Reference Data

| Source | Airfoils | Data type | Access |
|---|---|---|---|
| **XFoil 6.99** | Any NACA, any .dat | CL, CD, CM vs alpha at specified Re/M | Run locally, capture output. Gold standard for 2D panel+BL validation |
| **Abbott & von Doenhoff (1959)** "Theory of Wing Sections" | NACA 4-digit, 5-digit, 6-series | Wind tunnel CL, CD, CM, CL_max | Figures 4.x-7.x (digitized data available online). Industry reference |
| **UIUC Airfoil Database** (m-selig.ae.illinois.edu) | 1,600+ airfoils | Coordinates (.dat) + some polars | Free download. Covers GA, UAV, wind turbine sections |
| **Airfoil Tools** (airfoiltools.com) | Many common profiles | XFoil-generated polars at various Re | Web-scraped or manually extracted. Good for cross-checking |
| **NASA TN/TR reports** | NACA profiles | Wind tunnel data, original measurements | Public domain. Primary source for Abbott & von Doenhoff |

**Minimum viable database (ship with 0.2.0):**

| Airfoil | Points | Source | Why |
|---|---|---|---|
| NACA 0012 | 6 (alpha: -4, 0, 4, 8, 12, 16) × Re 1M, 3M | XFoil + Abbott | Symmetric baseline — any error is a solver bug |
| NACA 2412 | 6 × Re 1M, 3M | XFoil + Abbott | Cambered GA — the current test case, expand it |
| NACA 4412 | 6 × Re 1M, 3M | XFoil + Abbott | Higher camber — tests camber handling |
| NACA 0009 | 4 × Re 1M, 3M | XFoil | Thin section — tests thickness sensitivity |
| NACA 23012 | 6 × Re 3M, 6M | XFoil | 5-digit — needed for aircraft builder integration |

That's 5 airfoils × ~10 points each = ~50 reference points. Enough to catch
regressions and track solver accuracy across releases.

**Generating XFoil reference data:**

```bash
# Run XFoil for NACA 2412 at Re=3M, alpha sweep -8 to 16 deg
xfoil << EOF
naca 2412
oper
visc 3e6
pacc
exports/xfoil_naca2412_re3m.txt

aseq -8 16 0.5

quit
EOF
```

Parse the output into the JSON format above. This can be scripted with a
small Python helper in `scripts/generate_xfoil_reference.py`.

---

## Summary

| Metric | Status |
|---|---|
| Architecture | Clean 2-crate split, core is standalone library |
| Panel method | Constant-strength vortex panels, LU with partial pivoting, Kutta condition |
| Boundary layer | Integral estimate (Thwaites-like), not coupled to inviscid solution |
| Geometry | NACA 4-digit only — 5-digit and .dat import are the main gaps |
| Test coverage | 17 tests. Good sign/symmetry coverage. Limited reference validation (1 point) |
| Error handling | Excellent — no panics in solver, graceful fallback throughout |
| Performance | Fast enough for interactive use (~1-10 ms/solve, ~100 ms parallel polar) |
| Bevy viewer | 4 views, comprehensive controls, CSV export. No CLI args, no .dat I/O |
| Integration readiness | Solver API ready for library use. NACA 5-digit and .dat import needed first |
| QA workflow | Needs `cargo qa`, `cargo cov`, `cargo val` aliases and supporting infrastructure |
