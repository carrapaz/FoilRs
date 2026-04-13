//! Experimental source + linear-doublet panel method.
//!
//! This module keeps the higher-order work isolated from the shipped
//! constant-strength solver while we validate the formulation.

#![allow(dead_code)]

use std::f32::consts::PI;

use crate::math::{Vec2, fast_atan2, fast_ln};
use crate::state::NacaParams;

use super::panels::{Panel, build_panels};
use super::{
    PanelSolution, build_naca_body_geometry_sharp_te, camber_line,
    camber_slope, cl_cm_from_tangential_velocities,
    effective_num_points, interp_cp, thickness_distribution,
};

mod dirichlet;
mod neumann;

const COLLLOCATION_OFFSET: f32 = 1e-4;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExperimentalMethod {
    NeumannLinearVortex,
    DirichletDoublet,
}

impl ExperimentalMethod {
    pub fn label(self) -> &'static str {
        match self {
            Self::NeumannLinearVortex => "neumann-linear-vortex",
            Self::DirichletDoublet => "dirichlet-doublet",
        }
    }
}

#[derive(Clone, Copy)]
struct KernelTerms {
    x_local: f32,
    y_local: f32,
    x2: f32,
    length: f32,
    r1sq: f32,
    r2sq: f32,
    ln_term: f32,
    atan_term: f32,
}

#[inline]
fn kernel_terms(point: Vec2, panel: &Panel) -> KernelTerms {
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
    let length = panel.length.max(1e-8);
    let r1sq = (x_local * x_local + y_local * y_local).max(1e-12);
    let r2sq = (x2 * x2 + y_local * y_local).max(1e-12);
    let ln_term = fast_ln(r2sq / r1sq);
    let atan_term =
        fast_atan2(y_local, x2) - fast_atan2(y_local, x_local);

    KernelTerms {
        x_local,
        y_local,
        x2,
        length,
        r1sq,
        r2sq,
        ln_term,
        atan_term,
    }
}

/// Returns `(source_vel, vort_left_vel, vort_right_vel)`.
#[inline]
#[allow(clippy::excessive_precision)]
fn panel_influence(point: Vec2, panel: &Panel) -> (Vec2, Vec2, Vec2) {
    let k = kernel_terms(point, panel);
    let i2p = 1.0 / (2.0 * PI);
    let i4p = 1.0 / (4.0 * PI);
    let i2pl = i2p / k.length;

    let src = panel.tangent * (k.ln_term * i4p)
        + panel.normal * (k.atan_term * i2p);
    let uc = -k.atan_term * i2p;
    let vc = k.ln_term * i4p;
    let ur =
        -i2pl * (k.x_local * k.atan_term + 0.5 * k.y_local * k.ln_term);
    let vr = i2pl
        * (-0.5 * k.x_local * k.ln_term + k.y_local * k.atan_term
            - k.length);
    let vl = panel.tangent * (uc - ur) + panel.normal * (vc - vr);
    let vrt = panel.tangent * ur + panel.normal * vr;

    (src, vl, vrt)
}

#[inline]
fn source_potential(point: Vec2, panel: &Panel) -> f32 {
    let k = kernel_terms(point, panel);
    (k.x_local * fast_ln(k.r1sq)
        - k.x2 * fast_ln(k.r2sq)
        - 2.0 * k.length
        + 2.0 * k.y_local * k.atan_term)
        / (4.0 * PI)
}

#[inline]
fn doublet_potential_influence(
    point: Vec2,
    panel: &Panel,
) -> (f32, f32) {
    let k = kernel_terms(point, panel);
    let phi_const = -k.atan_term / (2.0 * PI);
    let phi_right = -(k.x_local * k.atan_term
        + 0.5 * k.y_local * k.ln_term)
        / (2.0 * PI * k.length);
    let phi_left = phi_const - phi_right;
    (phi_left, phi_right)
}

fn lu_factorize_f64(
    matrix: &[f64],
    n: usize,
) -> Option<(Vec<f64>, Vec<usize>)> {
    let mut lu = matrix.to_vec();
    let mut pivots = Vec::with_capacity(n);

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
        if pivot_val < 1e-12 {
            return None;
        }
        pivots.push(pivot_row);
        if pivot_row != k {
            for col in 0..n {
                lu.swap(k * n + col, pivot_row * n + col);
            }
        }

        let pivot = lu[k * n + k];
        if pivot.abs() < 1e-15 {
            return None;
        }
        for i in (k + 1)..n {
            lu[i * n + k] /= pivot;
            let lik = lu[i * n + k];
            if lik.abs() < 1e-15 {
                continue;
            }
            for j in (k + 1)..n {
                lu[i * n + j] -= lik * lu[k * n + j];
            }
        }
    }

    Some((lu, pivots))
}

fn lu_solve_f64(
    lu: &[f64],
    pivots: &[usize],
    rhs: &[f64],
    n: usize,
) -> Option<Vec<f64>> {
    if rhs.len() != n || pivots.len() != n || lu.len() != n * n {
        return None;
    }

    let mut x = rhs.to_vec();
    for (k, &pivot_row) in pivots.iter().enumerate() {
        if pivot_row != k {
            x.swap(k, pivot_row);
        }
    }

    for i in 0..n {
        let mut sum = x[i];
        for j in 0..i {
            sum -= lu[i * n + j] * x[j];
        }
        x[i] = sum;
    }

    for i in (0..n).rev() {
        let mut sum = x[i];
        for j in (i + 1)..n {
            sum -= lu[i * n + j] * x[j];
        }
        let diag = lu[i * n + i];
        if diag.abs() < 1e-15 {
            return None;
        }
        x[i] = sum / diag;
    }

    Some(x)
}

fn build_experimental_panels(
    params: &NacaParams,
    alpha_deg: f32,
) -> Option<(NacaParams, Vec<Panel>, Vec2)> {
    let alpha_rad = alpha_deg.to_radians();
    let freestream = Vec2::new(alpha_rad.cos(), alpha_rad.sin());

    let mut local = params.clone();
    local.num_points = effective_num_points(params);
    let geometry = build_naca_body_geometry_sharp_te(&local);
    let mut closed_panels = build_panels(&geometry);
    if closed_panels.len() < 6 {
        return None;
    }

    let open_len = closed_panels.len().saturating_sub(1);
    closed_panels.truncate(open_len);
    Some((local, closed_panels, freestream))
}

fn experimental_surface_velocities(
    panels: &[Panel],
    freestream: Vec2,
    method: ExperimentalMethod,
) -> Option<(Vec<f32>, f32)> {
    match method {
        ExperimentalMethod::NeumannLinearVortex => {
            neumann::surface_velocities(panels, freestream)
        }
        ExperimentalMethod::DirichletDoublet => {
            dirichlet::surface_velocities(panels, freestream)
        }
    }
}

fn build_panel_solution_from_surface_velocities(
    params: &NacaParams,
    panels: &[Panel],
    vt: &[f32],
    freestream: Vec2,
) -> Option<PanelSolution> {
    if panels.len() < 4 || vt.len() != panels.len() {
        return None;
    }

    let (cl_cached, cm_c4_cached) =
        cl_cm_from_tangential_velocities(panels, vt, freestream);
    let le_idx = panels
        .iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| a.mid.x.total_cmp(&b.mid.x))
        .map(|(i, _)| i)
        .unwrap_or(0);

    let mut upper: Vec<(f32, f32)> = (0..=le_idx)
        .map(|i| (panels[i].mid.x, 1.0 - vt[i] * vt[i]))
        .collect();
    upper.sort_by(|a, b| a.0.total_cmp(&b.0));

    let mut lower: Vec<(f32, f32)> = (le_idx + 1..panels.len())
        .map(|i| (panels[i].mid.x, 1.0 - vt[i] * vt[i]))
        .collect();
    lower.sort_by(|a, b| a.0.total_cmp(&b.0));

    if upper.len() < 2 || lower.len() < 2 {
        return None;
    }

    let sample_count = (params.num_points / 2).max(40);
    let m = params.m();
    let p = params.p();
    let t = params.t();

    let mut x = Vec::with_capacity(sample_count);
    let mut cp_upper = Vec::with_capacity(sample_count);
    let mut cp_lower = Vec::with_capacity(sample_count);
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

        x.push(x_c);
        cp_upper.push(interp_cp(&upper, x_c));
        cp_lower.push(interp_cp(&lower, x_c));
        upper_coords.push(lower_point);
        lower_coords.push(upper_point);
    }

    Some(PanelSolution {
        x,
        cp_upper,
        cp_lower,
        upper_coords,
        lower_coords,
        cl_cached: Some(cl_cached),
        cm_c4_cached: Some(cm_c4_cached),
    })
}

pub(super) fn try_compute_solution_with_method(
    params: &NacaParams,
    alpha_deg: f32,
    method: ExperimentalMethod,
) -> Option<PanelSolution> {
    let (local, panels, freestream) =
        build_experimental_panels(params, alpha_deg)?;
    let (vt, _) =
        experimental_surface_velocities(&panels, freestream, method)?;
    build_panel_solution_from_surface_velocities(
        &local, &panels, &vt, freestream,
    )
}

pub(super) fn try_compute_solution(
    params: &NacaParams,
    alpha_deg: f32,
) -> Option<PanelSolution> {
    try_compute_solution_with_method(
        params,
        alpha_deg,
        ExperimentalMethod::NeumannLinearVortex,
    )
}

#[cfg(test)]
pub(super) fn debug_snapshot_for_method(
    params: &NacaParams,
    alpha_deg: f32,
    method: ExperimentalMethod,
) -> Option<(f32, f32, f32)> {
    let (_local, panels, freestream) =
        build_experimental_panels(params, alpha_deg)?;
    let (vt, max_unknown) =
        experimental_surface_velocities(&panels, freestream, method)?;
    let (cl, _) =
        cl_cm_from_tangential_velocities(&panels, &vt, freestream);
    let max_vt = vt.iter().fold(0.0_f32, |acc, &v| acc.max(v.abs()));
    Some((cl, max_unknown, max_vt))
}

#[cfg(test)]
pub(super) fn debug_snapshot(
    params: &NacaParams,
    alpha_deg: f32,
) -> Option<(f32, f32, f32)> {
    debug_snapshot_for_method(
        params,
        alpha_deg,
        ExperimentalMethod::NeumannLinearVortex,
    )
}
