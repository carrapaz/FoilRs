use bevy_math::Vec2;
use std::f32::consts::PI;

use crate::state::NacaParams;

/// Build NACA 4-digit geometry in body coordinates as a closed loop.
///
/// Ordering: starts at the trailing edge, goes along the lower surface to the
/// leading edge, then returns along the upper surface back to the trailing
/// edge.
pub fn build_naca_body_geometry(params: &NacaParams) -> Vec<Vec2> {
    let m = params.m();
    let p = params.p();
    let t = params.t();
    let n = params.num_points.max(32);

    let mut upper: Vec<Vec2> = Vec::with_capacity(n);
    let mut lower: Vec<Vec2> = Vec::with_capacity(n);
    let mut full: Vec<Vec2> = Vec::with_capacity(2 * n);

    for i in (0..n).rev() {
        let beta = i as f32 / (n - 1) as f32;
        let x_c = 0.5 * (1.0 - (PI * beta).cos());

        let camber = camber_line(m, p, x_c);
        let slope = camber_slope(m, p, x_c);
        let theta = slope.atan();
        let thickness = thickness_distribution(t, x_c);

        let x_u = x_c - thickness * theta.sin();
        let y_u = camber + thickness * theta.cos();

        upper.push(Vec2::new(x_u, y_u));
    }

    for i in 0..n {
        let beta = i as f32 / (n - 1) as f32;
        let x_c = 0.5 * (1.0 - (PI * beta).cos());

        let camber = camber_line(m, p, x_c);
        let slope = camber_slope(m, p, x_c);
        let theta = slope.atan();
        let thickness = thickness_distribution(t, x_c);

        let x_l = x_c + thickness * theta.sin();
        let y_l = camber - thickness * theta.cos();

        lower.push(Vec2::new(x_l, y_l));
    }

    if upper.is_empty() || lower.is_empty() {
        return full;
    }

    // Counter-clockwise ordering: start at trailing edge, walk along lower
    // surface to the leading edge, then back along the upper surface.
    for pt in lower.iter().rev() {
        full.push(*pt);
    }
    for pt in upper.iter().rev().skip(1) {
        full.push(*pt);
    }
    // Close the loop by adding a small rounded “cap” at the trailing edge.
    // If we just subdivide the straight TE segment, all sub-panels overlap
    // visually and have identical tangents; an arc gives distinct tangents.
    const TE_PANELS: usize = 8;
    if let (Some(&te_lower), Some(&te_upper)) =
        (full.first(), full.last())
    {
        let gap = te_upper - te_lower;
        let gap_len = gap.length();
        if gap_len > 1e-8 && TE_PANELS >= 2 {
            // Replace the last point (TE upper) with TE upper + arc points,
            // then close back to TE lower.
            let _ = full.pop();
            full.push(te_upper);

            let center = 0.5 * (te_upper + te_lower);
            let r = 0.5 * gap_len;
            let axis = (te_upper - center).normalize_or_zero();
            let mut perp =
                Vec2::new(-axis.y, axis.x).normalize_or_zero();
            // Bulge inward (toward -x) so we don't extend beyond the TE.
            if perp.x > 0.0 {
                perp = -perp;
            }

            for i in 1..TE_PANELS {
                let theta = (i as f32 / TE_PANELS as f32) * PI;
                let p = center
                    + axis * (r * theta.cos())
                    + perp * (r * theta.sin());
                full.push(p);
            }
        }

        full.push(te_lower);
    }

    full
}

/// Build NACA 4-digit geometry as a closed loop with a *sharp* trailing edge.
///
/// This is preferable for vortex panel methods with a Kutta condition.
/// Ordering matches `build_naca_body_geometry`.
pub fn build_naca_body_geometry_sharp_te(
    params: &NacaParams,
) -> Vec<Vec2> {
    let m = params.m();
    let p = params.p();
    let t = params.t();
    let n = params.num_points.max(32);

    let mut upper: Vec<Vec2> = Vec::with_capacity(n);
    let mut lower: Vec<Vec2> = Vec::with_capacity(n);
    let mut full: Vec<Vec2> = Vec::with_capacity(2 * n + 1);

    for i in (0..n).rev() {
        let beta = i as f32 / (n - 1) as f32;
        let x_c = 0.5 * (1.0 - (PI * beta).cos());

        let camber = camber_line(m, p, x_c);
        let slope = camber_slope(m, p, x_c);
        let theta = slope.atan();
        let thickness = thickness_distribution(t, x_c);

        let x_u = x_c - thickness * theta.sin();
        let y_u = camber + thickness * theta.cos();

        upper.push(Vec2::new(x_u, y_u));
    }

    for i in 0..n {
        let beta = i as f32 / (n - 1) as f32;
        let x_c = 0.5 * (1.0 - (PI * beta).cos());

        let camber = camber_line(m, p, x_c);
        let slope = camber_slope(m, p, x_c);
        let theta = slope.atan();
        let thickness = thickness_distribution(t, x_c);

        let x_l = x_c + thickness * theta.sin();
        let y_l = camber - thickness * theta.cos();

        lower.push(Vec2::new(x_l, y_l));
    }

    if upper.is_empty() || lower.is_empty() {
        return full;
    }

    for pt in lower.iter().rev() {
        full.push(*pt);
    }
    for pt in upper.iter().rev().skip(1) {
        full.push(*pt);
    }

    if let Some(&first) = full.first() {
        full.push(first);
    }

    full
}

pub fn camber_line(m: f32, p: f32, x: f32) -> f32 {
    if m == 0.0 || p == 0.0 {
        0.0
    } else if x <= p {
        m / (p * p) * (2.0 * p * x - x * x)
    } else {
        m / ((1.0 - p) * (1.0 - p))
            * ((1.0 - 2.0 * p) + 2.0 * p * x - x * x)
    }
}

pub fn camber_slope(m: f32, p: f32, x: f32) -> f32 {
    if m == 0.0 || p == 0.0 {
        0.0
    } else if x <= p {
        2.0 * m / (p * p) * (p - x)
    } else {
        2.0 * m / ((1.0 - p) * (1.0 - p)) * (p - x)
    }
}

pub fn thickness_distribution(t: f32, x: f32) -> f32 {
    t / 0.2
        * (0.2969 * x.sqrt() - 0.1260 * x - 0.3516 * x * x
            + 0.2843 * x * x * x
            - 0.1015 * x * x * x * x)
}
