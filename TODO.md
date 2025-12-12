## TODO

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
