use bevy::{color::palettes::css, prelude::*};

use crate::{
    plotter::{PolarPlotLabels, refresh_polar_labels},
    state::{FlowSettings, NacaParams},
    views::CHORD_PX,
};

const ALPHA_MIN_DEG: f32 = -10.0;
const ALPHA_MAX_DEG: f32 = 15.0;
const ALPHA_STEP_DEG: f32 = 0.5;

const CL_BASE_Y: f32 = -120.0;
const CL_SCALE_Y: f32 = 120.0;

const CD_BASE_Y: f32 = -320.0;
const CD_SCALE_Y: f32 = 6000.0; // 0.01 Cd -> 60px

const CL_COLOR: Color = Color::srgb(0.25, 0.85, 0.95);
const CD_COLOR: Color = Color::srgb(0.95, 0.55, 0.25);

#[derive(Clone)]
pub(super) struct PolarGraphPrimitives {
    pub cl_pts: Vec<Vec2>,
    pub cd_pts: Vec<Vec2>,
}

pub(super) fn compute_polar_primitives(
    params: &NacaParams,
    flow: &FlowSettings,
    panel_system: Option<&crate::solvers::panel::PanelLuSystem>,
) -> PolarGraphPrimitives {
    let rows =
        crate::solvers::polar::compute_polar_sweep_parallel_with_system(
            params,
            flow,
            ALPHA_MIN_DEG,
            ALPHA_MAX_DEG,
            ALPHA_STEP_DEG,
            panel_system,
            None,
        );

    let mut cl_pts = Vec::with_capacity(rows.len());
    let mut cd_pts = Vec::with_capacity(rows.len());

    for row in rows {
        let x = alpha_to_world_x(row.alpha_deg);
        cl_pts.push(Vec2::new(x, CL_BASE_Y + row.cl * CL_SCALE_Y));

        let cd = row.cd_profile.unwrap_or(0.0).max(0.0);
        cd_pts.push(Vec2::new(x, CD_BASE_Y + cd * CD_SCALE_Y));
    }

    PolarGraphPrimitives { cl_pts, cd_pts }
}

pub(super) fn draw_polar_primitives(
    prims: &PolarGraphPrimitives,
    gizmos: &mut Gizmos,
    commands: &mut Commands,
    asset_server: &AssetServer,
    labels: &mut PolarPlotLabels,
    refresh_labels: bool,
) {
    draw_axes(gizmos);

    if refresh_labels {
        let alpha_ticks = [-10.0, -5.0, 0.0, 5.0, 10.0, 15.0];
        let cl_ticks = [-1.0, -0.5, 0.0, 0.5, 1.0];
        let cd_ticks = [0.0, 0.005, 0.010, 0.015, 0.020];
        refresh_polar_labels(
            commands,
            asset_server,
            labels,
            &alpha_ticks,
            &cl_ticks,
            &cd_ticks,
            CL_BASE_Y,
            CL_SCALE_Y,
            CD_BASE_Y,
            CD_SCALE_Y,
        );
    }

    if prims.cl_pts.len() >= 2 {
        gizmos.linestrip_2d(prims.cl_pts.iter().copied(), CL_COLOR);
    }
    if prims.cd_pts.len() >= 2 {
        gizmos.linestrip_2d(prims.cd_pts.iter().copied(), CD_COLOR);
    }
}

fn draw_axes(gizmos: &mut Gizmos) {
    let axis_color = Color::from(css::GRAY).with_alpha(0.65);

    // Shared alpha axis (bottom of each plot)
    let left = Vec2::new(-0.5 * CHORD_PX, 0.0);
    let right = Vec2::new(0.5 * CHORD_PX, 0.0);

    // CL axis line at y=CL_BASE_Y
    gizmos.line_2d(
        Vec2::new(left.x, CL_BASE_Y),
        Vec2::new(right.x, CL_BASE_Y),
        axis_color,
    );

    // CD axis line at y=CD_BASE_Y
    gizmos.line_2d(
        Vec2::new(left.x, CD_BASE_Y),
        Vec2::new(right.x, CD_BASE_Y),
        axis_color,
    );

    // Vertical ticks for alpha on both axes
    for &a in &[-10.0, -5.0, 0.0, 5.0, 10.0, 15.0] {
        let x = alpha_to_world_x(a);
        let tick = 8.0;
        gizmos.line_2d(
            Vec2::new(x, CL_BASE_Y - tick),
            Vec2::new(x, CL_BASE_Y + tick),
            axis_color.with_alpha(0.55),
        );
        gizmos.line_2d(
            Vec2::new(x, CD_BASE_Y - tick),
            Vec2::new(x, CD_BASE_Y + tick),
            axis_color.with_alpha(0.55),
        );
    }
}

fn alpha_to_world_x(alpha_deg: f32) -> f32 {
    let t = ((alpha_deg - ALPHA_MIN_DEG)
        / (ALPHA_MAX_DEG - ALPHA_MIN_DEG))
        .clamp(0.0, 1.0);
    (-0.5 * CHORD_PX) + t * CHORD_PX
}
