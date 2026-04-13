//! Source + linear-vortex panel method (Morino coupling).
//!
//! Unknowns: γ_0..γ_N (N+1 node vortex strengths).
//! Source on panel j is derived: σ_j = -(γ_{j+1} - γ_j) / L_j
//! (negative gradient of vortex strength, normalized by panel length).
//! This represents the thickness: where the vortex strength changes
//! rapidly, the body is thicker.
//!
//! Equations: N normal BCs + 1 Kutta (γ_0 + γ_N = 0) = N+1.
//! Matrix: (N+1)×(N+1) — same size as the old method.

use std::f32::consts::PI;

use crate::math::{Vec2, fast_atan2, fast_ln};

use super::panels::Panel;

const COLLOCATION_OFFSET: f32 = 1e-4;

/// Returns (source_vel, vort_left_vel, vort_right_vel).
#[inline]
#[allow(clippy::excessive_precision)]
pub(super) fn panel_influence_linear(
    point: Vec2,
    panel: &Panel,
) -> (Vec2, Vec2, Vec2) {
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
    let len = panel.length.max(1e-8);

    let r1_sq = (x_local * x_local + y_local * y_local).max(1e-12);
    let r2_sq = (x2 * x2 + y_local * y_local).max(1e-12);

    let ln_term = fast_ln(r2_sq / r1_sq);
    let atan_term =
        fast_atan2(y_local, x2) - fast_atan2(y_local, x_local);

    let inv_2pi = 1.0 / (2.0 * PI);
    let inv_4pi = 1.0 / (4.0 * PI);
    let inv_2pi_l = inv_2pi / len;

    let src = panel.tangent * (ln_term * inv_4pi)
        + panel.normal * (atan_term * inv_2pi);

    let u_const = -atan_term * inv_2pi;
    let v_const = ln_term * inv_4pi;
    let u_ramp =
        -inv_2pi_l * (x_local * atan_term + 0.5 * y_local * ln_term);
    let v_ramp = inv_2pi_l
        * (-0.5 * x_local * ln_term + y_local * atan_term - len);

    let vort_left = panel.tangent * (u_const - u_ramp)
        + panel.normal * (v_const - v_ramp);
    let vort_right = panel.tangent * u_ramp + panel.normal * v_ramp;

    (src, vort_left, vort_right)
}

/// Assemble the Morino-coupled matrix.
///
/// Source on panel j: σ_j = -(γ_{j+1} - γ_j) / L_j
///
/// For each collocation point i, the influence of node j comes from:
/// - Source of panels where j is involved: σ = -(γ_{j+1}-γ_j)/L
///   This contributes to column j via +src/L and column j+1 via -src/L
/// - Linear vortex: V_left for start-node, V_right for end-node
pub(super) fn assemble(
    panels: &[Panel],
) -> (Vec<f32>, Vec<f32>, usize) {
    let n = panels.len();
    let size = n + 1;
    let mut matrix = vec![0.0_f32; size * size];
    let mut tan_infl = vec![0.0_f32; n * size];

    for (i, panel_i) in panels.iter().enumerate() {
        let colloc = panel_i.mid + panel_i.normal * COLLOCATION_OFFSET;

        for (j, panel_j) in panels.iter().enumerate() {
            let (src, vort_left, vort_right) =
                panel_influence_linear(colloc, panel_j);

            let j1 = j + 1;
            let inv_l = 1.0 / panel_j.length.max(1e-8);

            // Source contribution: σ_j = -(γ_{j+1} - γ_j) / L_j
            // The velocity from source is: src * σ_j = src * (γ_j - γ_{j+1}) / L
            let src_n = src.dot(panel_i.normal) * inv_l;
            matrix[i * size + j] += src_n; // +γ_j / L
            matrix[i * size + j1] -= src_n; // -γ_{j+1} / L

            // Linear vortex contribution.
            matrix[i * size + j] += vort_left.dot(panel_i.normal);
            matrix[i * size + j1] += vort_right.dot(panel_i.normal);

            // Tangent influence for V_t.
            let src_t = src.dot(panel_i.tangent) * inv_l;
            tan_infl[i * size + j] += src_t;
            tan_infl[i * size + j1] -= src_t;
            tan_infl[i * size + j] += vort_left.dot(panel_i.tangent);
            tan_infl[i * size + j1] += vort_right.dot(panel_i.tangent);
        }
    }

    // Kutta: γ_0 + γ_N = 0.
    matrix[n * size + 0] = 1.0;
    matrix[n * size + n] = 1.0;

    (matrix, tan_infl, size)
}

pub(super) fn assemble_rhs(
    panels: &[Panel],
    freestream: Vec2,
) -> Vec<f32> {
    let n = panels.len();
    let size = n + 1;
    let mut rhs = vec![0.0_f32; size];
    for (i, panel_i) in panels.iter().enumerate() {
        rhs[i] = -freestream.dot(panel_i.normal);
    }
    rhs
}

/// V_t at each panel from node gammas via cached tan_infl matrix.
pub(super) fn tangential_velocities(
    panels: &[Panel],
    tan_infl: &[f32],
    node_gammas: &[f32],
    freestream: Vec2,
) -> Vec<f32> {
    let n = panels.len();
    let size = n + 1;
    let mut vt = Vec::with_capacity(n);
    for i in 0..n {
        let mut v = 0.0_f32;
        for j in 0..node_gammas.len().min(size) {
            v += tan_infl[i * size + j] * node_gammas[j];
        }
        vt.push(freestream.dot(panels[i].tangent) + v);
    }
    vt
}

/// Induced velocity at an arbitrary point.
pub(super) fn induced_velocity(
    point: Vec2,
    panels: &[Panel],
    node_gammas: &[f32],
) -> Vec2 {
    let mut vel = Vec2::ZERO;
    for (j, panel) in panels.iter().enumerate() {
        let (src, vl, vr) = panel_influence_linear(point, panel);
        let j1 = if j + 1 < node_gammas.len() { j + 1 } else { 0 };
        let inv_l = 1.0 / panel.length.max(1e-8);
        let sigma = (node_gammas[j] - node_gammas[j1]) * inv_l;
        vel += src * sigma;
        vel += vl * node_gammas[j] + vr * node_gammas[j1];
    }
    vel
}
