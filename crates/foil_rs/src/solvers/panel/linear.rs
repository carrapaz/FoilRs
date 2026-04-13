//! Constant-doublet + constant-source Dirichlet panel method.
//!
//! Source σ_j = V∞·n_j (Morino, prescribed). Unknowns: μ_0..μ_{N-1}
//! (panel doublets) + μ_wake (wake doublet). Total: N+1.
//! Equations: N (Dirichlet) + 1 (Kutta: μ_wake = μ_upper - μ_lower).

use std::f32::consts::PI;

use crate::math::{Vec2, fast_atan2, fast_ln};

use super::panels::Panel;

#[inline]
fn source_potential(point: Vec2, panel: &Panel) -> f32 {
    let dx = point.x - panel.start.x;
    let dy = point.y - panel.start.y;
    let x = dx * panel.tangent.x + dy * panel.tangent.y;
    let y = dx * panel.normal.x + dy * panel.normal.y;
    let y = if y.abs() < 1e-8 {
        if y >= 0.0 { 1e-8 } else { -1e-8 }
    } else {
        y
    };
    let x2 = x - panel.length;
    let r1_sq = (x * x + y * y).max(1e-14);
    let r2_sq = (x2 * x2 + y * y).max(1e-14);
    let atan_term = fast_atan2(y, x2) - fast_atan2(y, x);
    (1.0 / (4.0 * PI))
        * (x * fast_ln(r1_sq)
            - x2 * fast_ln(r2_sq)
            - 2.0 * panel.length
            + 2.0 * y * atan_term)
}

#[inline]
fn doublet_potential(point: Vec2, panel: &Panel) -> f32 {
    let dx = point.x - panel.start.x;
    let dy = point.y - panel.start.y;
    let x = dx * panel.tangent.x + dy * panel.tangent.y;
    let y = dx * panel.normal.x + dy * panel.normal.y;
    let y = if y.abs() < 1e-8 {
        if y >= 0.0 { 1e-8 } else { -1e-8 }
    } else {
        y
    };
    let x2 = x - panel.length;
    let atan_term = fast_atan2(y, x2) - fast_atan2(y, x);
    -atan_term / (2.0 * PI)
}

/// Wake potential: semi-infinite doublet panel from TE along +x.
/// For a point P at distance d from TE, the potential is -(1/2π)·θ
/// where θ = atan2(y, x) in TE-centered coordinates.
#[inline]
fn wake_potential(point: Vec2, te_pos: Vec2) -> f32 {
    let d = point - te_pos;
    -fast_atan2(d.y, d.x) / (2.0 * PI)
}

/// Assemble (N+1)×(N+1) matrix.
///
/// Unknowns x[0..N] = μ_0..μ_{N-1} (panel doublets), x[N] = μ_wake.
/// Rows 0..N-1: Dirichlet BC (φ_internal = 0 at each panel midpoint).
/// Row N: Kutta (μ_wake = μ_{te_upper} - μ_{te_lower}).
pub(super) fn assemble(
    panels: &[Panel],
    te_upper: usize,
    te_lower: usize,
) -> (Vec<f32>, usize) {
    let n = panels.len();
    let size = n + 1;
    let mut matrix = vec![0.0_f32; size * size];

    let te_pos = 0.5 * (panels[te_upper].mid + panels[te_lower].mid);

    // Dirichlet rows: 0.5·μ_i + Σ_{j≠i} doublet_pot × μ_j + wake_pot × μ_wake = RHS
    for i in 0..n {
        let pt = panels[i].mid;
        for j in 0..n {
            matrix[i * size + j] = if i == j {
                0.5
            } else {
                doublet_potential(pt, &panels[j])
            };
        }
        // Wake column (index N).
        matrix[i * size + n] = wake_potential(pt, te_pos);
    }

    // Kutta row: μ_wake - μ_{te_upper} + μ_{te_lower} = 0
    matrix[n * size + n] = 1.0;
    matrix[n * size + te_upper] = 1.0;
    matrix[n * size + te_lower] = -1.0;

    (matrix, size)
}

/// RHS: -(source potential + freestream potential) at each panel midpoint.
pub(super) fn assemble_rhs(
    panels: &[Panel],
    freestream: Vec2,
) -> Vec<f32> {
    let n = panels.len();
    let size = n + 1;
    let mut rhs = vec![0.0_f32; size];
    for i in 0..n {
        let pt = panels[i].mid;
        let mut phi_src = 0.0_f32;
        for pj in panels.iter() {
            phi_src +=
                source_potential(pt, pj) * freestream.dot(pj.normal);
        }
        rhs[i] = -(phi_src + freestream.dot(pt));
    }
    // Kutta rhs = 0
    rhs
}

/// V_t = V∞·t + dμ/ds + source_velocity_tangential.
pub(super) fn tangential_velocities(
    panels: &[Panel],
    solution: &[f32], // length N+1: [μ_0..μ_{N-1}, μ_wake]
    freestream: Vec2,
) -> Vec<f32> {
    let n = panels.len();
    let mus = &solution[..n];
    let inv_2pi = 1.0 / (2.0 * PI);
    let inv_4pi = 1.0 / (4.0 * PI);
    let mut vt = Vec::with_capacity(n);

    for i in 0..n {
        let mut v = freestream.dot(panels[i].tangent);

        // dμ/ds
        let ip = if i > 0 { i - 1 } else { n - 1 };
        let in_ = if i + 1 < n { i + 1 } else { 0 };
        let ds_p = panels[i].mid.distance(panels[ip].mid).max(1e-6);
        let ds_n = panels[in_].mid.distance(panels[i].mid).max(1e-6);
        v += 0.5
            * ((mus[i] - mus[ip]) / ds_p + (mus[in_] - mus[i]) / ds_n);

        // Source velocity tangential component.
        let pt = panels[i].mid;
        for pj in panels.iter() {
            let sigma = freestream.dot(pj.normal);
            if sigma.abs() < 1e-10 {
                continue;
            }
            let dx = pt.x - pj.start.x;
            let dy = pt.y - pj.start.y;
            let xl = dx * pj.tangent.x + dy * pj.tangent.y;
            let yl = dx * pj.normal.x + dy * pj.normal.y;
            let yl = if yl.abs() < 1e-6 {
                if yl >= 0.0 { 1e-6 } else { -1e-6 }
            } else {
                yl
            };
            let x2 = xl - pj.length;
            let r1sq = (xl * xl + yl * yl).max(1e-12);
            let r2sq = (x2 * x2 + yl * yl).max(1e-12);
            let sv = pj.tangent * (fast_ln(r2sq / r1sq) * inv_4pi)
                + pj.normal
                    * ((fast_atan2(yl, x2) - fast_atan2(yl, xl))
                        * inv_2pi);
            v += sv.dot(panels[i].tangent) * sigma;
        }

        vt.push(v);
    }
    vt
}

/// Induced velocity at point (for field viz).
pub(super) fn induced_velocity(
    point: Vec2,
    panels: &[Panel],
    solution: &[f32],
    freestream: Vec2,
) -> Vec2 {
    let n = panels.len();
    let mus = &solution[..n];
    let inv_2pi = 1.0 / (2.0 * PI);
    let inv_4pi = 1.0 / (4.0 * PI);
    let mut vel = Vec2::ZERO;
    for (j, pj) in panels.iter().enumerate() {
        let dx = point.x - pj.start.x;
        let dy = point.y - pj.start.y;
        let xl = dx * pj.tangent.x + dy * pj.tangent.y;
        let yl = dx * pj.normal.x + dy * pj.normal.y;
        let yl = if yl.abs() < 1e-6 {
            if yl >= 0.0 { 1e-6 } else { -1e-6 }
        } else {
            yl
        };
        let x2 = xl - pj.length;
        let r1sq = (xl * xl + yl * yl).max(1e-12);
        let r2sq = (x2 * x2 + yl * yl).max(1e-12);
        let ln_t = fast_ln(r2sq / r1sq);
        let at_t = fast_atan2(yl, x2) - fast_atan2(yl, xl);
        let sigma = freestream.dot(pj.normal);
        vel += (pj.tangent * (ln_t * inv_4pi)
            + pj.normal * (at_t * inv_2pi))
            * sigma;
        vel += (pj.tangent * (-at_t * inv_2pi)
            + pj.normal * (ln_t * inv_4pi))
            * mus[j];
    }
    vel
}

pub(super) fn te_panel_indices(panels: &[Panel]) -> (usize, usize) {
    let n = panels.len();
    let le = panels
        .iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| a.mid.x.total_cmp(&b.mid.x))
        .map(|(i, _)| i)
        .unwrap_or(0);
    let te_lower = (0..=le)
        .max_by(|&a, &b| panels[a].mid.x.total_cmp(&panels[b].mid.x))
        .unwrap_or(0);
    let te_upper = (le..n)
        .max_by(|&a, &b| panels[a].mid.x.total_cmp(&panels[b].mid.x))
        .unwrap_or(n - 1);
    (te_upper, te_lower)
}
