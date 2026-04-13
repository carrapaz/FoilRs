## TODO

### Completed
- [x] Split into a Cargo workspace: `foil_rs` (core) + `foil_rs_bevy` (UI/app).
- [x] Improve headless benchmarking harness (warmup, multi-run stats).
- [x] (Perf) Make boundary-layer integration allocation-free in sweeps.
- [x] Unify theming: rely on Feathers theme tokens + `ThemeBackgroundColor`/`ThemeBorderColor` instead of spawning hard-coded colors.
- [x] Remove UI rebuild-on-theme-toggle (preserve UI state like focus/sections; scale to more widgets).
- [x] Deduplicate common UI “pill button” spawning patterns in `crates/foil_rs_bevy/src/ui/layout/topbar.rs`.
- [x] Sweep α to generate polars (CL, CDp) with charts and CSV export.
- [x] Add headless/batch mode for scripted sweeps/CI plus benchmarks for solver performance.
- [x] Add headless examples: `crates/foil_rs/examples/headless.rs`, `crates/foil_rs/examples/bench_headless.rs`, `crates/foil_rs/examples/export_polar_csv.rs`.
- [x] Move UI-only enums out of `state` into `ui` so `foil_rs` stays a clean library API.
- [x] Introduce a cached/factorized panel-system path so polar sweeps reuse geometry/matrix across α (big perf win).
- [x] Refactor `compute_polar_sweep` to use deterministic integer steps + `Vec::with_capacity` (avoid float-drift + reallocs).
- [x] Add optional multi-threaded polar sweeps (no extra deps; uses `std::thread`).
- [x] Add an XFoil-like monochrome UI theme toggle (black/white).
- [x] Deduplicate NACA parsing logic across examples (shared `NacaParams::from_naca4` helper).
- [x] Pin dependency versions for reproducibility (`log = "*"`, etc.).
- [x] Fix flow arrows so the grid stays fixed but arrow directions align with freestream orientation.
- [x] Add viewport polish: subtle background grid, small margin around gizmo area.
- [x] Show live NACA code/header near controls and tint UI based on mode.
- [x] Split UI into modular layout files (top bar + sections).
- [x] Replace view dropdown with top-bar view tabs.
- [x] Add input-mode selector (Slider vs Type-only).
- [x] Add numeric typed inputs (NACA digits, α, Re, Mach).
- [x] Replace toy flow with proper vortex panel solver + Kutta condition.
- [x] Wire panel-based Cp(x) into the Cp view and color-match airfoil segments.
- [x] Add sanity tests (CL sign, NACA 2412 snapshot) so regressions are caught automatically.
- [x] Make the field visualization more visually pleasing and prevent it from penetrating the airfoil.
- [x] Add a boundary-layer estimate (transition/separation heuristics + Cd_profile) and surface flow-state indicator.
- [x] Support Mach/Re inputs, transition modeling, and viscosity toggles so solver inputs match real-world conditions.
- [x] Make field/Cp visualizations respond to Mach/Re/viscosity (PG scaling + viscous masking/damping).
- [x] Show panel discretization controls only in Panels view (hide geometry/flow controls there).
- [x] Expand test coverage and reference data (symmetric foils + polar sweep invariants) to keep the solver calibrated.

### Known solver issues (quantified by `cargo val`)

These are tracked by the validation framework and measured against XFoil /
Abbott & Von Doenhoff reference data.  Run `cargo val` to see current state.

**Current validation accuracy** (run `cargo val` to see latest):

| Metric | NACA 0008 | NACA 0012 | NACA 2412 | NACA 4412 | Overall |
|---|---|---|---|---|---|
| CL | 24.5% | 6.3% | 12.4% | 19.3% | **15.6%** |
| CD | 10.0% | 5.7% | 6.3% | 10.8% | **8.2%** |
| CM | 14.4% | 13.2% | 7.3% | 2.5% | **9.3%** |

At positive alpha (flight regime), cambered CL is 5-10%.

#### CL: thin airfoil over-prediction (NACA 0008: 24.5%)

For t/c < 10%, the constant-strength panels on upper and lower surfaces
are very close together, and the V_t at collocation points picks up
near-field artifacts from adjacent panels.  CL_alpha for 0008 is 8.4/rad
vs reference 6.3/rad.

**Root cause:** constant-strength panels have velocity discontinuities at
junctions.  For thin airfoils, these jumps on one surface influence the
other surface's collocation point.

**Fix:** linear-strength vortex panels (see plan below) and/or adaptive
panel spacing with more panels for thin sections.

#### CL: cambered airfoil residual (2412: 12.4%, 4412: 19.3%)

After the thin-airfoil CL_0 correction and V-I coupling, cambered CL at
positive alpha is 6-7%.  The remaining error at negative alpha (where
CL_ref is near zero) inflates the mean.

**Root cause:** inviscid panel method compared against viscous XFoil
reference.  The displacement thickness reduces effective camber by ~5-10%
at high loading.

**Fix:** V-I coupling already helps (23% → 12.4% for 2412).  Further
improvement from linear panels (smoother V_t → better CL_0).

#### CD: under-prediction at high alpha (~10% at α=8-10°)

Squire-Young + CL² increment gives good CD at low-moderate alpha but
under-predicts at high alpha where the BL thickens rapidly.

**Fix:** V-I coupling partially addresses this.  Full fix requires
both-surface transpiration (blocked by LE suction-peak artifacts on the
geometry-upper surface).

---

### Planned: higher-order panels and adaptive spacing

The constant-strength panel method has fundamental accuracy limits for thin
airfoils (t/c < 10%) and near the LE where the solution has steep gradients.
Two improvements address this systematically.

#### Phase A: Linear-strength vortex panels

**What changes:**
Replace the constant-strength vortex distribution (one γ per panel) with a
**linearly-varying** distribution (γ varies linearly from start to end of
each panel, with continuity enforced at panel junctions).

**Why this helps:**
- The constant-strength panel has a velocity discontinuity at every panel
  junction — the tangential velocity "jumps" between panels.  For thin
  airfoils where upper and lower panels are close, these jumps from one
  surface influence the other surface's collocation point, creating the
  artifacts that cause 24% CL error on NACA 0008.
- Linear-strength panels give a smooth velocity distribution along the
  surface.  The V_t at any point is continuous, and the influence functions
  decay faster with distance (smoother kernel).
- The lift (circulation) is exact for the discretisation — no need for
  the CL_0 correction or thin-airfoil corrections.

**What changes in the code:**

1. **Panel struct** — add `gamma_start` and `gamma_end` fields:
   ```rust
   pub(crate) struct Panel {
       pub(crate) start: Vec2,
       pub(crate) mid: Vec2,
       pub(crate) normal: Vec2,
       pub(crate) tangent: Vec2,
       pub(crate) length: f32,
   }
   ```
   No change to Panel itself — the γ distribution is stored in the
   solution vector, not in the geometry.

2. **Influence functions** — replace `panel_influence()`:
   - Current: `line_source_velocity` + `line_vortex_velocity` for a
     constant-strength panel.  Returns one (source, vortex) velocity pair.
   - New: `linear_panel_influence()` returns influence of a linearly-
     varying source and vortex.  For a panel from point A to B with
     strength varying from σ_A to σ_B:
     ```
     V(P) = σ_A * I₀(P, A, B) + (σ_B - σ_A) * I₁(P, A, B)
     ```
     where I₀ is the constant-strength kernel (existing) and I₁ is the
     linear kernel (new).

     **Verified ramp kernel formulas** (confirmed against 10⁶-point
     numerical quadrature, 2026-04-13):
     ```rust
     // Ramp vortex: γ = 0 at panel start, 1 at panel end.
     // In local coordinates (x along tangent, y along normal):
     u_ramp = -(1/(2πL)) * (x * atan_term + 0.5 * y * ln_term)
     v_ramp =  (1/(2πL)) * (-0.5 * x * ln_term + y * atan_term - L)

     // V_left (1→0) = V_const - V_ramp
     // V_right (0→1) = V_ramp
     ```
     Note: u_ramp has a **leading minus sign** that is easy to miss
     in the derivation (the -y/(2πL) factor from the integral).  The
     v_ramp sign matches the existing constant kernel convention.

     **Implementation notes (from multiple attempts, 2026-04-13):**

     1. The ramp kernel was verified against 10⁶-point numerical
        quadrature.  u_ramp needs a leading minus sign.

     2. **Linear vortex-only (no sources):** matrix stays (N+1)×(N+1)
        but CL was 43% error and CD 58%.  The vortex-only method cannot
        represent thickness at 160 panels — the gamma values near the
        LE reach ±70 (extreme) trying to model both thickness and
        circulation with one singularity type.  The KJ circulation
        integral Γ = ∫γds gave CL=8 at alpha=0 for a symmetric airfoil.

     3. **The correct approach is constant source + linear vortex**
        (what XFoil uses).  Sources model thickness (non-lifting),
        the linear vortex models circulation (lifting).  The matrix
        doubles to ~(2N)×(2N) with N source + N+1 node-gamma unknowns.
        This is a larger effort (~600 lines) but architecturally sound.

     4. The `integrate_surface_from_ue()` BL entry point is ready for
        direct V_t input when the linear panel method lands.

     ~30 lines for the kernel, ~600 lines total for source + linear vortex.

3. **Matrix assembly** — the matrix size changes:
   - Current: N_panels × N_panels + 1 (one source per panel + one global γ).
   - New: N_panels × N_panels + N_nodes (one source per panel + one γ per
     node, where N_nodes = N_panels for a closed body with the TE node
     shared).
   - The Kutta condition becomes: γ_TE_upper + γ_TE_lower = 0 (zero
     circulation at the trailing edge).
   - Matrix size increases from (N+1)² to ~(2N)² — about 4× more entries,
     but the LU solve is only ~8× slower (O(N³) → O((2N)³) = 8× for
     factorisation; back-substitution is O(N²) → 4×).

4. **V_t recovery** — the tangent influence matrix doubles in size
   (one column per node for the vortex, one per panel for the source).
   Per-alpha back-substitution cost ~4×.

5. **CL/CM** — with linear panels, the surface V_t is smooth and
   continuous.  The `surface_cp_from_panels` integration becomes accurate
   even at the LE.  The thin-airfoil CL_0 and CM corrections can be
   removed — the panel method gives the right answer directly.

**Implementation size:** ~300-400 lines
- Linear vortex influence function (~40 lines)
- Updated matrix assembly (~80 lines)
- Updated Kutta condition (~20 lines)
- Updated V_t recovery (~30 lines)
- Remove CL_0 and CM corrections (~-50 lines)
- Tests (~150 lines)

**Performance impact:**
- Matrix assembly: ~2× slower (twice as many influence evaluations per entry)
- LU factorisation: ~4-8× slower (matrix is ~2× larger)
- Per-alpha back-substitution: ~4× slower
- Net: single solve ~3× slower (10 ms → 30 ms), polar sweep ~4× slower
  (13 ms → 50 ms single-thread, ~15 ms parallel).  Still comfortably
  within budget.

**Expected accuracy improvement:**
- NACA 0008 CL: 24% → <5% (thin airfoil artifacts eliminated)
- NACA 0012 CL: 6% → <3% (smoother V_t at LE)
- Cambered CL: 12-19% → <8% (CL_0 correction no longer needed)
- CM: 9% → <3% (accurate CM from panel Cp, no thin-airfoil fallback)

**Risk:** the LU factorisation of the larger matrix may be less
well-conditioned.  Mitigation: use f64 for the matrix solve (the influence
functions can stay f32; only the solve needs double precision).

#### Phase B: Adaptive panel spacing

**What changes:**
Instead of uniform cosine spacing (same N_panels for all airfoils),
adapt the panel count and distribution based on thickness and curvature.

**Strategy:**

1. **Curvature-based refinement**: place more panels where the surface
   curvature is high (near the LE and at the max-thickness point).
   Estimate curvature from the NACA geometry functions:
   ```
   κ(x) = |y''| / (1 + y'²)^(3/2)
   ```
   The panel arc-length should be proportional to `1/sqrt(κ)` — shorter
   panels in high-curvature regions.

2. **Thickness-dependent total count**: thin airfoils need more panels
   because upper and lower surfaces are closer.  Heuristic:
   ```
   N_panels = max(120, 160 * (0.12 / t).sqrt())
   ```
   This gives: NACA 0008 → 196 panels, NACA 0012 → 160 (unchanged),
   NACA 0018 → 131 panels.

3. **LE clustering exponent**: the current cosine spacing uses
   `x = 0.5*(1 - cos(πβ))` where β is uniform.  A modified spacing
   `x = 0.5*(1 - cos(πβ^p))` with p > 1 concentrates more points near
   the LE.  For thin airfoils, p = 1.2-1.5 gives better LE resolution.

**Implementation:**

1. Replace `effective_num_points(params)` with:
   ```rust
   fn adaptive_num_points(params: &NacaParams) -> usize {
       let t = params.t().max(0.04);
       let base = 160.0 * (0.12 / t).sqrt();
       (base as usize).clamp(100, 320)
   }
   ```

2. Replace the geometry sampling in `build_naca_body_geometry_sharp_te`
   with curvature-adaptive spacing:
   ```rust
   fn adaptive_cosine_spacing(n: usize, le_exponent: f32) -> Vec<f32> {
       (0..n).map(|i| {
           let beta = (i as f32 / (n - 1) as f32).powf(le_exponent);
           0.5 * (1.0 - (PI * beta).cos())
       }).collect()
   }
   ```

3. Keep the existing `num_points` field on `NacaParams` as an override.
   When `num_points == 0` (or a new `adaptive: bool` flag), use the
   automatic count.

**Implementation size:** ~80-120 lines
- `adaptive_num_points()` (~10 lines)
- `adaptive_cosine_spacing()` (~15 lines)
- Modified geometry generation (~30 lines)
- Tests (~50 lines)

**Performance impact:** negligible for typical airfoils (160 panels).
Thin airfoils get ~200 panels (25% more) which costs ~1.5× in assembly
and LU, but these are the cases where accuracy improves most.

**Expected accuracy improvement:**
- NACA 0008 CL: 24% → ~15% (more panels + better LE clustering)
- NACA 0004/0006: currently very inaccurate, would become usable
- Other airfoils: <2% change (panel count stays similar)

#### Recommended order

1. **Phase B first** (adaptive spacing) — low risk, low effort (~80 lines),
   immediate benefit for thin airfoils.  Can be validated with `cargo val`
   before proceeding.

2. **Phase A second** (linear panels) — higher effort (~400 lines),
   fundamental architecture change, but eliminates the root cause of all
   remaining CL/CM accuracy issues.  The CL_0 correction, thin-airfoil CM,
   and Cp integration artifacts all disappear.

Together, these two changes would bring the solver to <5% CL error and <3%
CM error across all NACA 4-digit airfoils — XFoil-grade accuracy without
viscous-inviscid coupling.

---

### Essential for a truly useful release
- [x] Support multi-polars across Re/M (multiple curves + CSV) for XFOIL comparison (headless export + core API).
- [x] CL computed from panel-integrated forces (Kutta-Joukowski at collocation points) instead of off-surface Cp sampling.
- [x] Profile drag from Squire-Young formula (momentum thickness at TE) instead of friction-only integration.
- [x] Validation framework (`cargo val`) with reference data from Abbott & Von Doenhoff.
- [x] QA gate (`cargo qa`) with fmt, clippy strict, tests, ignored tests, validation.
- [ ] Allow importing/saving arbitrary airfoil shapes (e.g., .dat) with cosine spacing and normalization utilities.
- [ ] UI polish for production use: richer charts (Cp annotations, polar plots), presets/reset, tooltips, run/stop controls.
- [ ] Support geometry/solver diagnostics: visualize panel discretization (midpoints, normals, circulation) similar to XFOIL.
- [ ] Unify solver/plot conventions with XFoil (Cp sign/orientation, upper/lower labeling, and whether Re affects Cp).

### Performance & robustness

**Current performance** (NACA 2412, 160 panels, Apple Silicon release build):

| Operation | Time | vs XFoil |
|---|---|---|
| Single solve | 3.5 ms | 3-15x faster |
| 51-pt polar (1 thread) | 13 ms | 40-150x faster |
| 51-pt polar (8 threads) | ~3.5 ms | 140-570x faster |

**Optimisations already done:**
- [x] Fast transcendentals: `fast_atan2` (minimax) + `fast_ln` (Padé) replacing libm.
- [x] Merged `panel_influence()`: single coordinate transform for both source + vortex.
- [x] Single-pass `induced_velocity_from_solution`: merged source/vortex accumulation.
- [x] Cached tangent influence matrix: per-alpha CL/CM via matrix-vector multiply (zero transcendentals), eliminating O(N²) re-evaluation.
- [x] LU factorisation reuse across alpha in polar sweeps (`PanelLuSystem`).
- [x] Multi-threaded polar sweeps via `std::thread::scope()`.

**Remaining optimisations** (diminishing returns — solver is no longer the
bottleneck for any use case):

- [ ] **Skip Cp sampling in polar-only path** (~1.5x on polar sweep).
  The polar sweep needs CL, CD, CM — not the Cp(x) distribution.  CL/CM
  come from the cached V_t path.  CD comes from BL, which currently
  reads Cp samples.  If the BL solver were refactored to accept panel V_t
  directly (converting to Cp internally), the 160 μs/alpha Cp sampling
  step could be eliminated entirely.  ~30 lines of change.  Polar would
  drop to ~8 ms single-threaded, ~2 ms parallel.

- [ ] **Reduce Cp sample count for BL** (~1.3x on Cp path).  The BL
  integration is a smooth integral — 40 samples would likely give the
  same CD as 80.  Needs validation to confirm no accuracy regression.

- [ ] **Adaptive panel count**.  Thin airfoils (t/c < 8%) over-predict CL
  by ~30% with 160 panels.  Increasing to 240-320 panels for thin sections
  (and reducing to 80 for thick sections where the solver is already
  accurate) would improve accuracy without hurting average performance.

- [ ] **SIMD-batch matrix-vector multiply**.  The tangential-velocity
  recovery is a dense matrix-vector multiply (N×N+1 × N+1).  ARM NEON
  4-wide f32 would give ~2-3x on this step, but it's already ~20 μs —
  diminishing returns.

- [ ] **f16 far-field influence coefficients**.  For panels separated by
  > 5 chord lengths, the influence function values are small and low-
  precision storage would halve memory bandwidth.  Not worth the
  complexity at 160 panels (the full matrix fits in L1 cache).

- [ ] Validate solver stability across imported geometries (cusps, blunt trailing edges) and add fallbacks/error reporting.
- [ ] Audit coordinate-frame usage (airfoil rotation vs freestream rotation) to prevent “field lines rotate twice with α”.

### Nice-to-have ergonomics
- [ ] Adapt visualization domain to window size, add zoom/pan, and allow field-domain resizing.
- [ ] Provide undo/reset for NACA/α sliders and expose presets for common sections.
