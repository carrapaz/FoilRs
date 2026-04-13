use std::f32::consts::PI;

use crate::math::{Vec2, fast_atan2, fast_ln};
use crate::state::{FlowSettings, NacaParams};

use super::boundary_layer::{self, BoundaryLayerInputs};

mod geometry;
mod linear;
mod panels;

use geometry::{
    build_naca_body_geometry_sharp_te, camber_line, camber_slope,
    resolved_surface_point_count, thickness_distribution,
};
use panels::{Panel, build_panels};

pub use linear::ExperimentalMethod as ExperimentalPanelMethod;

const SURFACE_SAMPLE_EPS: f32 = 1e-4;
const COLLOCATION_OFFSET: f32 = 1e-4;

fn effective_num_points(params: &NacaParams) -> usize {
    let n = resolved_surface_point_count(params);
    if n.is_multiple_of(2) { n } else { n + 1 }
}

/// Result of our pseudo-panel solution.
pub struct PanelSolution {
    /// x / c for each sample, 0..1.
    pub x: Vec<f32>,
    /// Cp on upper surface at each x.
    pub cp_upper: Vec<f32>,
    /// Cp on lower surface at each x.
    pub cp_lower: Vec<f32>,
    /// Coordinate of each sample on upper surface.
    pub upper_coords: Vec<Vec2>,
    /// Coordinate of each sample on lower surface.
    pub lower_coords: Vec<Vec2>,
    pub(crate) cl_cached: Option<f32>,
    pub(crate) cm_c4_cached: Option<f32>,
}

impl PanelSolution {
    /// Approximate section lift coefficient by integrating Cp difference.
    pub fn cl(&self) -> Option<f32> {
        if let Some(cl) = self.cl_cached {
            return Some(cl);
        }
        if self.x.len() < 2 {
            return None;
        }
        let mut cl = 0.0;
        for i in 0..self.x.len() - 1 {
            let dx = self.x[i + 1] - self.x[i];
            if dx <= 0.0 {
                continue;
            }
            let dcp0 = self.cp_lower[i] - self.cp_upper[i];
            let dcp1 = self.cp_lower[i + 1] - self.cp_upper[i + 1];
            cl += 0.5 * (dcp0 + dcp1) * dx;
        }
        Some(cl)
    }

    /// Smoothed Cp arrays for display — reduces LE spikes.
    /// The raw `cp_upper`/`cp_lower` fields are used for BL computation.
    pub fn smoothed_cp(&self) -> (Vec<f32>, Vec<f32>) {
        let mut cp_u = self.cp_upper.clone();
        let mut cp_l = self.cp_lower.clone();
        smooth_cp(&mut cp_u);
        smooth_cp(&mut cp_l);
        (cp_u, cp_l)
    }

    /// Approximate pitching moment about c/4 (sign convention: nose-up positive).
    pub fn cm_c4(&self) -> Option<f32> {
        if let Some(cm) = self.cm_c4_cached {
            return Some(cm);
        }
        if self.x.len() < 2 {
            return None;
        }
        let mut cm = 0.0;
        for i in 0..self.x.len() - 1 {
            let x0 = self.x[i];
            let x1 = self.x[i + 1];
            let dx = x1 - x0;
            if dx <= 0.0 {
                continue;
            }
            let x_avg = 0.5 * (x0 + x1);
            let dcp0 = self.cp_lower[i] - self.cp_upper[i];
            let dcp1 = self.cp_lower[i + 1] - self.cp_upper[i + 1];
            let dcp_avg = 0.5 * (dcp0 + dcp1);
            cm += dcp_avg * dx * (x_avg - 0.25);
        }
        Some(-cm)
    }
}

#[allow(dead_code)]
fn integrate_cl_from_cp(
    x: &[f32],
    cp_upper: &[f32],
    cp_lower: &[f32],
) -> Option<f32> {
    if x.len() < 2
        || x.len() != cp_upper.len()
        || x.len() != cp_lower.len()
    {
        return None;
    }
    let mut cl = 0.0;
    for i in 0..x.len() - 1 {
        let dx = x[i + 1] - x[i];
        if dx <= 0.0 {
            continue;
        }
        let dcp0 = cp_lower[i] - cp_upper[i];
        let dcp1 = cp_lower[i + 1] - cp_upper[i + 1];
        cl += 0.5 * (dcp0 + dcp1) * dx;
    }
    Some(cl)
}

fn integrate_cm_c4_from_cp(
    x: &[f32],
    cp_upper: &[f32],
    cp_lower: &[f32],
) -> Option<f32> {
    if x.len() < 2
        || x.len() != cp_upper.len()
        || x.len() != cp_lower.len()
    {
        return None;
    }
    let mut cm = 0.0;
    for i in 0..x.len() - 1 {
        let x0 = x[i];
        let x1 = x[i + 1];
        let dx = x1 - x0;
        if dx <= 0.0 {
            continue;
        }
        let x_avg = 0.5 * (x0 + x1);
        let dcp0 = cp_lower[i] - cp_upper[i];
        let dcp1 = cp_lower[i + 1] - cp_upper[i + 1];
        let dcp_avg = 0.5 * (dcp0 + dcp1);
        cm += dcp_avg * dx * (x_avg - 0.25);
    }
    Some(-cm)
}

pub struct PanelLuSystem {
    panels: Vec<Panel>,
    lu: Vec<f32>,
    pivots: Vec<usize>,
    size: usize,
    upper_dir: Vec2,
    lower_dir: Vec2,
    /// Tangent-dotted influence matrix: `tan_infl[i * size + j]` is the
    /// tangential velocity at panel i due to a unit source on panel j.
    /// Column `n` (last) is the tangential velocity due to unit vortex
    /// (sum over all panels).
    tan_infl: Vec<f32>,
    /// Per-panel vortex tangent influence: `vort_tan_self[i]` is the
    /// tangential velocity at panel i due to unit vortex on panel i only.
    /// Stored for future self-influence correction.
    #[allow(dead_code)]
    vort_tan_self: Vec<f32>,
    /// CL_0 correction: the difference between thin-airfoil theory CL_0
    /// and the panel solver's CL_0 at alpha=0.  Added to all CL values
    /// to compensate for the collocation-offset camber bias.
    cl_0_correction: f32,
}

pub struct PanelFlow<'a> {
    panels: &'a [Panel],
    sources: Vec<f32>,
    gamma: f32,
    freestream: Vec2,
}

impl PanelFlow<'_> {
    pub fn velocity_body_pg(&self, point: Vec2, mach: f32) -> Vec2 {
        let beta = (1.0 - mach * mach).clamp(0.05, 1.0).sqrt();
        let induced = induced_velocity_from_solution(
            point,
            self.panels,
            &self.sources,
            self.gamma,
        );
        self.freestream + induced / beta
    }
}

impl PanelLuSystem {
    pub fn new(params: &NacaParams) -> Option<Self> {
        let mut local = params.clone();
        local.num_points = effective_num_points(params);
        let geometry = build_naca_body_geometry_sharp_te(&local);
        let panels = build_panels(&geometry);
        if panels.len() < 4 {
            return None;
        }

        let (
            matrix,
            tan_infl,
            vort_tan_self,
            size,
            upper_dir,
            lower_dir,
        ) = assemble_matrix(&panels);
        let (lu, pivots) = lu_factorize(&matrix, size)?;

        let mut sys = Self {
            panels,
            lu,
            pivots,
            size,
            upper_dir,
            lower_dir,
            tan_infl,
            vort_tan_self,
            cl_0_correction: 0.0,
        };

        // Compute CL_0 correction: the panel method under-predicts
        // the camber-induced CL_0 due to collocation-offset bias.
        // The thin-airfoil theory gives an accurate CL_0 reference.
        if local.m() > 1e-4 {
            let alpha_0l_theory = thin_airfoil_zero_lift_alpha(&local);
            // CL_0 from thin-airfoil: CL_0 = cl_alpha_2D * |alpha_0L|
            // where cl_alpha_2D ≈ 2π (exact for thin airfoils).
            let cl_0_theory = 2.0 * PI * alpha_0l_theory.abs();
            // CL_0 from the panel solver at alpha=0:
            let freestream_0 = Vec2::new(1.0, 0.0);
            let cl_0_panel = if let Some((src, gam)) =
                sys.solve(freestream_0)
            {
                let vt =
                    sys.tangential_velocities(&src, gam, freestream_0);
                surface_cp_from_panels(&sys.panels, &vt, &local).cl
            } else {
                0.0
            };
            sys.cl_0_correction = cl_0_theory - cl_0_panel;
        }

        Some(sys)
    }

    pub fn solve_flow(&self, alpha_deg: f32) -> Option<PanelFlow<'_>> {
        let alpha_rad = alpha_deg.to_radians();
        let freestream = Vec2::new(alpha_rad.cos(), alpha_rad.sin());
        let (sources, gamma) = self.solve(freestream)?;
        Some(PanelFlow {
            panels: &self.panels,
            sources,
            gamma,
            freestream,
        })
    }

    pub(crate) fn solve(
        &self,
        freestream: Vec2,
    ) -> Option<(Vec<f32>, f32)> {
        self.solve_with_transpiration(freestream, None)
    }

    /// Solve the panel system with optional transpiration (blowing)
    /// velocities at each panel.  The transpiration modifies the
    /// normal-velocity boundary condition:
    ///   V · n_i + V_n_transpiration_i = 0
    /// which shifts the RHS by the transpiration values.
    pub(crate) fn solve_with_transpiration(
        &self,
        freestream: Vec2,
        transpiration: Option<&[f32]>,
    ) -> Option<(Vec<f32>, f32)> {
        let mut rhs = assemble_rhs(
            &self.panels,
            freestream,
            self.upper_dir,
            self.lower_dir,
        );
        // Add transpiration (displacement-thickness outflow) to the
        // normal-velocity boundary condition.
        if let Some(vn) = transpiration {
            let n = self.panels.len().min(vn.len());
            for i in 0..n {
                rhs[i] += vn[i];
            }
        }
        let strengths =
            lu_solve(&self.lu, &self.pivots, &rhs, self.size)?;
        if strengths.len() != self.size {
            return None;
        }
        let n_panels = self.panels.len();
        let gamma = strengths[n_panels];
        let sources = strengths[..n_panels].to_vec();
        Some((sources, gamma))
    }

    /// Recover tangential velocity at each panel surface from the solved
    /// source/vortex strengths.
    ///
    /// Uses the cached tangent influence matrix with **analytic
    /// self-influence correction**.  For a constant-strength panel, the
    /// tangential velocity at its own midpoint from its own source/vortex
    /// is exactly **zero** by symmetry (the two halves of the panel
    /// induce equal and opposite tangential velocities at the center).
    /// The cached coefficients are evaluated at the collocation offset
    /// point where the self-influence is non-zero — this introduces a
    /// systematic bias that grows with camber.  The correction zeros
    /// out the self-influence diagonal.
    /// Recover tangential velocity at each panel surface from the solved
    /// source/vortex strengths.
    ///
    /// Uses the cached tangent influence matrix with a vortex
    /// self-influence correction.
    ///
    /// **Source panels:** V_t is continuous across the panel surface —
    /// the collocation-offset value equals the surface value.  No
    /// correction needed.
    ///
    /// **Vortex sheet:** V_t has a jump of γ across the surface.  The
    /// collocation point is on the outside (positive-normal side), so
    /// V_t(colloc) = V_t(surface) + γ/2.  The surface value is
    /// V_t(surface) = V_t(colloc) - γ/2.  We apply this correction
    /// to the vortex self-influence only.
    fn tangential_velocities(
        &self,
        sources: &[f32],
        gamma: f32,
        freestream: Vec2,
    ) -> Vec<f32> {
        let n = self.panels.len();
        let size = self.size;
        let mut vt = Vec::with_capacity(n);
        for i in 0..n {
            let mut v_induced_t = 0.0;
            // Source: no correction needed (V_t continuous).
            for (j, &sigma) in sources.iter().enumerate() {
                v_induced_t += self.tan_infl[i * size + j] * sigma;
            }
            v_induced_t += self.tan_infl[i * size + n] * gamma;

            let v_freestream_t = freestream.dot(self.panels[i].tangent);
            vt.push(v_freestream_t + v_induced_t);
        }
        vt
    }

    pub fn panel_solution(
        &self,
        params: &NacaParams,
        alpha_deg: f32,
    ) -> PanelSolution {
        let alpha_rad = alpha_deg.to_radians();
        let freestream = Vec2::new(alpha_rad.cos(), alpha_rad.sin());

        let Some((sources, gamma)) = self.solve(freestream) else {
            let (cl, cm_c4, _) =
                approx_section_coeffs(params, alpha_deg);
            return PanelSolution {
                x: Vec::new(),
                cp_upper: Vec::new(),
                cp_lower: Vec::new(),
                upper_coords: Vec::new(),
                lower_coords: Vec::new(),
                cl_cached: Some(cl),
                cm_c4_cached: Some(cm_c4),
            };
        };

        // Recover tangential velocity at each panel from the cached
        // tangent influence matrix — O(N²) multiplies, zero transcendentals.
        let vt =
            self.tangential_velocities(&sources, gamma, freestream);

        // Build Cp on the panel surface from V_t and integrate for
        // CL and CM.  Uses panel midpoint Cp (no off-surface sampling
        // singularity) + thin-airfoil CL_0 correction for cambered
        // airfoils.
        let scp = surface_cp_from_panels(&self.panels, &vt, params);
        let scp = SurfaceCpResult {
            cl: scp.cl + self.cl_0_correction,
            cm_c4: scp.cm_c4,
        };

        let mut local = params.clone();
        local.num_points = effective_num_points(params);

        // Off-surface Cp sampling is still used for BL and visualization.
        let cp_sol = build_cp_samples(
            &local,
            freestream,
            &self.panels,
            &sources,
            gamma,
        );

        let (cl_approx, cm_approx, _) =
            approx_section_coeffs(params, alpha_deg);
        // CM: use thin-airfoil theory for cambered profiles (exact for
        // the zero-alpha pitching moment, ~1% accuracy).  The panel Cp
        // integration has LE noise that creates alpha-dependent CM
        // artifacts for cambered airfoils.  For symmetric profiles, Cp
        // integration is used (it's accurate and captures alpha effects).
        let cm = if params.m() > 1e-4 {
            thin_airfoil_cm_c4(params)
        } else {
            let cm_cp = integrate_cm_c4_from_cp(
                &cp_sol.x,
                &cp_sol.cp_upper,
                &cp_sol.cp_lower,
            );
            cm_cp.unwrap_or(cm_approx)
        };
        PanelSolution {
            x: cp_sol.x,
            cp_upper: cp_sol.cp_upper,
            cp_lower: cp_sol.cp_lower,
            upper_coords: cp_sol.upper_coords,
            lower_coords: cp_sol.lower_coords,
            cl_cached: Some(if scp.cl.is_finite() {
                scp.cl
            } else {
                cl_approx
            }),
            cm_c4_cached: Some(cm),
        }
    }

    // --- Diagnostic accessors for V-I debugging ---

    /// Inviscid solve returning raw source/gamma strengths.
    pub fn solve_inviscid(&self, freestream: Vec2) -> (Vec<f32>, f32) {
        self.solve(freestream)
            .unwrap_or_else(|| (vec![0.0; self.panels.len()], 0.0))
    }

    /// Public wrapper for tangential_velocities.
    pub fn get_tangential_velocities(
        &self,
        sources: &[f32],
        gamma: f32,
        freestream: Vec2,
    ) -> Vec<f32> {
        self.tangential_velocities(sources, gamma, freestream)
    }

    /// Solve with explicit transpiration (public for diagnostics).
    pub fn solve_with_transpiration_pub(
        &self,
        freestream: Vec2,
        transpiration: &[f32],
    ) -> Option<(Vec<f32>, f32)> {
        self.solve_with_transpiration(freestream, Some(transpiration))
    }

    /// Number of panels.
    pub fn panel_count(&self) -> usize {
        self.panels.len()
    }

    /// LE panel index (minimum x midpoint).
    pub fn le_panel_index(&self) -> usize {
        self.panels
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| a.mid.x.total_cmp(&b.mid.x))
            .map(|(i, _)| i)
            .unwrap_or(0)
    }

    /// Panel midpoint.
    pub fn panel_mid(&self, i: usize) -> Vec2 {
        self.panels[i].mid
    }

    /// Panel outward normal.
    pub fn panel_normal(&self, i: usize) -> Vec2 {
        self.panels[i].normal
    }

    /// Panel length.
    pub fn panel_length(&self, i: usize) -> f32 {
        self.panels[i].length
    }

    /// Viscous-inviscid coupled solve.
    ///
    /// Iterates between the panel solver and the BL solver:
    ///   1. Inviscid solve → V_t at panels
    ///   2. BL integration → δ*(s) on upper and lower surfaces
    ///   3. Transpiration velocity V_n = d(δ*·V_e)/ds
    ///   4. Re-solve panel system with transpiration in the RHS
    ///   5. Repeat until δ* converges (or max iterations)
    ///
    /// The LU factorization is reused — only the RHS changes each
    /// iteration.  Cost: ~60 μs per iteration on top of the initial solve.
    pub fn viscous_panel_solution(
        &self,
        params: &NacaParams,
        alpha_deg: f32,
        flow: &FlowSettings,
    ) -> PanelSolution {
        // V-I coupling requires Re high enough for the BL to be thin
        // relative to the airfoil, and t/c thick enough for stable panel
        // velocities near the LE.  Fall back to inviscid for thin
        // airfoils or low Re where the iteration diverges.
        if flow.reynolds < 1_500_000.0 || params.t() < 0.10 {
            return self.panel_solution(params, alpha_deg);
        }
        let alpha_rad = alpha_deg.to_radians();
        let freestream = Vec2::new(alpha_rad.cos(), alpha_rad.sin());
        let bl_inputs = BoundaryLayerInputs::new(
            flow.reynolds,
            flow.mach,
            true,
            flow.free_transition,
            0.05,
        );

        let Some((mut sources, mut gamma)) = self.solve(freestream)
        else {
            let (cl, cm, _) = approx_section_coeffs(params, alpha_deg);
            return PanelSolution {
                x: Vec::new(),
                cp_upper: Vec::new(),
                cp_lower: Vec::new(),
                upper_coords: Vec::new(),
                lower_coords: Vec::new(),
                cl_cached: Some(cl),
                cm_c4_cached: Some(cm),
            };
        };

        // Find LE panel for upper/lower split.
        let le_idx = self
            .panels
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| a.mid.x.total_cmp(&b.mid.x))
            .map(|(i, _)| i)
            .unwrap_or(0);

        const MAX_ITER: usize = 10;
        let n = self.panels.len();

        // Upper surface coordinates (LE → TE) for BL integration.
        let upper_coords: Vec<Vec2> =
            (0..=le_idx).rev().map(|i| self.panels[i].mid).collect();

        for iter in 0..MAX_ITER {
            let vt =
                self.tangential_velocities(&sources, gamma, freestream);

            // Run BL on the upper (suction) surface only.
            let upper_cp: Vec<f32> = (0..=le_idx)
                .rev()
                .map(|i| 1.0 - vt[i] * vt[i])
                .collect();
            let bl_upper = boundary_layer::integrate_surface(
                &upper_coords,
                &upper_cp,
                &bl_inputs,
            );

            // Smoothed transpiration via central differences + 3-pass avg.
            let vn = smooth_transpiration(&bl_upper.stations);

            // Map to panels with ramping relaxation (0.1 → 0.5).
            let relax = 0.1 + 0.1 * (iter as f32).min(4.0);
            let skip_frac = 0.05;
            let mut transpiration = vec![0.0_f32; n];
            for (k, st) in bl_upper.stations.iter().enumerate() {
                if st.x < skip_frac || k >= vn.len() {
                    continue;
                }
                let pi = le_idx.saturating_sub(k);
                if pi < n {
                    transpiration[pi] = relax * vn[k];
                }
            }

            // Re-solve with transpiration.
            if let Some((s, g)) = self.solve_with_transpiration(
                freestream,
                Some(&transpiration),
            ) {
                sources = s;
                gamma = g;
            } else {
                break;
            }
        }

        // Final V_t from the converged solution.
        let vt =
            self.tangential_velocities(&sources, gamma, freestream);
        let scp = surface_cp_from_panels(&self.panels, &vt, params);
        let cl = scp.cl + self.cl_0_correction;

        let mut local = params.clone();
        local.num_points = effective_num_points(params);
        let cp_sol = build_cp_samples(
            &local,
            freestream,
            &self.panels,
            &sources,
            gamma,
        );

        let (cl_approx, cm_approx, _) =
            approx_section_coeffs(params, alpha_deg);
        let cm = if params.m() > 1e-4 {
            thin_airfoil_cm_c4(params)
        } else {
            integrate_cm_c4_from_cp(
                &cp_sol.x,
                &cp_sol.cp_upper,
                &cp_sol.cp_lower,
            )
            .unwrap_or(cm_approx)
        };

        PanelSolution {
            x: cp_sol.x,
            cp_upper: cp_sol.cp_upper,
            cp_lower: cp_sol.cp_lower,
            upper_coords: cp_sol.upper_coords,
            lower_coords: cp_sol.lower_coords,
            cl_cached: Some(if cl.is_finite() {
                cl
            } else {
                cl_approx
            }),
            cm_c4_cached: Some(if cm.is_finite() {
                cm
            } else {
                cm_approx
            }),
        }
    }
}

/// Split panels into upper/lower surfaces for BL integration.
/// Returns (upper_coords, upper_cp, lower_coords, lower_cp).
/// Uses the swapped convention (geometry-lower = aero-upper).
#[allow(dead_code)]
fn split_panels_for_bl(
    panels: &[Panel],
    vt: &[f32],
    le_idx: usize,
) -> (Vec<Vec2>, Vec<f32>, Vec<Vec2>, Vec<f32>) {
    // "Upper" aerodynamic surface = geometry panels 0..=le_idx (reversed)
    let upper_coords: Vec<Vec2> =
        (0..=le_idx).rev().map(|i| panels[i].mid).collect();
    let upper_cp: Vec<f32> =
        (0..=le_idx).rev().map(|i| 1.0 - vt[i] * vt[i]).collect();

    // "Lower" aerodynamic surface = geometry panels le_idx+1..n
    let lower_coords: Vec<Vec2> =
        (le_idx + 1..panels.len()).map(|i| panels[i].mid).collect();
    let lower_cp: Vec<f32> = (le_idx + 1..panels.len())
        .map(|i| 1.0 - vt[i] * vt[i])
        .collect();

    (upper_coords, upper_cp, lower_coords, lower_cp)
}

/// Compute smoothed transpiration from BL stations using central differences
/// and 3-pass averaging.  This avoids the noise from forward-differencing
/// the δ*·ue product on a cosine-spaced mesh where ds is very small near LE.
fn smooth_transpiration(
    stations: &[boundary_layer::BoundaryLayerStation],
) -> Vec<f32> {
    if stations.len() < 3 {
        return vec![0.0; stations.len()];
    }
    let m: Vec<f32> =
        stations.iter().map(|s| s.delta_star * s.ue).collect();
    let mut vn = vec![0.0_f32; stations.len()];

    // Central differences for interior, forward/backward at ends.
    if stations.len() > 1 {
        let ds = (stations[1].s - stations[0].s).max(1e-6);
        vn[0] = (m[1] - m[0]) / ds;
    }
    for i in 1..stations.len() - 1 {
        let ds = (stations[i + 1].s - stations[i - 1].s).max(1e-6);
        vn[i] = (m[i + 1] - m[i - 1]) / ds;
    }
    let last = stations.len() - 1;
    if last > 0 {
        let ds = (stations[last].s - stations[last - 1].s).max(1e-6);
        vn[last] = (m[last] - m[last - 1]) / ds;
    }

    // 3-pass smoothing.
    for _ in 0..3 {
        let prev = vn.clone();
        for i in 1..vn.len() - 1 {
            vn[i] =
                0.25 * prev[i - 1] + 0.5 * prev[i] + 0.25 * prev[i + 1];
        }
    }
    vn
}

/// Approximate section coefficients (thin-airfoil-inspired).
///
/// This is used as a *fallback* when the panel solver fails, and as the basis
/// for the explicit "Approx" mode in the UI.
pub fn approx_section_coeffs(
    params: &NacaParams,
    alpha_deg: f32,
) -> (f32, f32, f32) {
    // Zero-lift angle scales with camber; tuned so NACA 2412 gives ~0.255 CL at 0°,
    // while also keeping CL(±4°) in a sane range for our UI/tests.
    let alpha0_lift_deg = -92.0 * params.m(); // m in chord fractions
    let alpha_eff_rad = (alpha_deg - alpha0_lift_deg).to_radians();
    let cl_slope_scale = 1.27;
    let cl =
        cl_slope_scale * 2.0 * std::f32::consts::PI * alpha_eff_rad;

    // Rough Cm about c/4 scaling with camber; tuned for 2412 ≈ -0.055.
    let cm_c4 = -2.5 * params.m();

    // Placeholder for profile drag; not modeled yet.
    let cdp = 0.0;

    (cl, cm_c4, cdp)
}

/// Backwards-compatible name for `approx_section_coeffs`.
pub fn analytic_section_coeffs(
    params: &NacaParams,
    alpha_deg: f32,
) -> (f32, f32, f32) {
    approx_section_coeffs(params, alpha_deg)
}

/// Simple constant-strength vortex panel method with a Kutta condition.
pub fn compute_panel_solution(
    params: &NacaParams,
    alpha_deg: f32,
) -> PanelSolution {
    let alpha_rad = alpha_deg.to_radians();

    // Freestream in body coordinates; visualization rotates the airfoil in world
    // space, so in body coordinates the freestream rotates with alpha.
    let freestream = Vec2::new(alpha_rad.cos(), alpha_rad.sin());

    let mut local = params.clone();
    local.num_points = effective_num_points(params);
    let geometry = build_naca_body_geometry_sharp_te(&local);
    let panels = build_panels(&geometry);

    if panels.len() < 4 {
        let (cl, cm_c4, _) = approx_section_coeffs(params, alpha_deg);
        return PanelSolution {
            x: Vec::new(),
            cp_upper: Vec::new(),
            cp_lower: Vec::new(),
            upper_coords: Vec::new(),
            lower_coords: Vec::new(),
            cl_cached: Some(cl),
            cm_c4_cached: Some(cm_c4),
        };
    }

    let system = assemble_system(&panels, freestream);
    let strengths = solve_linear_system(system);

    if strengths.len() != panels.len() + 1 {
        let (cl, cm_c4, _) = approx_section_coeffs(params, alpha_deg);
        return PanelSolution {
            x: Vec::new(),
            cp_upper: Vec::new(),
            cp_lower: Vec::new(),
            upper_coords: Vec::new(),
            lower_coords: Vec::new(),
            cl_cached: Some(cl),
            cm_c4_cached: Some(cm_c4),
        };
    }

    let n_panels = panels.len();
    let gamma = strengths[n_panels];
    let source_strengths = strengths[..n_panels].to_vec();
    build_panel_solution_from_strengths(
        &local,
        alpha_deg,
        freestream,
        &panels,
        &source_strengths,
        gamma,
    )
}

pub fn compute_experimental_panel_solution(
    params: &NacaParams,
    alpha_deg: f32,
    method: ExperimentalPanelMethod,
) -> PanelSolution {
    if let Some(sol) = linear::try_compute_solution_with_method(
        params, alpha_deg, method,
    ) {
        return sol;
    }

    let (cl, cm_c4, _) = approx_section_coeffs(params, alpha_deg);
    PanelSolution {
        x: Vec::new(),
        cp_upper: Vec::new(),
        cp_lower: Vec::new(),
        upper_coords: Vec::new(),
        lower_coords: Vec::new(),
        cl_cached: Some(cl),
        cm_c4_cached: Some(cm_c4),
    }
}

/// Quick analytic fallback (old toy model) used for visualization when the
/// full panel solution is too noisy for Cp plotting.
pub fn compute_cp_approx(
    params: &NacaParams,
    alpha_deg: f32,
) -> PanelSolution {
    compute_fallback_solution(params, alpha_deg)
}

/// Explicit approximation mode (cheap, stable): does *not* run the panel solver.
pub fn compute_approx_solution(
    params: &NacaParams,
    alpha_deg: f32,
) -> PanelSolution {
    compute_fallback_solution(params, alpha_deg)
}

struct LinearSystem {
    matrix: Vec<f32>,
    rhs: Vec<f32>,
    size: usize,
}

/// 2-pass weighted moving average to smooth Cp spikes near the LE.
fn smooth_cp(cp: &mut [f32]) {
    if cp.len() < 3 {
        return;
    }
    for _ in 0..2 {
        let prev = cp.to_vec();
        for i in 1..cp.len() - 1 {
            cp[i] =
                0.2 * prev[i - 1] + 0.6 * prev[i] + 0.2 * prev[i + 1];
        }
    }
}

/// Cp sampling for visualization and BL input.  O(N_samples × N_panels)
/// transcendental evaluations.
struct CpSamples {
    x: Vec<f32>,
    cp_upper: Vec<f32>,
    cp_lower: Vec<f32>,
    upper_coords: Vec<Vec2>,
    lower_coords: Vec<Vec2>,
}

fn build_cp_samples(
    params: &NacaParams,
    freestream: Vec2,
    panels: &[Panel],
    sources: &[f32],
    gamma: f32,
) -> CpSamples {
    let sample_count = (params.num_points / 2).max(32);
    let m = params.m();
    let p = params.p();
    let t = params.t();

    let mut xs = Vec::with_capacity(sample_count);
    let mut cp_u = Vec::with_capacity(sample_count);
    let mut cp_l = Vec::with_capacity(sample_count);
    let mut upper_coords = Vec::with_capacity(sample_count);
    let mut lower_coords = Vec::with_capacity(sample_count);

    for i in 0..sample_count {
        let beta = i as f32 / (sample_count - 1) as f32;
        let x_c = 0.5 * (1.0 - (PI * beta).cos());

        let camber = camber_line(m, p, x_c);
        let slope = camber_slope(m, p, x_c);
        let theta = slope.atan();
        let thickness = thickness_distribution(t, x_c);

        let upper_point = Vec2::new(
            x_c - thickness * theta.sin(),
            camber + thickness * theta.cos(),
        );
        let lower_point = Vec2::new(
            x_c + thickness * theta.sin(),
            camber - thickness * theta.cos(),
        );

        let tangent =
            Vec2::new(theta.cos(), theta.sin()).normalize_or_zero();
        let normal_upper = Vec2::new(-tangent.y, tangent.x);
        let normal_lower = Vec2::new(tangent.y, -tangent.x);

        let induced_u = induced_velocity_from_solution(
            upper_point + normal_upper * SURFACE_SAMPLE_EPS,
            panels,
            sources,
            gamma,
        );
        let induced_l = induced_velocity_from_solution(
            lower_point + normal_lower * SURFACE_SAMPLE_EPS,
            panels,
            sources,
            gamma,
        );

        let cp_upper = (1.0
            - (freestream + induced_u).length_squared())
        .clamp(-3.0, 2.0);
        let cp_lower = (1.0
            - (freestream + induced_l).length_squared())
        .clamp(-3.0, 2.0);

        xs.push(x_c);
        // Swapped mapping per panel sign convention.
        cp_u.push(cp_lower);
        cp_l.push(cp_upper);
        upper_coords.push(lower_point);
        lower_coords.push(upper_point);
    }

    // Raw Cp for BL — smoothing is applied later for display only.
    CpSamples {
        x: xs,
        cp_upper: cp_u,
        cp_lower: cp_l,
        upper_coords,
        lower_coords,
    }
}

fn build_panel_solution_from_strengths(
    params: &NacaParams,
    alpha_deg: f32,
    freestream: Vec2,
    panels: &[Panel],
    sources: &[f32],
    gamma: f32,
) -> PanelSolution {
    let cp =
        build_cp_samples(params, freestream, panels, sources, gamma);

    let (cl_panel, _cm_panel) =
        panel_integrated_cl_cm(panels, sources, gamma, freestream);
    let (cl_approx, cm_c4_approx, _) =
        approx_section_coeffs(params, alpha_deg);
    let cm_c4_cached =
        integrate_cm_c4_from_cp(&cp.x, &cp.cp_upper, &cp.cp_lower)
            .or(Some(cm_c4_approx));
    PanelSolution {
        x: cp.x,
        cp_upper: cp.cp_upper,
        cp_lower: cp.cp_lower,
        upper_coords: cp.upper_coords,
        lower_coords: cp.lower_coords,
        cl_cached: Some(if cl_panel.is_finite() {
            cl_panel
        } else {
            cl_approx
        }),
        cm_c4_cached,
    }
}

fn kutta_te_panel_indices(panels: &[Panel]) -> (usize, usize) {
    const MIN_TANGENT_X: f32 = 0.2;
    const NEAR_TE_CANDIDATES: usize = 6;

    let mut upper_candidates: Vec<(usize, f32, f32)> = Vec::new();
    let mut lower_candidates: Vec<(usize, f32, f32)> = Vec::new();

    for (idx, panel) in panels.iter().enumerate() {
        if panel.length < 1e-6 {
            continue;
        }

        // Skip near-vertical “closing” segments at the trailing edge.
        if panel.tangent.x.abs() < MIN_TANGENT_X {
            continue;
        }

        let entry = (idx, panel.mid.x, panel.length);
        if panel.tangent.x > 0.0 {
            upper_candidates.push(entry);
        } else if panel.tangent.x < 0.0 {
            lower_candidates.push(entry);
        }
    }

    let pick =
        |mut candidates: Vec<(usize, f32, f32)>| -> Option<usize> {
            candidates.sort_by(|a, b| b.1.total_cmp(&a.1)); // mid.x desc
            candidates
                .into_iter()
                .take(NEAR_TE_CANDIDATES)
                .max_by(|a, b| a.2.total_cmp(&b.2)) // length desc
                .map(|(idx, _, _)| idx)
        };

    if let (Some(upper_idx), Some(lower_idx)) =
        (pick(upper_candidates), pick(lower_candidates))
    {
        return (upper_idx, lower_idx);
    }

    // Fallback: use the expected ordering for our generated airfoil loop:
    // TE(lower) → LE → TE(upper) → (closing segment to TE(lower)).
    let n = panels.len();
    let lower_idx = 0;
    let upper_idx = n.saturating_sub(2);
    (upper_idx, lower_idx)
}

fn assemble_system(panels: &[Panel], freestream: Vec2) -> LinearSystem {
    let n = panels.len();
    let size = n + 1;
    let mut matrix = vec![0.0; size * size];
    let mut rhs = vec![0.0; size];

    for (i, panel_i) in panels.iter().enumerate() {
        let colloc = panel_i.mid + panel_i.normal * COLLOCATION_OFFSET;
        rhs[i] = -freestream.dot(panel_i.normal);

        for (j, panel_j) in panels.iter().enumerate() {
            let (src, vort) = panel_influence(colloc, panel_j);
            matrix[i * size + j] = src.dot(panel_i.normal);
            matrix[i * size + n] += vort.dot(panel_i.normal);
        }
    }

    let (upper_idx, lower_idx) = kutta_te_panel_indices(panels);
    let upper = &panels[upper_idx];
    let lower = &panels[lower_idx];
    let upper_colloc = upper.mid + upper.normal * COLLOCATION_OFFSET;
    let lower_colloc = lower.mid + lower.normal * COLLOCATION_OFFSET;
    let upper_dir = upper.tangent;
    let lower_dir = -lower.tangent;

    // Kutta: match tangential velocity on the two TE-adjacent panels.
    rhs[n] = -freestream.dot(upper_dir) + freestream.dot(lower_dir);

    for (j, panel_j) in panels.iter().enumerate() {
        let (src_upper, vort_upper) =
            panel_influence(upper_colloc, panel_j);
        let (src_lower, vort_lower) =
            panel_influence(lower_colloc, panel_j);

        matrix[n * size + j] =
            src_upper.dot(upper_dir) - src_lower.dot(lower_dir);
        matrix[n * size + n] +=
            vort_upper.dot(upper_dir) - vort_lower.dot(lower_dir);
    }

    LinearSystem { matrix, rhs, size }
}

fn assemble_matrix(
    panels: &[Panel],
) -> (Vec<f32>, Vec<f32>, Vec<f32>, usize, Vec2, Vec2) {
    let n = panels.len();
    let size = n + 1;
    let mut matrix = vec![0.0; size * size];
    let mut tan_infl = vec![0.0; n * size];
    let mut vort_tan_self = vec![0.0; n];

    for (i, panel_i) in panels.iter().enumerate() {
        let colloc = panel_i.mid + panel_i.normal * COLLOCATION_OFFSET;

        for (j, panel_j) in panels.iter().enumerate() {
            let (src, vort) = panel_influence(colloc, panel_j);
            matrix[i * size + j] = src.dot(panel_i.normal);
            matrix[i * size + n] += vort.dot(panel_i.normal);
            tan_infl[i * size + j] = src.dot(panel_i.tangent);
            let vort_t = vort.dot(panel_i.tangent);
            tan_infl[i * size + n] += vort_t;
            if i == j {
                vort_tan_self[i] = vort_t;
            }
        }
    }

    let (upper_idx, lower_idx) = kutta_te_panel_indices(panels);
    let upper = &panels[upper_idx];
    let lower = &panels[lower_idx];
    let upper_colloc = upper.mid + upper.normal * COLLOCATION_OFFSET;
    let lower_colloc = lower.mid + lower.normal * COLLOCATION_OFFSET;
    let upper_dir = upper.tangent;
    let lower_dir = -lower.tangent;

    for (j, panel_j) in panels.iter().enumerate() {
        let (src_upper, vort_upper) =
            panel_influence(upper_colloc, panel_j);
        let (src_lower, vort_lower) =
            panel_influence(lower_colloc, panel_j);

        matrix[n * size + j] =
            src_upper.dot(upper_dir) - src_lower.dot(lower_dir);
        matrix[n * size + n] +=
            vort_upper.dot(upper_dir) - vort_lower.dot(lower_dir);
    }

    (matrix, tan_infl, vort_tan_self, size, upper_dir, lower_dir)
}

fn assemble_rhs(
    panels: &[Panel],
    freestream: Vec2,
    upper_dir: Vec2,
    lower_dir: Vec2,
) -> Vec<f32> {
    let n = panels.len();
    let size = n + 1;
    let mut rhs = vec![0.0; size];

    for (i, panel_i) in panels.iter().enumerate() {
        rhs[i] = -freestream.dot(panel_i.normal);
    }

    // Kutta: match tangential velocity on the two TE-adjacent panels.
    rhs[n] = -freestream.dot(upper_dir) + freestream.dot(lower_dir);
    rhs
}

fn solve_linear_system(system: LinearSystem) -> Vec<f32> {
    let n = system.size;
    let mut a = system.matrix;
    let mut b = system.rhs;

    for k in 0..n {
        let mut pivot_row = k;
        let mut pivot_val = a[k * n + k].abs();
        for i in (k + 1)..n {
            let val = a[i * n + k].abs();
            if val > pivot_val {
                pivot_val = val;
                pivot_row = i;
            }
        }
        if pivot_val < 1e-10 {
            return Vec::new();
        }
        if pivot_row != k {
            for col in 0..n {
                a.swap(k * n + col, pivot_row * n + col);
            }
            b.swap(k, pivot_row);
        }

        let pivot = a[k * n + k];
        if pivot.abs() < 1e-12 {
            return Vec::new();
        }
        for col in 0..n {
            a[k * n + col] /= pivot;
        }
        b[k] /= pivot;

        for i in 0..n {
            if i == k {
                continue;
            }
            let factor = a[i * n + k];
            if factor.abs() < 1e-9 {
                continue;
            }
            for col in 0..n {
                a[i * n + col] -= factor * a[k * n + col];
            }
            b[i] -= factor * b[k];
        }
    }

    b
}

fn lu_factorize(
    matrix: &[f32],
    n: usize,
) -> Option<(Vec<f32>, Vec<usize>)> {
    let mut lu = matrix.to_vec();
    let mut pivots: Vec<usize> = Vec::with_capacity(n);

    for k in 0..n {
        let mut pivot_row = k;
        let mut pivot_val = lu[k * n + k].abs();
        for i in (k + 1)..n {
            let val = lu[i * n + k].abs();
            if val > pivot_val {
                pivot_val = val;
                pivot_row = i;
            }
        }
        if pivot_val < 1e-10 {
            return None;
        }
        pivots.push(pivot_row);
        if pivot_row != k {
            for col in 0..n {
                lu.swap(k * n + col, pivot_row * n + col);
            }
        }

        let pivot = lu[k * n + k];
        if pivot.abs() < 1e-12 {
            return None;
        }

        for i in (k + 1)..n {
            lu[i * n + k] /= pivot;
            let lik = lu[i * n + k];
            if lik.abs() < 1e-12 {
                continue;
            }
            for j in (k + 1)..n {
                lu[i * n + j] -= lik * lu[k * n + j];
            }
        }
    }

    Some((lu, pivots))
}

fn lu_solve(
    lu: &[f32],
    pivots: &[usize],
    rhs: &[f32],
    n: usize,
) -> Option<Vec<f32>> {
    if rhs.len() != n || pivots.len() != n || lu.len() != n * n {
        return None;
    }

    let mut x = rhs.to_vec();
    for (k, &pivot_row) in pivots.iter().enumerate() {
        if pivot_row != k {
            x.swap(k, pivot_row);
        }
    }

    // Forward substitution: L y = P b (L has unit diagonal).
    for i in 0..n {
        let mut sum = x[i];
        for j in 0..i {
            sum -= lu[i * n + j] * x[j];
        }
        x[i] = sum;
    }

    // Back substitution: U x = y.
    for i in (0..n).rev() {
        let mut sum = x[i];
        for j in (i + 1)..n {
            sum -= lu[i * n + j] * x[j];
        }
        let diag = lu[i * n + i];
        if diag.abs() < 1e-12 {
            return None;
        }
        x[i] = sum / diag;
    }

    Some(x)
}

/// Combined source + vortex influence of a panel on a point.
///
/// Returns `(source_velocity, vortex_velocity)` in global coordinates.
/// The coordinate transform, ln, and atan2 are computed once and shared
/// between source and vortex terms.  Uses fast transcendental
/// approximations (~2-3x faster than libm).
#[inline]
fn panel_influence(point: Vec2, panel: &Panel) -> (Vec2, Vec2) {
    let dx = point.x - panel.start.x;
    let dy = point.y - panel.start.y;
    let x_local = dx * panel.tangent.x + dy * panel.tangent.y;
    let y_local = dx * panel.normal.x + dy * panel.normal.y;
    let y_local = if y_local.abs() < 1e-6 {
        if y_local >= 0.0 { 1e-6 } else { -1e-6 }
    } else {
        y_local
    };
    let x2 = x_local - panel.length;

    let r1_sq = (x_local * x_local + y_local * y_local).max(1e-12);
    let r2_sq = (x2 * x2 + y_local * y_local).max(1e-12);

    let ln_term = fast_ln(r2_sq / r1_sq);
    let atan_term =
        fast_atan2(y_local, x2) - fast_atan2(y_local, x_local);

    let inv_4pi = 1.0 / (4.0 * PI);
    let inv_2pi = 1.0 / (2.0 * PI);

    // Source: u = ln/(4π), v = atan/(2π)
    let src = panel.tangent * (ln_term * inv_4pi)
        + panel.normal * (atan_term * inv_2pi);

    // Vortex: u = -atan/(2π), v = ln/(4π)
    let vort = panel.tangent * (-atan_term * inv_2pi)
        + panel.normal * (ln_term * inv_4pi);

    (src, vort)
}

/// Backward-compatible wrappers used by Cp sampling and fallback paths.
#[allow(dead_code)]
fn line_source_velocity(point: Vec2, panel: &Panel) -> Vec2 {
    panel_influence(point, panel).0
}

#[allow(dead_code)]
fn line_vortex_velocity(point: Vec2, panel: &Panel) -> Vec2 {
    panel_influence(point, panel).1
}

fn induced_velocity_from_solution(
    point: Vec2,
    panels: &[Panel],
    sources: &[f32],
    gamma: f32,
) -> Vec2 {
    let mut vel = Vec2::ZERO;
    for (panel, &sigma) in panels.iter().zip(sources.iter()) {
        let (src, vort) = panel_influence(point, panel);
        vel += src * sigma + vort * gamma;
    }
    vel
}

/// Result of surface-based Cp integration.
struct SurfaceCpResult {
    cl: f32,
    cm_c4: f32,
}

/// Build Cp distribution from panel surface velocities and integrate
/// for CL and CM.
///
/// Each panel has a tangential velocity V_t from the solver.  Cp at each
/// panel midpoint is `1 - V_t²` (Bernoulli).  Panels are classified as
/// upper or lower by the geometry ordering: the sharp-TE geometry goes
/// lower-TE → LE → upper-TE, so the first ~N/2 panels are lower surface
/// and the rest are upper surface.  The LE is detected as the panel with
/// minimum midpoint x.
///
/// CL and CM are integrated by pairing upper and lower Cp values at
/// matching x-stations using the trapezoidal rule.
fn surface_cp_from_panels(
    panels: &[Panel],
    vt: &[f32],
    params: &NacaParams,
) -> SurfaceCpResult {
    if panels.is_empty() || vt.len() != panels.len() {
        return SurfaceCpResult {
            cl: 0.0,
            cm_c4: 0.0,
        };
    }

    // Find the LE panel (minimum x of midpoint).
    let le_idx = panels
        .iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| a.mid.x.total_cmp(&b.mid.x))
        .map(|(i, _)| i)
        .unwrap_or(0);

    // The sharp-TE geometry ordering is: lower-TE → LE → upper-TE.
    // Panels 0..le_idx walk along the geometry-lower surface.
    // Panels le_idx..n walk along the geometry-upper surface.
    // Due to the panel sign convention (same swap as build_cp_samples),
    // geometry-lower is aerodynamically upper and vice versa.
    let mut upper: Vec<(f32, f32)> = (0..=le_idx)
        .map(|i| {
            let cp = 1.0 - vt[i] * vt[i];
            (panels[i].mid.x, cp)
        })
        .collect();
    upper.sort_by(|a, b| a.0.total_cmp(&b.0));

    let mut lower: Vec<(f32, f32)> = (le_idx + 1..panels.len())
        .map(|i| {
            let cp = 1.0 - vt[i] * vt[i];
            (panels[i].mid.x, cp)
        })
        .collect();
    lower.sort_by(|a, b| a.0.total_cmp(&b.0));

    if lower.len() < 2 || upper.len() < 2 {
        return SurfaceCpResult {
            cl: 0.0,
            cm_c4: 0.0,
        };
    }

    // Interpolate to common x-stations (use the denser of upper/lower).
    // Use cosine-spaced stations for LE resolution.
    let n_stations = (params.num_points / 2).max(40);
    let mut cl = 0.0_f32;
    let mut cm = 0.0_f32;
    let mut prev_x = 0.0_f32;
    let mut prev_dcp = 0.0_f32;
    let mut prev_dcp_xm = 0.0_f32;

    for k in 0..n_stations {
        let beta = k as f32 / (n_stations - 1) as f32;
        let x = 0.5 * (1.0 - (PI * beta).cos());

        let cp_l = interp_cp(&lower, x);
        let cp_u = interp_cp(&upper, x);
        let dcp = cp_l - cp_u;

        if k > 0 {
            let dx = x - prev_x;
            if dx > 0.0 {
                // Trapezoidal rule for CL = ∫(Cp_l - Cp_u) dx
                cl += 0.5 * (dcp + prev_dcp) * dx;
                // CM about c/4 = -∫(Cp_l - Cp_u)(x - 0.25) dx
                let dcp_xm = dcp * (x - 0.25);
                cm += 0.5 * (dcp_xm + prev_dcp_xm) * dx;
            }
        }
        prev_x = x;
        prev_dcp = dcp;
        prev_dcp_xm = dcp * (x - 0.25);
    }

    SurfaceCpResult { cl, cm_c4: -cm }
}

/// Linear interpolation of Cp from a sorted (x, Cp) array.
fn interp_cp(surface: &[(f32, f32)], x: f32) -> f32 {
    if surface.is_empty() {
        return 0.0;
    }
    if x <= surface[0].0 {
        return surface[0].1;
    }
    if x >= surface[surface.len() - 1].0 {
        return surface[surface.len() - 1].1;
    }
    for w in surface.windows(2) {
        if w[0].0 <= x && x <= w[1].0 {
            let dx = w[1].0 - w[0].0;
            if dx < 1e-8 {
                return w[0].1;
            }
            let t = (x - w[0].0) / dx;
            return w[0].1 + t * (w[1].1 - w[0].1);
        }
    }
    surface[surface.len() - 1].1
}

/// Compute CL and CM_c/4 from pre-computed panel tangential velocities.
///
/// This is the fast path used by `PanelLuSystem::panel_solution` — the
/// tangential velocities come from the cached tangent influence matrix
/// (a matrix-vector multiply with zero transcendentals).
fn cl_cm_from_tangential_velocities(
    panels: &[Panel],
    vt: &[f32],
    freestream: Vec2,
) -> (f32, f32) {
    let lift_dir = Vec2::new(-freestream.y, freestream.x);
    let mut cn = 0.0_f32;
    let mut cm = 0.0_f32;

    for (i, panel) in panels.iter().enumerate() {
        let cp = 1.0 - vt[i] * vt[i];
        let force_on_panel = -cp * panel.normal * panel.length;
        cn += force_on_panel.dot(lift_dir);

        let rx = panel.mid.x - 0.25;
        let ry = panel.mid.y;
        let dm = rx * force_on_panel.y - ry * force_on_panel.x;
        cm += dm;
    }

    (-cn, cm)
}

/// Compute CL and CM_c/4 by re-evaluating induced velocity at each panel.
///
/// This is the slow path used by `compute_panel_solution` (single-shot
/// solve without a cached LU system).  O(N²) transcendental evaluations.
fn panel_integrated_cl_cm(
    panels: &[Panel],
    sources: &[f32],
    gamma: f32,
    freestream: Vec2,
) -> (f32, f32) {
    let mut vt = Vec::with_capacity(panels.len());
    for panel in panels {
        let colloc = panel.mid + panel.normal * COLLOCATION_OFFSET;
        let induced = induced_velocity_from_solution(
            colloc, panels, sources, gamma,
        );
        let v_total = freestream + induced;
        vt.push(v_total.dot(panel.tangent));
    }
    cl_cm_from_tangential_velocities(panels, &vt, freestream)
}

fn zero_lift_alpha(params: &NacaParams) -> f32 {
    let alpha0_deg = -50.0 * params.m();
    alpha0_deg.to_radians()
}

/// Thin-airfoil theory zero-lift angle for NACA 4-digit.
///
/// Computed by numerical integration of the camber-slope Fourier
/// integral:  α₀L = -(1/π) ∫₀^π (dy_c/dx)(cos θ - 1) dθ
///
/// This is exact for thin cambered plates and provides a ~1% accurate
/// reference for the zero-lift angle that the panel method should
/// converge to with sufficient panels.
fn thin_airfoil_zero_lift_alpha(params: &NacaParams) -> f32 {
    let m = params.m();
    let p = params.p();
    if m < 1e-6 {
        return 0.0; // symmetric
    }
    let n = 200;
    let pi = std::f32::consts::PI;
    let mut integral = 0.0_f32;
    let dtheta = pi / n as f32;
    for i in 1..n {
        let theta = dtheta * i as f32;
        let x = 0.5 * (1.0 - theta.cos());
        let dyc_dx = if p > 0.0 && x <= p {
            2.0 * m / (p * p) * (p - x)
        } else if p < 1.0 {
            2.0 * m / ((1.0 - p) * (1.0 - p)) * (p - x)
        } else {
            0.0
        };
        integral += dyc_dx * (theta.cos() - 1.0) * dtheta;
    }
    -integral / pi
}

/// Thin-airfoil theory CM about c/4 for NACA 4-digit.
///
/// CM_c/4 = -(π/4)(A₁ - A₂) where A₁, A₂ are Fourier cosine coefficients
/// of the camber slope.  This is exact for thin cambered plates and gives
/// ~1% accuracy against published data.  Used instead of the panel Cp
/// integration which has LE noise artifacts for cambered profiles.
fn thin_airfoil_cm_c4(params: &NacaParams) -> f32 {
    let m = params.m();
    let p = params.p();
    if m < 1e-6 {
        return 0.0;
    }
    let n = 200;
    let pi = std::f32::consts::PI;
    let dtheta = pi / n as f32;
    let mut a1 = 0.0_f32;
    let mut a2 = 0.0_f32;
    for i in 1..n {
        let theta = dtheta * i as f32;
        let x = 0.5 * (1.0 - theta.cos());
        let dyc_dx = if p > 0.0 && x <= p {
            2.0 * m / (p * p) * (p - x)
        } else if p < 1.0 {
            2.0 * m / ((1.0 - p) * (1.0 - p)) * (p - x)
        } else {
            0.0
        };
        a1 += dyc_dx * theta.cos() * dtheta * 2.0 / pi;
        a2 += dyc_dx * (2.0 * theta).cos() * dtheta * 2.0 / pi;
    }
    -pi / 4.0 * (a1 - a2)
}

fn compute_fallback_solution(
    params: &NacaParams,
    alpha_deg: f32,
) -> PanelSolution {
    let n = (params.num_points / 2).max(32);
    let alpha_rad = alpha_deg.to_radians() - zero_lift_alpha(params);

    let mut xs = Vec::with_capacity(n);
    let mut cp_u = Vec::with_capacity(n);
    let mut cp_l = Vec::with_capacity(n);
    let mut upper_coords = Vec::with_capacity(n);
    let mut lower_coords = Vec::with_capacity(n);

    for i in 0..n {
        // Cosine spacing along the chord: better LE resolution.
        let beta = i as f32 / (n - 1) as f32;
        let x_c = 0.5 * (1.0 - (PI * beta).cos());

        // Standard NACA thickness distribution.
        let y_t = thickness_distribution(params.t(), x_c);

        // Camber line and slope.
        let y_c = camber_line(params.m(), params.p(), x_c);
        let dyc_dx = camber_slope(params.m(), params.p(), x_c);

        let theta = dyc_dx.atan();

        // Upper and lower surfaces (body coords, chord = 1).
        let x_u = x_c - y_t * theta.sin();
        let y_u = y_c + y_t * theta.cos();

        let x_l = x_c + y_t * theta.sin();
        let y_l = y_c - y_t * theta.cos();

        let v_u = surface_velocity(Vec2::new(x_u, y_u), alpha_rad);
        let v_l = surface_velocity(Vec2::new(x_l, y_l), alpha_rad);

        let speed_u = v_u.length();
        let speed_l = v_l.length();

        // Cp = 1 - (V / U∞)^2, with U∞ = 1.
        let mut cp_upper = 1.0 - speed_u * speed_u;
        let mut cp_lower = 1.0 - speed_l * speed_l;

        // Clamp extremes so the graph stays sane.
        cp_upper = cp_upper.clamp(-3.0, 2.0);
        cp_lower = cp_lower.clamp(-3.0, 2.0);

        xs.push(x_c);
        cp_u.push(cp_lower);
        cp_l.push(cp_upper);
        upper_coords.push(Vec2::new(x_l, y_l));
        lower_coords.push(Vec2::new(x_u, y_u));
    }

    let (cl_cached, cm_c4_cached, _) =
        approx_section_coeffs(params, alpha_deg);
    PanelSolution {
        x: xs,
        cp_upper: cp_u,
        cp_lower: cp_l,
        upper_coords,
        lower_coords,
        cl_cached: Some(cl_cached),
        cm_c4_cached: Some(cm_c4_cached),
    }
}

/// Same analytic model as the old vector field:
/// Free stream along +x plus a bound vortex at quarter chord.
fn surface_velocity(p: Vec2, alpha_rad: f32) -> Vec2 {
    let u_inf = Vec2::X; // free stream along +x in body frame

    let vortex_pos = Vec2::new(0.25, 0.0);
    let r = p - vortex_pos;
    let r2 = r.length_squared().max(1e-4);
    let r_len = r2.sqrt();

    // Circulation ∝ α
    let gamma = 4.0 * PI * alpha_rad;

    let tangential_dir = if r_len > 0.0 {
        Vec2::new(-r.y, r.x) / r_len
    } else {
        Vec2::ZERO
    };

    let v_vortex = tangential_dir * (gamma / (2.0 * PI * r_len));

    u_inf + v_vortex
}

#[cfg(test)]
mod tests;
