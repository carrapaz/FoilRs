use bevy::{color::palettes::css, prelude::*};

use crate::{
    plotter::{PolarPlotLabels, refresh_polar_labels},
    state::{FlowSettings, NacaParams},
    ui::UiCoeffMode,
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
    pub used_fallback: bool,
}

pub(super) fn compute_polar_primitives(
    params: &NacaParams,
    flow: &FlowSettings,
    alpha_min_deg: f32,
    alpha_max_deg: f32,
    alpha_step_deg: f32,
    threads: Option<usize>,
    panel_system: Option<&crate::solvers::panel::PanelLuSystem>,
    coeff_mode: UiCoeffMode,
) -> PolarGraphPrimitives {
    let mode = match coeff_mode {
        UiCoeffMode::Panel => crate::solvers::polar::PolarMode::Panel,
        UiCoeffMode::Approx => crate::solvers::polar::PolarMode::Approx,
    };
    let res = crate::solvers::polar::compute_polar_sweep_parallel_with_system_mode(
        params,
        flow,
        alpha_min_deg,
        alpha_max_deg,
        alpha_step_deg,
        panel_system,
        threads,
        mode,
    );
    let rows = res.rows;
    let used_fallback = res.used_fallback;

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

    PolarGraphPrimitives {
        cl_pts,
        cd_pts,
        used_fallback,
    }
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
    let cl_ticks = [-1.0, -0.5, 0.0, 0.5, 1.0];
    let cd_ticks = [0.0, 0.005, 0.010, 0.015, 0.020];
    draw_axes(
        gizmos,
        alpha_min_deg,
        alpha_max_deg,
        &alpha_ticks,
        &cl_ticks,
        &cd_ticks,
    );

    if refresh_labels {
        refresh_polar_labels(
            commands,
            asset_server,
            labels,
            alpha_min_deg,
            alpha_max_deg,
            CL_COLOR,
            CD_COLOR,
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

    draw_point_markers(gizmos, &prims.cl_pts, CL_COLOR);
    draw_point_markers(gizmos, &prims.cd_pts, CD_COLOR);
}

fn draw_axes(
    gizmos: &mut Gizmos,
    alpha_min_deg: f32,
    alpha_max_deg: f32,
    alpha_ticks: &[f32],
    cl_ticks: &[f32],
    cd_ticks: &[f32],
) {
    let axis_color = Color::from(css::GRAY).with_alpha(0.65);
    let grid_color = Color::from(css::GRAY).with_alpha(0.18);
    let box_color = Color::from(css::GRAY).with_alpha(0.25);

    // Shared alpha axis (bottom of each plot)
    let x_left = -0.5 * CHORD_PX;
    let x_right = 0.5 * CHORD_PX;

    // CL axis line at y=CL_BASE_Y
    gizmos.line_2d(
        Vec2::new(x_left, CL_BASE_Y),
        Vec2::new(x_right, CL_BASE_Y),
        axis_color,
    );

    // CD axis line at y=CD_BASE_Y
    gizmos.line_2d(
        Vec2::new(x_left, CD_BASE_Y),
        Vec2::new(x_right, CD_BASE_Y),
        axis_color,
    );

    // Plot boxes help convey that these are two stacked plots.
    let cl_tick_min = cl_ticks.first().copied().unwrap_or(-1.0);
    let cl_tick_max = cl_ticks.last().copied().unwrap_or(1.0);
    let cd_tick_min = cd_ticks.first().copied().unwrap_or(0.0);
    let cd_tick_max = cd_ticks.last().copied().unwrap_or(0.02);

    let cl_y_min = CL_BASE_Y + cl_tick_min * CL_SCALE_Y;
    let cl_y_max = CL_BASE_Y + cl_tick_max * CL_SCALE_Y;
    let cd_y_min = CD_BASE_Y + cd_tick_min * CD_SCALE_Y;
    let cd_y_max = CD_BASE_Y + cd_tick_max * CD_SCALE_Y;

    draw_plot_box(
        gizmos, x_left, x_right, cl_y_min, cl_y_max, box_color,
    );
    draw_plot_box(
        gizmos, x_left, x_right, cd_y_min, cd_y_max, box_color,
    );

    // Horizontal gridlines (CL/CD).
    for &cl in cl_ticks {
        let y = CL_BASE_Y + cl * CL_SCALE_Y;
        gizmos.line_2d(
            Vec2::new(x_left, y),
            Vec2::new(x_right, y),
            grid_color,
        );
    }
    for &cd in cd_ticks {
        let y = CD_BASE_Y + cd * CD_SCALE_Y;
        gizmos.line_2d(
            Vec2::new(x_left, y),
            Vec2::new(x_right, y),
            grid_color,
        );
    }

    // Vertical gridlines for alpha on both plots.
    for &a in alpha_ticks {
        let x = alpha_to_world_x(a, alpha_min_deg, alpha_max_deg);
        gizmos.line_2d(
            Vec2::new(x, cl_y_min),
            Vec2::new(x, cl_y_max),
            grid_color,
        );
        gizmos.line_2d(
            Vec2::new(x, cd_y_min),
            Vec2::new(x, cd_y_max),
            grid_color,
        );
    }

    // Stronger reference line at alpha=0 if it's in-range.
    if alpha_min_deg <= 0.0 && 0.0 <= alpha_max_deg {
        let x0 = alpha_to_world_x(0.0, alpha_min_deg, alpha_max_deg);
        let ref_color = Color::from(css::WHITE).with_alpha(0.22);
        gizmos.line_2d(
            Vec2::new(x0, cl_y_min),
            Vec2::new(x0, cl_y_max),
            ref_color,
        );
        gizmos.line_2d(
            Vec2::new(x0, cd_y_min),
            Vec2::new(x0, cd_y_max),
            ref_color,
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

fn draw_plot_box(
    gizmos: &mut Gizmos,
    x_left: f32,
    x_right: f32,
    y_min: f32,
    y_max: f32,
    color: Color,
) {
    let a = Vec2::new(x_left, y_min);
    let b = Vec2::new(x_right, y_min);
    let c = Vec2::new(x_right, y_max);
    let d = Vec2::new(x_left, y_max);
    gizmos.line_2d(a, b, color);
    gizmos.line_2d(b, c, color);
    gizmos.line_2d(c, d, color);
    gizmos.line_2d(d, a, color);
}

fn draw_point_markers(gizmos: &mut Gizmos, pts: &[Vec2], color: Color) {
    if pts.is_empty() {
        return;
    }
    let every = (pts.len() / 50).max(1);
    let size = 3.5;
    let c = color.with_alpha(0.75);
    for p in pts.iter().step_by(every) {
        gizmos.line_2d(
            Vec2::new(p.x - size, p.y),
            Vec2::new(p.x + size, p.y),
            c,
        );
        gizmos.line_2d(
            Vec2::new(p.x, p.y - size),
            Vec2::new(p.x, p.y + size),
            c,
        );
    }
}
