use super::{
    COLLLOCATION_OFFSET, Panel, Vec2, doublet_potential_influence,
    lu_factorize_f64, lu_solve_f64, source_potential,
};

fn assemble_system(
    panels: &[Panel],
    freestream: Vec2,
) -> (Vec<f64>, Vec<f64>, usize) {
    let size = panels.len();
    let mut matrix = vec![0.0_f64; size * size];
    let mut rhs = vec![0.0_f64; size];
    let le_idx = panels
        .iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| a.mid.x.total_cmp(&b.mid.x))
        .map(|(i, _)| i)
        .unwrap_or(0);

    for (i, pi) in panels.iter().enumerate() {
        let colloc = pi.mid - pi.normal * COLLLOCATION_OFFSET;
        let mut rhs_i = -(freestream.dot(colloc) as f64);

        for pj in panels {
            let sigma = freestream.dot(pj.normal);
            rhs_i -= (source_potential(colloc, pj) * sigma) as f64;
        }

        for (j, pj) in panels.iter().enumerate() {
            let (phi_left, phi_right) =
                doublet_potential_influence(colloc, pj);
            if j > 0 {
                matrix[i * size + (j - 1)] += phi_left as f64;
            }
            matrix[i * size + j] += phi_right as f64;
        }

        let wake_sign = if i <= le_idx { -0.5 } else { 0.5 };
        // Approximate the wake jump from the TE-adjacent panel averages:
        //   mu_wake = 0.5 * (mu_{N-1} + mu_N) - 0.5 * mu_1
        matrix[i * size + 0] += (-0.5 * wake_sign) as f64;
        if size >= 2 {
            matrix[i * size + (size - 2)] += (0.5 * wake_sign) as f64;
        }
        matrix[i * size + (size - 1)] += (0.5 * wake_sign) as f64;
        rhs[i] = rhs_i;
    }

    (matrix, rhs, size)
}

fn solve_doublets(
    panels: &[Panel],
    freestream: Vec2,
) -> Option<Vec<f32>> {
    let (matrix, rhs, size) = assemble_system(panels, freestream);
    let (lu, pivots) = lu_factorize_f64(&matrix, size)?;
    let solved = lu_solve_f64(&lu, &pivots, &rhs, size)?;

    let mut mu = Vec::with_capacity(size + 1);
    mu.push(0.0);
    mu.extend(solved.into_iter().map(|v| v as f32));
    Some(mu)
}

fn tangential_velocities(
    panels: &[Panel],
    mu_nodes: &[f32],
) -> Vec<f32> {
    if mu_nodes.len() != panels.len() + 1 || panels.is_empty() {
        return Vec::new();
    }

    let mut s_nodes = Vec::with_capacity(mu_nodes.len());
    s_nodes.push(0.0_f32);
    for panel in panels {
        let next_s =
            s_nodes.last().copied().unwrap_or(0.0) + panel.length;
        s_nodes.push(next_s);
    }

    let mut node_vt = vec![0.0_f32; mu_nodes.len()];
    if mu_nodes.len() >= 2 {
        let ds0 = (s_nodes[1] - s_nodes[0]).max(1e-8);
        node_vt[0] = (mu_nodes[1] - mu_nodes[0]) / ds0;

        for i in 1..mu_nodes.len() - 1 {
            let ds = (s_nodes[i + 1] - s_nodes[i - 1]).max(1e-8);
            node_vt[i] = (mu_nodes[i + 1] - mu_nodes[i - 1]) / ds;
        }

        let last = mu_nodes.len() - 1;
        let ds_last = (s_nodes[last] - s_nodes[last - 1]).max(1e-8);
        node_vt[last] = (mu_nodes[last] - mu_nodes[last - 1]) / ds_last;
    }

    let mut vt = Vec::with_capacity(panels.len());
    for i in 0..panels.len() {
        vt.push(0.5 * (node_vt[i] + node_vt[i + 1]));
    }
    vt
}

pub(super) fn surface_velocities(
    panels: &[Panel],
    freestream: Vec2,
) -> Option<(Vec<f32>, f32)> {
    let mu_nodes = solve_doublets(panels, freestream)?;
    let max_unknown =
        mu_nodes.iter().fold(0.0_f32, |acc, &mu| acc.max(mu.abs()));
    let vt = tangential_velocities(panels, &mu_nodes);
    Some((vt, max_unknown))
}
