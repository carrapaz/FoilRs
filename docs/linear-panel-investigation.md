# Linear Panel Method Investigation — 2026-04-13

## Goal

Replace the constant-strength Hess-Smith panel method with a higher-order
formulation that produces smooth Cp(x) curves and accurate CL/CM without
off-surface sampling or empirical corrections.

## Motivation

The constant-strength method at its ceiling (CL 16%, CD 8%, CM 9%) has
fundamental limitations:

- **Off-surface Cp sampling** creates LE spikes — the velocity at points
  offset 1e-4 from the surface diverges near the leading edge where panel
  singularities are strongest.
- **CL_0 correction** (thin-airfoil theory) needed for cambered airfoils
  because the panel method under-predicts the camber-induced zero-lift CL
  by ~28%.
- **CM thin-airfoil fallback** needed because the Cp-integrated CM has
  alpha-dependent noise for cambered profiles.
- **Thin airfoil CL over-prediction** (NACA 0008: 24%) from
  closely-spaced panels creating cross-surface artifacts.
- **Cp(x) display spikes** requiring smoothing for the Bevy viewer.

All these trace to one root cause: the constant-strength panels have
velocity discontinuities at every junction, and evaluating velocity near
the surface amplifies these discontinuities.

## What was implemented

### Verified influence functions

All influence functions below were verified against 10⁶-point numerical
quadrature with < 1e-4 relative error.

**Source velocity** (existing, unchanged):
```
u_src = ln(r₂²/r₁²) / (4π)
v_src = (θ₂ - θ₁) / (2π)
```

**Constant vortex velocity** (existing):
```
u_vort = -(θ₂ - θ₁) / (2π)
v_vort = ln(r₂²/r₁²) / (4π)
```

**Linear vortex ramp velocity** (new, 0 at start → 1 at end):
```
u_ramp = -(1/(2πL)) × (x·atan_term + 0.5·y·ln_term)
v_ramp =  (1/(2πL)) × (-0.5·x·ln_term + y·atan_term - L)
```
**Note**: u_ramp has a leading minus sign that was initially missing,
causing the first implementation attempt to produce CL with wrong sign.

**Linear vortex left/right decomposition**:
```
V_left  = V_const - V_ramp   (strength 1 at start, 0 at end)
V_right = V_ramp              (strength 0 at start, 1 at end)
```

**Source potential** (verified):
```
φ_src = (1/(4π)) × [x·ln(r₁²) - x₂·ln(r₂²) - 2L + 2y·atan_term]
```

**Constant doublet potential** (verified):
```
φ_doublet = -atan_term / (2π)
```

### Matrix formulations attempted

#### 1. Linear vortex only (no sources)

**Matrix**: (N+1)×(N+1). Unknowns: γ₀..γ_N. Kutta: γ₀ + γ_N = 0.
Normal BC at N panel collocation points.

**Result**: CL = 43%, CD = 58%. The vortex-only method cannot separate
thickness from circulation. Gamma values reach ±70 near the LE
(extreme) trying to represent both effects with one singularity type.
The KJ circulation integral Γ = ∫γds gave CL = 8 at alpha = 0 for a
symmetric airfoil.

**Conclusion**: sources are essential for thickness representation.

#### 2. Morino coupling (σ = dγ/ds)

**Matrix**: (N+1)×(N+1). Unknowns: γ₀..γ_N. Source derived as
σ_j = -(γ_{j+1} - γ_j) / L_j. Normal BC + Kutta.

**Result**: gamma oscillates ±60, CL diverges. The coupling produces
source strengths proportional to the gamma gradient. Near the LE where
gamma changes rapidly, source strengths are ~100+.

**Conclusion**: σ = dγ/ds is the **doublet-source** equivalence from
potential theory (σ = ∂μ/∂n for doublets), not a vortex-source
relationship. The identity doesn't apply to velocity-based vortex panels.

#### 3. Independent source + linear vortex (2N+1 matrix)

**Matrix**: (2N+1)×(2N+1). Unknowns: σ₀..σ_{N-1}, γ₀..γ_N.
N normal BCs + N tangential BCs + 1 Kutta.

**Result**: CL = 0.0002 (essentially zero). The tangential BC
`V_t(surface) = -V∞·t` overconstains the system — the solver forces
the total velocity to zero at every panel, giving Cp = 1 everywhere.

**Conclusion**: in inviscid flow, the tangential velocity is NOT
prescribed — it's free. The tangential BCs are wrong. Without them,
the system has N+1 equations for 2N+1 unknowns (underdetermined).

#### 4. Dirichlet with constant doublets + wake

**Matrix**: N×N (body doublets) or (N+1)×(N+1) (with wake unknown).
Source prescribed (σ = V∞·n, Morino). Dirichlet BC: φ_internal = 0
at each panel midpoint with ±0.5 self-influence.

**Result**: μ distribution ≈ -V∞·x (cancels freestream potential).
μ_wake ≈ 0 at all alpha. CL = 0. CM correct: -0.048 for NACA 0012,
-0.053 for NACA 2412 (reference -0.053).

**Key finding**: the doublet gradient dμ/ds produces the correct
non-lifting velocity field. CM from the doublet gradient is accurate
(1% for cambered airfoils). But the wake coupling does not produce
circulation because:
- The wake potential atan2(y, x) from the TE is ±0.48 at most panels
  (nearly equal to self-influence 0.5)
- The solver finds a solution where the wake contribution is absorbed
  into the body doublet distribution without needing μ_wake ≠ 0
- The missing piece is the **wake branch cut** — the potential must
  JUMP across the wake line, not be evaluated at a single point

**Conclusion**: the Dirichlet formulation is correct for the non-lifting
part but requires explicit wake panel(s) with proper branch-cut handling
to generate circulation.

#### 5. Neumann with prescribed source + linear vortex

**Matrix**: (N+1)×(N+1). Unknowns: γ₀..γ_N. Source prescribed
(σ = V∞·n). N normal BCs + 1 Kutta (γ₀ + γ_N = 0).

**Result**: matrix is SINGULAR. The LU factorization fails because
the system has a null space — adding a constant to all γ values doesn't
change any velocity (since V_t depends on dγ/ds, not γ itself).

**Conclusion**: the prescribed-source Neumann formulation needs an
additional constraint to pin the absolute γ level (e.g. γ_{LE} = 0).
But this is ad hoc and may introduce other issues.

### Additional infrastructure implemented

- **f64 LU factorization and solve** — for the Dirichlet formulation
  where the perturbation (μ_lift ~ 0.07) is small relative to the base
  doublet value (μ_body ~ -1.0). Implemented in `lu_factorize_f64` and
  `lu_solve_f64`.

- **BL entry from edge velocity** — `integrate_surface_from_ue(coords,
  ue, inputs)` accepts V_t directly instead of going through Cp→ue
  conversion. Ready for when the linear panel method provides smooth V_t.

- **Cp display smoothing** — `PanelSolution::smoothed_cp()` applies
  2-pass moving average for the Bevy Cp(x) view. The raw Cp is preserved
  for BL computation.

- **V_t = dμ/ds + V∞·t + source_velocity** formula — the complete
  surface velocity for the Dirichlet formulation. The source velocity
  is O(N²) at runtime since σ depends on the freestream direction.

## What XFoil does differently

From studying XFoil's Fortran source (panel.f, iopan.f, hsmsol.f):

1. **Constant source + constant doublet** (not linear) for the potential
   formulation. The doublet is equivalent to a constant vortex sheet.

2. **Doublet strengths are the unknowns**, NOT vortex strengths. The
   velocity is recovered as dμ/ds (doublet gradient along the surface),
   not from the vortex influence functions.

3. **The Kutta condition is implemented via the WAKE**: a chain of
   explicit wake panels extends from the TE downstream. The wake
   doublet strength μ_wake = μ_{TE_upper} - μ_{TE_lower} is NOT an
   independent unknown — it's derived from the body panel values.

4. **The wake is a BRANCH CUT**: the potential jumps by μ_wake across
   the wake line. This is what generates circulation. Without the branch
   cut, the Dirichlet system produces only the non-lifting solution.

5. **The system is N×N** (N body panels, N doublet unknowns). One of
   the Dirichlet equations is replaced by the Kutta condition
   (μ₁ = μ_N for XFoil's panel numbering).

6. **The Newton iteration** in `HSMSOL` simultaneously solves the panel
   equations AND the boundary layer equations. The BL displacement
   thickness modifies the panel solution, and the panel solution drives
   the BL. This is the viscous-inviscid coupling that makes XFoil
   accurate for stall prediction.

## What's needed to complete the linear panel method

### Option A: Replicate XFoil's exact formulation (~2-4 weeks)

Study `panel.f` and `iopan.f` line by line. The key is the wake panel
chain and the branch-cut implementation in the doublet potential
evaluation. The Newton-iteration coupling in `hsmsol.f` is a separate
(later) effort.

### Option B: Independent source + doublet with Dirichlet (2N+1 matrix)

Use the Dirichlet BC with independent source and doublet unknowns.
The 2N+1 matrix is 4× the size of the current (N+1)² method.  The
extra equations come from the Dirichlet internal potential condition
(N equations) + N normal BCs + 1 Kutta.  Wait — that's 2N+1 equations
for 2N unknowns.  The system is actually N×N if we prescribe the
source (Morino) and use only the Dirichlet condition + Kutta.

The missing piece for this to work: **wake branch cut in the doublet
potential evaluation**.  When evaluating the doublet potential at a body
panel that is ABOVE the wake line, the wake contribution is +μ_wake/2.
Below, it's -μ_wake/2.  The current implementation evaluates the wake
as a single panel whose potential is a smooth function, missing the jump.

### Option C: Use the working constant-strength method + corrections

The current CL 16% / CD 8% / CM 9% with corrections is already usable
for the aircraft builder integration.  The linear panel upgrade can
happen later as a focused effort.

## Current state of the code

- **`linear.rs`**: contains all verified influence functions (velocity
  and potential), the Neumann assembly with prescribed sources, V_t
  computation with source velocity contribution, and field visualization
  induced velocity.  Not connected to the active solver.

- **`mod.rs`**: unchanged from main — constant-strength Hess-Smith at
  CL 16% / CD 8% / CM 9%.

- **All 58 tests pass**, validation unchanged.

## Recommendation

The linear panel method is a research-level effort that requires either
(A) replicating XFoil's Fortran, or (B) implementing the wake branch
cut from first principles.  Both need focused time.  The current solver
is production-usable for the aircraft builder.
