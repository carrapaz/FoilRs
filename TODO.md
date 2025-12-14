## TODO

### Next up (code health + scalability)
- [ ] Move UI-only enums out of `src/state/` (e.g. `VisualMode`, `TableField`) into `src/ui/` so `foil_rs` stays a clean library API.
- [ ] Introduce a cached/factorized panel-system path so polar sweeps reuse geometry/matrix across α (big perf win).
- [ ] Refactor `compute_polar_sweep` to use deterministic integer steps + `Vec::with_capacity` (avoid float-drift + reallocs).
- [ ] Unify theming: rely on Feathers theme tokens + `ThemeBackgroundColor`/`ThemeBorderColor` instead of spawning hard-coded colors + respawning UI on theme toggle.
- [ ] Remove UI rebuild-on-theme-toggle (preserve UI state like focus/sections; scale to more widgets).
- [ ] Deduplicate common UI “pill button” spawning patterns in `src/ui/layout/topbar.rs`.
- [ ] Deduplicate NACA parsing logic across examples (move into a shared helper API).
- [ ] (Perf) Allow boundary-layer integration to reuse a scratch buffer / avoid repeated allocations.
- [ ] Improve headless benchmarking harness (warmup, multi-run stats, optional criterion behind dev-dep).
- [ ] Pin dependency versions for reproducibility (`log = "*"`, etc.).
- [ ] Consider splitting into `foil_rs_core` + `foil_rs_bevy` crates for long-term scalability (core solver/lib vs UI/app).

### Completed
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

### Essential for a truly useful release
- [ ] Expand test coverage and reference data (flat plate, symmetric foils, NACA 2412 across α) to keep the solver calibrated.
- [x] Sweep α to generate polars (CL, CDp) with charts and CSV export.
- [ ] Support multi-polars across Re/M (multiple curves + CSV) for XFOIL comparison.
- [ ] Allow importing/saving arbitrary airfoil shapes (e.g., .dat) with cosine spacing and normalization utilities.
- [ ] Add headless/batch mode for scripted sweeps/CI plus benchmarks for solver performance.
- [ ] UI polish for production use: richer charts (Cp annotations, polar plots), presets/reset, tooltips, run/stop controls.
- [ ] Support geometry/solver diagnostics: visualize panel discretization (midpoints, normals, circulation) similar to XFOIL.
- [ ] Unify solver/plot conventions with XFoil (Cp sign/orientation, upper/lower labeling, and whether Re affects Cp).
- [ ] Decide what drives summary CL/CM (panel-integrated vs tuned analytic “reference”) and label accordingly (maybe exposing it as an option to user?).

### Performance & robustness
- [ ] Profile and optimize matrix assembly/solve so interactive updates stay responsive with higher panel counts.
- [ ] Validate solver stability across imported geometries (cusps, blunt trailing edges) and add fallbacks/error reporting.
- [ ] Audit coordinate-frame usage (airfoil rotation vs freestream rotation) to prevent “field lines rotate twice with α”.

### Nice-to-have ergonomics
- [ ] Adapt visualization domain to window size, add zoom/pan, and allow field-domain resizing.
- [ ] Provide undo/reset for NACA/α sliders and expose presets for common sections.
