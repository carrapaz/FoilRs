//! Neumann source + linear-vortex panel method.
//!
//! Sources: σ_j = V∞·n_j (prescribed, Morino). NOT unknowns.
//! Unknowns: γ_0..γ_N (N+1 node vortex strengths, linear per panel).
//! Equations: N normal BCs + 1 Kutta = N+1.
//! Matrix: (N+1)×(N+1).
//!
//! The normal BC at each panel midpoint is:
//!   Σ_j [vort_influence · n_i] × γ_j = -V∞·n_i - Σ_j [source_vel · n_i] × σ_j
//!
//! The RHS absorbs both the freestream and the known source contribution.
//! The source velocity represents the non-lifting (thickness) part.

use std::f32::consts::PI;

use crate::math::{Vec2, fast_atan2, fast_ln};

use super::panels::Panel;

const COLLOCATION_OFFSET: f32 = 1e-4;

/// Returns (source_vel, vort_left_vel, vort_right_vel).
#[inline]
#[allow(clippy::excessive_precision)]
fn panel_influence(point: Vec2, panel: &Panel) -> (Vec2, Vec2, Vec2) {
    let dx = point.x - panel.start.x;
    let dy = point.y - panel.start.y;
    let xl = dx * panel.tangent.x + dy * panel.tangent.y;
    let yl = dx * panel.normal.x + dy * panel.normal.y;
    let yl = if yl.abs() < 1e-6 {
        if yl >= 0.0 { 1e-6 } else { -1e-6 }
    } else {
        yl
    };
    let x2 = xl - panel.length;
    let len = panel.length.max(1e-8);
    let r1sq = (xl * xl + yl * yl).max(1e-12);
    let r2sq = (x2 * x2 + yl * yl).max(1e-12);
    let ln_t = fast_ln(r2sq / r1sq);
    let at_t = fast_atan2(yl, x2) - fast_atan2(yl, xl);
    let i2p = 1.0 / (2.0 * PI);
    let i4p = 1.0 / (4.0 * PI);
    let i2pl = i2p / len;

    let src =
        panel.tangent * (ln_t * i4p) + panel.normal * (at_t * i2p);
    let uc = -at_t * i2p;
    let vc = ln_t * i4p;
    let ur = -i2pl * (xl * at_t + 0.5 * yl * ln_t);
    let vr = i2pl * (-0.5 * xl * ln_t + yl * at_t - len);
    let vl = panel.tangent * (uc - ur) + panel.normal * (vc - vr);
    let vrt = panel.tangent * ur + panel.normal * vr;

    (src, vl, vrt)
}

/// Assemble the (N+1)×(N+1) matrix.
///
/// LHS: linear vortex influence (normal component) at each collocation pt.
/// RHS: -(freestream + source velocity) normal component.
pub(super) fn assemble(
    panels: &[Panel],
    te_upper: usize,
    te_lower: usize,
) -> (Vec<f32>, Vec<f32>, usize) {
    let n = panels.len();
    let size = n + 1;
    let mut matrix = vec![0.0_f32; size * size];
    let mut tan_infl = vec![0.0_f32; n * size]; // for V_t recovery

    for (i, pi) in panels.iter().enumerate() {
        let colloc = pi.mid + pi.normal * COLLOCATION_OFFSET;

        for (j, pj) in panels.iter().enumerate() {
            let (src, vl, vr) = panel_influence(colloc, pj);
            let j1 = j + 1;

            // Normal BC: vortex only (source goes to RHS).
            matrix[i * size + j] += vl.dot(pi.normal);
            matrix[i * size + j1] += vr.dot(pi.normal);

            // Tangent influence for V_t recovery.
            tan_infl[i * size + j] += src.dot(pi.tangent); // source V_t (will multiply by σ at runtime)
            // ... actually we need to store vortex tangent influence too.
            // Let me separate source_tan and vort_tan.
        }
    }

    // Hmm, the tan_infl needs both source and vortex components.
    // For V_t recovery: V_t = V∞·t + Σ_j source_vel·t × σ_j + Σ_j vort_vel·t × γ_j
    // The source σ_j = V∞·n_j is known at runtime (depends on freestream).
    // So we can't pre-cache the source V_t contribution.
    // → tan_infl stores ONLY the vortex tangent influence.
    // The source V_t is computed at runtime in tangential_velocities().

    // Redo: tan_infl = vortex tangent influence only.
    tan_infl.fill(0.0);
    for (i, pi) in panels.iter().enumerate() {
        let colloc = pi.mid + pi.normal * COLLOCATION_OFFSET;
        for (j, pj) in panels.iter().enumerate() {
            let (_src, vl, vr) = panel_influence(colloc, pj);
            tan_infl[i * size + j] += vl.dot(pi.tangent);
            tan_infl[i * size + j + 1] += vr.dot(pi.tangent);
        }
    }

    // Kutta: γ_0 + γ_N = 0 (zero vortex at both TE nodes).
    matrix[n * size + 0] = 1.0;
    matrix[n * size + n] = 1.0;

    (matrix, tan_infl, size)
}

/// Build the RHS.
pub(super) fn assemble_rhs(
    panels: &[Panel],
    freestream: Vec2,
) -> Vec<f32> {
    let n = panels.len();
    let size = n + 1;
    let mut rhs = vec![0.0_f32; size];

    for (i, pi) in panels.iter().enumerate() {
        let colloc = pi.mid + pi.normal * COLLOCATION_OFFSET;
        let mut src_n = 0.0_f32;
        for pj in panels.iter() {
            let sigma = freestream.dot(pj.normal);
            let (src, _, _) = panel_influence(colloc, pj);
            src_n += src.dot(pi.normal) * sigma;
        }
        rhs[i] = -freestream.dot(pi.normal) - src_n;
    }

    // Kutta RHS: γ_0 + γ_N = 0 → rhs = 0.
    rhs[n] = 0.0;

    rhs
}

/// V_t at each panel from node gammas + prescribed source.
pub(super) fn tangential_velocities(
    panels: &[Panel],
    tan_infl: &[f32],
    node_gammas: &[f32],
    freestream: Vec2,
) -> Vec<f32> {
    let n = panels.len();
    let size = n + 1;
    let inv_2pi = 1.0 / (2.0 * PI);
    let inv_4pi = 1.0 / (4.0 * PI);
    let mut vt = Vec::with_capacity(n);

    for i in 0..n {
        let mut v = freestream.dot(panels[i].tangent);

        // Vortex tangent from cached tan_infl.
        for j in 0..node_gammas.len().min(size) {
            v += tan_infl[i * size + j] * node_gammas[j];
        }

        // Source tangent (computed at runtime since σ depends on freestream).
        let colloc =
            panels[i].mid + panels[i].normal * COLLOCATION_OFFSET;
        for pj in panels.iter() {
            let sigma = freestream.dot(pj.normal);
            if sigma.abs() < 1e-10 {
                continue;
            }
            let dx = colloc.x - pj.start.x;
            let dy = colloc.y - pj.start.y;
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

/// Induced velocity at point (for viz).
pub(super) fn induced_velocity(
    point: Vec2,
    panels: &[Panel],
    node_gammas: &[f32],
    freestream: Vec2,
) -> Vec2 {
    let mut vel = Vec2::ZERO;
    for (j, pj) in panels.iter().enumerate() {
        let (src, vl, vr) = panel_influence(point, pj);
        let sigma = freestream.dot(pj.normal);
        vel += src * sigma;
        let j1 = if j + 1 < node_gammas.len() { j + 1 } else { 0 };
        vel += vl * node_gammas[j] + vr * node_gammas[j1];
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
