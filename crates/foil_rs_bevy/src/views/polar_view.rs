use bevy::{color::palettes::css, prelude::*};

use crate::{
    plotter::{PolarPlotLabels, refresh_polar_labels},
    state::{FlowSettings, NacaParams},
    views::CHORD_PX,
};

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
    alpha_min_deg: f32,
    alpha_max_deg: f32,
    alpha_step_deg: f32,
    threads: Option<usize>,
    panel_system: Option<&crate::solvers::panel::PanelLuSystem>,
) -> PolarGraphPrimitives {
    let rows =
        crate::solvers::polar::compute_polar_sweep_parallel_with_system(
            params,
            flow,
            alpha_min_deg,
            alpha_max_deg,
            alpha_step_deg,
            panel_system,
            threads,
        );

    let mut cl_pts = Vec::with_capacity(rows.len());
    let mut cd_pts = Vec::with_capacity(rows.len());

    for row in rows {
        let x = alpha_to_world_x(
            row.alpha_deg,
            alpha_min_deg,
            alpha_max_deg,
        );
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
    alpha_min_deg: f32,
    alpha_max_deg: f32,
) {
    let alpha_ticks = alpha_tick_values(alpha_min_deg, alpha_max_deg);
    draw_axes(gizmos, &alpha_ticks, alpha_min_deg, alpha_max_deg);

    if refresh_labels {
        let cl_ticks = [-1.0, -0.5, 0.0, 0.5, 1.0];
        let cd_ticks = [0.0, 0.005, 0.010, 0.015, 0.020];
        refresh_polar_labels(
            commands,
            asset_server,
            labels,
            alpha_min_deg,
            alpha_max_deg,
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

fn draw_axes(
    gizmos: &mut Gizmos,
    alpha_ticks: &[f32],
    alpha_min_deg: f32,
    alpha_max_deg: f32,
) {
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
    for &a in alpha_ticks {
        let x = alpha_to_world_x(a, alpha_min_deg, alpha_max_deg);
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

fn alpha_to_world_x(
    alpha_deg: f32,
    alpha_min_deg: f32,
    alpha_max_deg: f32,
) -> f32 {
    let denom = (alpha_max_deg - alpha_min_deg).abs().max(1e-6);
    let t = ((alpha_deg - alpha_min_deg) / denom).clamp(0.0, 1.0);
    (-0.5 * CHORD_PX) + t * CHORD_PX
}

fn alpha_tick_values(
    alpha_min_deg: f32,
    alpha_max_deg: f32,
) -> Vec<f32> {
    let a0 = alpha_min_deg.min(alpha_max_deg);
    let a1 = alpha_min_deg.max(alpha_max_deg);
    let range = (a1 - a0).max(1e-6);

    let step = if range <= 6.0 {
        1.0
    } else if range <= 12.0 {
        2.0
    } else if range <= 30.0 {
        5.0
    } else {
        10.0
    };

    let start = (a0 / step).ceil() * step;
    let mut ticks = Vec::new();

    ticks.push(a0);
    let mut a = start;
    while a <= a1 + 1e-6 {
        if (a - a0).abs() > 1e-3 && (a - a1).abs() > 1e-3 {
            ticks.push(a);
        }
        a += step;
    }
    ticks.push(a1);

    ticks.sort_by(|a, b| a.total_cmp(b));
    ticks.dedup_by(|a, b| (*a - *b).abs() < 1e-3);
    ticks
}
