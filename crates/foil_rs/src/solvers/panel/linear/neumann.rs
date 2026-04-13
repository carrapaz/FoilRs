use super::{
    COLLLOCATION_OFFSET, Panel, Vec2, lu_factorize_f64, lu_solve_f64,
    panel_influence,
};

fn leading_edge_panel_index(panels: &[Panel]) -> usize {
    panels
        .iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| a.mid.x.total_cmp(&b.mid.x))
        .map(|(i, _)| i)
        .unwrap_or(0)
}

fn solve_linear_vortex_nodes(
    panels: &[Panel],
    freestream: Vec2,
) -> Option<Vec<f32>> {
    let n = panels.len();
    if n < 2 {
        return None;
    }

    let size = n + 1;
    let gauge_row = leading_edge_panel_index(panels);
    let mut matrix = vec![0.0_f64; size * size];
    let mut rhs = vec![0.0_f64; size];

    for (i, pi) in panels.iter().enumerate() {
        if i == gauge_row {
            continue;
        }

        let colloc = pi.mid + pi.normal * COLLLOCATION_OFFSET;
        let mut rhs_i = -(freestream.dot(pi.normal) as f64);

        for (j, pj) in panels.iter().enumerate() {
            let sigma = -freestream.dot(pj.normal);
            let (src, vl, vr) = panel_influence(colloc, pj);
            rhs_i -= (src.dot(pi.normal) * sigma) as f64;
            matrix[i * size + j] += vl.dot(pi.normal) as f64;
            matrix[i * size + (j + 1)] += vr.dot(pi.normal) as f64;
        }

        rhs[i] = rhs_i;
    }

    for col in 0..size {
        matrix[gauge_row * size + col] = 1.0;
    }
    rhs[gauge_row] = 0.0;

    matrix[n * size + 0] = 1.0;
    matrix[n * size + n] = -1.0;
    rhs[n] = 0.0;

    let (lu, pivots) = lu_factorize_f64(&matrix, size)?;
    let solved = lu_solve_f64(&lu, &pivots, &rhs, size)?;
    Some(solved.into_iter().map(|v| v as f32).collect())
}

fn source_tangential_velocity(
    panel_idx: usize,
    point: Vec2,
    tangent: Vec2,
    panels: &[Panel],
    freestream: Vec2,
) -> f32 {
    let mut vt = 0.0_f32;
    for (j, pj) in panels.iter().enumerate() {
        if j == panel_idx {
            continue;
        }
        let sigma = -freestream.dot(pj.normal);
        let (src, _, _) = panel_influence(point, pj);
        vt += src.dot(tangent) * sigma;
    }
    vt
}

fn vortex_tangential_velocities(
    panels: &[Panel],
    node_gammas: &[f32],
    freestream: Vec2,
) -> Vec<f32> {
    if node_gammas.len() != panels.len() + 1 || panels.is_empty() {
        return Vec::new();
    }

    let mut vt = Vec::with_capacity(panels.len());
    for (i, panel) in panels.iter().enumerate() {
        let point = panel.mid + panel.normal * COLLLOCATION_OFFSET;
        let mut vortex_t = 0.0_f32;

        for (j, pj) in panels.iter().enumerate() {
            let (_, vl, vr) = panel_influence(point, pj);
            vortex_t += vl.dot(panel.tangent) * node_gammas[j];
            vortex_t += vr.dot(panel.tangent) * node_gammas[j + 1];
        }

        let gamma_mid = 0.5 * (node_gammas[i] + node_gammas[i + 1]);
        let source_t = source_tangential_velocity(
            i,
            panel.mid,
            panel.tangent,
            panels,
            freestream,
        );
        vt.push(
            freestream.dot(panel.tangent) + source_t - vortex_t
                + 0.5 * gamma_mid,
        );
    }
    vt
}

pub(super) fn surface_velocities(
    panels: &[Panel],
    freestream: Vec2,
) -> Option<(Vec<f32>, f32)> {
    let node_gammas = solve_linear_vortex_nodes(panels, freestream)?;
    let max_unknown = node_gammas
        .iter()
        .fold(0.0_f32, |acc, &gamma| acc.max(gamma.abs()));
    let vt =
        vortex_tangential_velocities(panels, &node_gammas, freestream);
    Some((vt, max_unknown))
}
