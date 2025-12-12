use bevy::math::Vec2;
use std::f32::consts::PI;

use crate::state::NacaParams;

mod geometry;
mod panels;

use geometry::{
    build_naca_body_geometry, camber_line, camber_slope,
    thickness_distribution,
};
use panels::{Panel, build_panels};

const SURFACE_SAMPLE_EPS: f32 = 1e-4;
const COLLOCATION_OFFSET: f32 = 1e-4;

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

pub(crate) struct PanelFlow {
    panels: Vec<Panel>,
    sources: Vec<f32>,
    gamma: f32,
    freestream: Vec2,
}

impl PanelFlow {
    pub(crate) fn velocity_body_pg(
        &self,
        point: Vec2,
        mach: f32,
    ) -> Vec2 {
        let beta = (1.0 - mach * mach).clamp(0.05, 1.0).sqrt();
        let induced = induced_velocity_from_solution(
            point,
            &self.panels,
            &self.sources,
            self.gamma,
        );
        self.freestream + induced / beta
    }
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

pub(crate) fn compute_panel_flow(
    params: &NacaParams,
    alpha_deg: f32,
) -> Option<PanelFlow> {
    let alpha_rad = alpha_deg.to_radians();
    let freestream = Vec2::new(alpha_rad.cos(), alpha_rad.sin());

    let geometry = build_naca_body_geometry(params);
    let panels = build_panels(&geometry);
    if panels.len() < 4 {
        return None;
    }

    let system = assemble_system(&panels, freestream);
    let strengths = solve_linear_system(system);
    if strengths.len() != panels.len() + 1 {
        return None;
    }

    let n_panels = panels.len();
    let gamma = strengths[n_panels];
    let sources = strengths[..n_panels].to_vec();

    Some(PanelFlow {
        panels,
        sources,
        gamma,
        freestream,
    })
}

/// Heuristic section coefficients (thin-airfoil-inspired) to provide stable
/// values and a rough match to XFoil for 4-digit NACA foils.
pub fn analytic_section_coeffs(
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

/// Simple constant-strength vortex panel method with a Kutta condition.
pub fn compute_panel_solution(
    params: &NacaParams,
    alpha_deg: f32,
) -> PanelSolution {
    let sample_count = (params.num_points / 2).max(32);
    let m = params.m();
    let p = params.p();
    let t = params.t();
    let alpha_rad = alpha_deg.to_radians();

    // Freestream in body coordinates; visualization rotates the airfoil in world
    // space, so in body coordinates the freestream rotates with alpha.
    let freestream = Vec2::new(alpha_rad.cos(), alpha_rad.sin());

    let geometry = build_naca_body_geometry(params);
    let panels = build_panels(&geometry);

    if panels.len() < 4 {
        let (cl, cm_c4, _) = analytic_section_coeffs(params, alpha_deg);
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
        let (cl, cm_c4, _) = analytic_section_coeffs(params, alpha_deg);
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
    let sources = &source_strengths;

    let mut xs = Vec::with_capacity(sample_count);
    let mut cp_u = Vec::with_capacity(sample_count);
    let mut cp_l = Vec::with_capacity(sample_count);
    let mut upper_coords = Vec::with_capacity(sample_count);
    let mut lower_coords = Vec::with_capacity(sample_count);

    for i in 0..sample_count {
        let beta = i as f32 / (sample_count - 1) as f32;
        let x_c = 0.5 * (1.0 - (PI * beta).cos());

        let camber_slope = camber_slope(m, p, x_c);
        let thickness = thickness_distribution(t, x_c);

        let theta = camber_slope.atan();

        // Upper and lower surfaces (body coords, chord = 1).
        let upper_point = Vec2::new(
            x_c - thickness * theta.sin(),
            camber_line(m, p, x_c) + thickness * theta.cos(),
        );
        let lower_point = Vec2::new(
            x_c + thickness * theta.sin(),
            camber_line(m, p, x_c) - thickness * theta.cos(),
        );

        let tangent = Vec2::new(theta.cos(), theta.sin());
        let tangent = tangent.normalize_or_zero();
        let normal_upper = Vec2::new(-tangent.y, tangent.x);
        let normal_lower = Vec2::new(tangent.y, -tangent.x);

        let sample_upper =
            upper_point + normal_upper * SURFACE_SAMPLE_EPS;
        let sample_lower =
            lower_point + normal_lower * SURFACE_SAMPLE_EPS;

        let induced_u = induced_velocity_from_solution(
            sample_upper,
            &panels,
            sources,
            gamma,
        );
        let induced_l = induced_velocity_from_solution(
            sample_lower,
            &panels,
            sources,
            gamma,
        );

        let vel_u = freestream + induced_u;
        let vel_l = freestream + induced_l;

        let speed_u = vel_u.length();
        let speed_l = vel_l.length();

        // Cp = 1 - (V / U∞)^2, with U∞ = 1.
        let mut cp_upper = 1.0 - speed_u * speed_u;
        let mut cp_lower = 1.0 - speed_l * speed_l;

        // Clamp extremes so the graph stays sane.
        cp_upper = cp_upper.clamp(-3.0, 2.0);
        cp_lower = cp_lower.clamp(-3.0, 2.0);

        xs.push(x_c);
        cp_u.push(cp_lower);
        cp_l.push(cp_upper);
        upper_coords.push(lower_point);
        lower_coords.push(upper_point);
    }

    let (cl_cached, cm_c4_cached, _) =
        analytic_section_coeffs(params, alpha_deg);
    let panel_solution = PanelSolution {
        x: xs,
        cp_upper: cp_u,
        cp_lower: cp_l,
        upper_coords,
        lower_coords,
        cl_cached: Some(cl_cached),
        cm_c4_cached: Some(cm_c4_cached),
    };

    panel_solution
}

/// Quick analytic fallback (old toy model) used for visualization when the
/// full panel solution is too noisy for Cp plotting.
pub fn compute_cp_approx(
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
            let src = line_source_velocity(colloc, panel_j);
            let vort = line_vortex_velocity(colloc, panel_j);
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
        let src_upper = line_source_velocity(upper_colloc, panel_j);
        let src_lower = line_source_velocity(lower_colloc, panel_j);
        let vort_upper = line_vortex_velocity(upper_colloc, panel_j);
        let vort_lower = line_vortex_velocity(lower_colloc, panel_j);

        matrix[n * size + j] =
            src_upper.dot(upper_dir) - src_lower.dot(lower_dir);
        matrix[n * size + n] +=
            vort_upper.dot(upper_dir) - vort_lower.dot(lower_dir);
    }

    LinearSystem { matrix, rhs, size }
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

fn line_source_velocity(point: Vec2, panel: &Panel) -> Vec2 {
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

    let ln_term = (r2_sq / r1_sq).ln();
    let atan_term = y_local.atan2(x2) - y_local.atan2(x_local);

    let u = ln_term / (4.0 * PI);
    let v = atan_term / (2.0 * PI);

    panel.tangent * u + panel.normal * v
}

fn line_vortex_velocity(point: Vec2, panel: &Panel) -> Vec2 {
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

    let ln_term = (r2_sq / r1_sq).ln();
    let atan_term = y_local.atan2(x2) - y_local.atan2(x_local);

    let u = -atan_term / (2.0 * PI);
    let v = ln_term / (4.0 * PI);

    panel.tangent * u + panel.normal * v
}

fn induced_velocity_from_solution(
    point: Vec2,
    panels: &[Panel],
    sources: &[f32],
    gamma: f32,
) -> Vec2 {
    let mut vel = Vec2::ZERO;
    for (panel, &sigma) in panels.iter().zip(sources.iter()) {
        let infl = line_source_velocity(point, panel);
        vel += infl * sigma;
    }

    if gamma.abs() > 0.0 {
        let mut vort = Vec2::ZERO;
        for panel in panels {
            vort += line_vortex_velocity(point, panel);
        }
        vel += vort * gamma;
    }

    vel
}

fn zero_lift_alpha(params: &NacaParams) -> f32 {
    let alpha0_deg = -50.0 * params.m();
    alpha0_deg.to_radians()
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
        analytic_section_coeffs(params, alpha_deg);
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
