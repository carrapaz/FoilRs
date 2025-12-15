use bevy::{color::palettes::css, prelude::*};

use crate::{
    plotter::{CpPlotLabels, refresh_cp_labels},
    solvers,
    state::{FlowSettings, NacaParams},
    ui::UiCoeffMode,
    views::CHORD_PX,
};

pub(super) const CP_UPPER_COLOR: Color = Color::srgb(0.2, 0.9, 0.9);
pub(super) const CP_LOWER_COLOR: Color = Color::srgb(0.9, 0.4, 0.2);

#[derive(Clone)]
pub(super) struct CpGraphPrimitives {
    pub upper_pts: Vec<Vec2>,
    pub lower_pts: Vec<Vec2>,
}

fn cp_corrected(cp: f32, flow: &FlowSettings) -> f32 {
    // Prandtl–Glauert compressibility correction for subsonic flow.
    let beta = (1.0 - flow.mach * flow.mach).clamp(0.05, 1.0).sqrt();
    let mut corrected = cp / beta;

    // Simple visualization-only viscous damping: lower Reynolds reduces Cp
    // magnitude (brings pressures closer to freestream).
    if flow.viscous {
        let re_factor =
            (flow.reynolds / 1_000_000.0).sqrt().clamp(0.35, 1.0);
        corrected *= re_factor;
    }

    corrected
}

pub(super) fn compute_cp_graph_primitives(
    params: &NacaParams,
    flow: &FlowSettings,
    base_y: f32,
    scale_y: f32,
    panel_system: Option<&crate::solvers::panel::PanelLuSystem>,
    coeff_mode: UiCoeffMode,
) -> (Option<CpGraphPrimitives>, bool) {
    let mut used_fallback = false;
    let sol = match coeff_mode {
        UiCoeffMode::Approx => solvers::panel::compute_approx_solution(
            params,
            flow.alpha_deg,
        ),
        UiCoeffMode::Panel => {
            let sol = panel_system
                .map(|sys| sys.panel_solution(params, flow.alpha_deg))
                .unwrap_or_else(|| {
                    solvers::compute_panel_solution(
                        params,
                        flow.alpha_deg,
                    )
                });
            if sol.x.is_empty() {
                used_fallback = true;
                solvers::panel::compute_approx_solution(
                    params,
                    flow.alpha_deg,
                )
            } else {
                sol
            }
        }
    };

    let mut upper_pts = Vec::with_capacity(sol.x.len());
    let mut lower_pts = Vec::with_capacity(sol.x.len());

    for i in 0..sol.x.len() {
        let x = sol.x[i];
        let cp_u = cp_corrected(sol.cp_upper[i], flow);
        let cp_l = cp_corrected(sol.cp_lower[i], flow);

        let world_x = (x - 0.5) * CHORD_PX;
        upper_pts.push(Vec2::new(world_x, base_y - cp_u * scale_y));
        lower_pts.push(Vec2::new(world_x, base_y - cp_l * scale_y));
    }

    (
        Some(CpGraphPrimitives {
            upper_pts,
            lower_pts,
        }),
        used_fallback,
    )
}

/// Draw Cp(x) graph below the airfoil, in “screen-ish” coordinates.
pub(super) fn draw_cp_graph_primitives(
    prims: &CpGraphPrimitives,
    gizmos: &mut Gizmos,
    commands: &mut Commands,
    asset_server: &AssetServer,
    cp_labels: &mut CpPlotLabels,
    base_y: f32,
    scale_y: f32,
    refresh_labels: bool,
) {
    // Axis
    let axis_left = Vec2::new(-0.5 * CHORD_PX, base_y);
    let axis_right = Vec2::new(0.5 * CHORD_PX, base_y);
    gizmos.line_2d(
        axis_left,
        axis_right,
        Color::from(css::GRAY).with_alpha(0.7),
    );

    // Ticks
    let x_ticks = [0.0, 0.25, 0.5, 0.75, 1.0];
    for &x in &x_ticks {
        let wx = (x - 0.5) * CHORD_PX;
        let tick_len = 8.0;
        gizmos.line_2d(
            Vec2::new(wx, base_y - tick_len),
            Vec2::new(wx, base_y + tick_len),
            Color::from(css::GRAY).with_alpha(0.6),
        );
    }
    let cp_ticks = [-3.0, -2.0, -1.0, 0.0, 1.0, 2.0];
    for &cp in &cp_ticks {
        let wy = base_y - cp * scale_y;
        let tick_len = 10.0;
        gizmos.line_2d(
            Vec2::new(axis_left.x - tick_len, wy),
            Vec2::new(axis_left.x + tick_len, wy),
            Color::from(css::GRAY).with_alpha(0.6),
        );
    }

    // Labels
    if refresh_labels {
        refresh_cp_labels(
            commands,
            asset_server,
            cp_labels,
            &x_ticks,
            &cp_ticks,
            base_y,
        );
    }

    // Upper / lower Cp curves
    gizmos
        .linestrip_2d(prims.upper_pts.iter().copied(), CP_UPPER_COLOR);
    gizmos
        .linestrip_2d(prims.lower_pts.iter().copied(), CP_LOWER_COLOR);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flow(mach: f32, reynolds: f32, viscous: bool) -> FlowSettings {
        FlowSettings(foil_rs::state::FlowSettings {
            alpha_deg: 4.0,
            reynolds,
            mach,
            viscous,
            free_transition: true,
        })
    }

    #[test]
    fn cp_correction_identity_at_mach0_inviscid() {
        let f = flow(0.0, 1_000_000.0, false);
        let cp = -1.2;
        let corrected = cp_corrected(cp, &f);
        assert!((corrected - cp).abs() < 1e-6);
    }

    #[test]
    fn cp_correction_increases_magnitude_with_mach() {
        let inviscid_low = flow(0.0, 1_000_000.0, false);
        let inviscid_high = flow(0.6, 1_000_000.0, false);

        let cp = -1.0;
        let c0 = cp_corrected(cp, &inviscid_low);
        let c1 = cp_corrected(cp, &inviscid_high);
        assert!(c1 < c0, "expected Cp more negative at higher Mach");

        let beta = (1.0 - 0.6_f32 * 0.6).sqrt();
        let expected = cp / beta;
        assert!(
            (c1 - expected).abs() < 1e-4,
            "expected PG correction cp/beta={}, got {}",
            expected,
            c1
        );
    }

    #[test]
    fn cp_correction_damps_magnitude_at_low_re_when_viscous() {
        let f_low = flow(0.0, 100_000.0, true);
        let f_high = flow(0.0, 10_000_000.0, true);

        let cp = -1.0;
        let c_low = cp_corrected(cp, &f_low);
        let c_high = cp_corrected(cp, &f_high);

        assert!(
            c_high.abs() > c_low.abs(),
            "expected higher-Re Cp magnitude; low={}, high={}",
            c_low,
            c_high
        );
    }
}
