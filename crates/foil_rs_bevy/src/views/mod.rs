use bevy::{
    color::palettes::css,
    math::{Mat2, Vec2},
    prelude::*,
};

use crate::airfoil::build_naca_body_geometry;
use crate::plotter::{CpPlotLabels, PolarPlotLabels};
use crate::state::{FlowSettings, NacaParams};
use crate::ui::{PolarSweepSettings, VisualMode};

mod cp_view;
mod field_view;
mod panel_view;
mod polar_view;

use cp_view::{
    CP_LOWER_COLOR, CP_UPPER_COLOR, CpGraphPrimitives,
    compute_cp_graph_primitives, draw_cp_graph_primitives,
};
use field_view::{
    FieldPrimitives, compute_field_primitives, draw_field_primitives,
};
use panel_view::{
    PanelPrimitives, compute_panel_primitives, draw_panel_primitives,
};
use polar_view::{
    PolarGraphPrimitives, compute_polar_primitives,
    draw_polar_primitives,
};

pub(crate) const CHORD_PX: f32 = 450.0;

#[derive(Clone, Copy, PartialEq, Eq)]
struct NacaKey {
    m: u8,
    p: u8,
    t: u8,
    num_points: usize,
}

impl From<&NacaParams> for NacaKey {
    fn from(params: &NacaParams) -> Self {
        Self {
            m: params.m_digit.round().clamp(0.0, 9.0) as u8,
            p: params.p_digit.round().clamp(0.0, 9.0) as u8,
            t: params.t_digits.round().clamp(0.0, 99.0) as u8,
            num_points: params.num_points,
        }
    }
}

#[derive(Default)]
pub struct VizCache {
    naca_key: Option<NacaKey>,
    body: Vec<Vec2>,

    body_world_alpha_bits: Option<u32>,
    body_world: Vec<Vec2>,

    panel_system: Option<crate::solvers::panel::PanelLuSystem>,

    field_key: Option<(NacaKey, u32, u32, u32, bool)>,
    field_prims: FieldPrimitives,

    panel_key: Option<(NacaKey, u32)>,
    panel_prims: PanelPrimitives,

    cp_key: Option<(NacaKey, u32, u32, u32, u32, u32, bool)>,
    cp_prims: Option<CpGraphPrimitives>,
    cp_labels_key: Option<(u32, u32)>,
    cp_labels_dirty: bool,

    polar_key:
        Option<(NacaKey, u32, u32, bool, bool, u32, u32, u32, u8)>,
    polar_prims: Option<PolarGraphPrimitives>,
    polar_labels_dirty: bool,
}

/// Main drawing system: airfoil + either field or Cp(x) depending on VisualMode.
pub fn draw_airfoil_and_visualization(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut cp_labels: ResMut<CpPlotLabels>,
    mut polar_labels: ResMut<PolarPlotLabels>,
    params: Res<NacaParams>,
    flow: Res<FlowSettings>,
    sweep: Res<PolarSweepSettings>,
    mode: Res<VisualMode>,
    mut gizmos: Gizmos,
    mut cache: Local<VizCache>,
) {
    let alpha_rad = flow.alpha_deg.to_radians();

    let naca_key = NacaKey::from(&*params);
    if cache.naca_key != Some(naca_key) {
        cache.naca_key = Some(naca_key);
        cache.body = build_naca_body_geometry(&params);
        cache.panel_system =
            crate::solvers::panel::PanelLuSystem::new(&params);
        cache.body_world_alpha_bits = None;
        cache.field_key = None;
        cache.panel_key = None;
        cache.cp_key = None;
        cache.cp_prims = None;
        cache.polar_key = None;
        cache.polar_prims = None;
        cache.polar_labels_dirty = true;
    }

    let alpha_bits = flow.alpha_deg.to_bits();
    if cache.body_world_alpha_bits != Some(alpha_bits) {
        let world: Vec<Vec2> = cache
            .body
            .iter()
            .map(|p| body_to_world(*p, alpha_rad, CHORD_PX))
            .collect();
        cache.body_world_alpha_bits = Some(alpha_bits);
        cache.body_world = world;
    }

    // Light grid background for the viewport to give scale.
    draw_grid(&mut gizmos);

    // Airfoil
    let airfoil_color = Color::srgb(1.0, 1.0, 1.0);
    if *mode == VisualMode::Cp {
        let le_idx = cache
            .body
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| a.x.total_cmp(&b.x))
            .map(|(idx, _)| idx)
            .unwrap_or(0)
            .min(cache.body_world.len().saturating_sub(1));

        let te_upper_idx = cache
            .body
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| {
                a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y))
            })
            .map(|(idx, _)| idx)
            .unwrap_or(le_idx)
            .min(cache.body_world.len().saturating_sub(1));

        if le_idx > 0 {
            gizmos.linestrip_2d(
                cache.body_world[..=le_idx].iter().copied(),
                CP_LOWER_COLOR,
            );
        }

        if le_idx < te_upper_idx {
            gizmos.linestrip_2d(
                cache.body_world[le_idx..=te_upper_idx].iter().copied(),
                CP_UPPER_COLOR,
            );
        }

        if te_upper_idx + 1 < cache.body_world.len() {
            gizmos.linestrip_2d(
                cache.body_world[te_upper_idx..].iter().copied(),
                airfoil_color.with_alpha(0.8),
            );
        }
    } else {
        gizmos.linestrip_2d(
            cache.body_world.iter().copied(),
            airfoil_color,
        );
        if let (Some(first), Some(last)) =
            (cache.body_world.first(), cache.body_world.last())
        {
            if (*first - *last).length_squared() > 1e-6 {
                gizmos.line_2d(*last, *first, airfoil_color);
            }
        }
    }

    // Chord line
    let le = body_to_world(Vec2::new(0.0, 0.0), alpha_rad, CHORD_PX);
    let te = body_to_world(Vec2::new(1.0, 0.0), alpha_rad, CHORD_PX);
    gizmos.line_2d(le, te, Color::from(css::GRAY).with_alpha(0.6));

    match *mode {
        VisualMode::Field => {
            // Clear any lingering Cp labels when leaving Cp view.
            crate::plotter::clear_cp_labels(
                &mut commands,
                &mut cp_labels,
            );
            crate::plotter::clear_polar_labels(
                &mut commands,
                &mut polar_labels,
            );
            let key = (
                naca_key,
                alpha_bits,
                flow.mach.to_bits(),
                flow.reynolds.to_bits(),
                flow.viscous,
            );
            if cache.field_key != Some(key) {
                let prims = compute_field_primitives(
                    &flow,
                    &cache.body,
                    cache.panel_system.as_ref(),
                );
                cache.field_key = Some(key);
                cache.field_prims = prims;
            }
            draw_field_primitives(&cache.field_prims, &mut gizmos);
        }
        VisualMode::Cp => {
            crate::plotter::clear_polar_labels(
                &mut commands,
                &mut polar_labels,
            );
            let base_y: f32 = -260.0;
            let scale_y: f32 = 120.0;
            let key = (
                naca_key,
                alpha_bits,
                flow.mach.to_bits(),
                flow.reynolds.to_bits(),
                base_y.to_bits(),
                scale_y.to_bits(),
                flow.viscous,
            );
            if cache.cp_key != Some(key) {
                cache.cp_key = Some(key);
                cache.cp_prims = compute_cp_graph_primitives(
                    &params,
                    &flow,
                    base_y,
                    scale_y,
                    cache.panel_system.as_ref(),
                );
            }

            let labels_key = (base_y.to_bits(), scale_y.to_bits());
            if cache.cp_labels_key != Some(labels_key) {
                cache.cp_labels_key = Some(labels_key);
                cache.cp_labels_dirty = true;
            }

            let Some(prims) = cache.cp_prims.as_ref() else {
                return;
            };

            let refresh_labels =
                cache.cp_labels_dirty || cp_labels.entities.is_empty();
            draw_cp_graph_primitives(
                prims,
                &mut gizmos,
                &mut commands,
                &asset_server,
                &mut cp_labels,
                base_y,
                scale_y,
                refresh_labels,
            );
            cache.cp_labels_dirty = false;
        }
        VisualMode::Polars => {
            crate::plotter::clear_cp_labels(
                &mut commands,
                &mut cp_labels,
            );
            let threads = match sweep.threads {
                0 => None,
                n => Some(n as usize),
            };
            let key = (
                naca_key,
                flow.mach.to_bits(),
                flow.reynolds.to_bits(),
                flow.viscous,
                flow.free_transition,
                sweep.alpha_min_deg.to_bits(),
                sweep.alpha_max_deg.to_bits(),
                sweep.alpha_step_deg.to_bits(),
                sweep.threads,
            );
            if cache.polar_key != Some(key) {
                cache.polar_key = Some(key);
                cache.polar_prims = Some(compute_polar_primitives(
                    &params,
                    &flow,
                    sweep.alpha_min_deg,
                    sweep.alpha_max_deg,
                    sweep.alpha_step_deg,
                    threads,
                    cache.panel_system.as_ref(),
                ));
                cache.polar_labels_dirty = true;
            }

            let refresh_labels = cache.polar_labels_dirty
                || polar_labels.entities.is_empty();
            cache.polar_labels_dirty = false;

            let Some(prims) = cache.polar_prims.as_ref() else {
                return;
            };
            draw_polar_primitives(
                prims,
                &mut gizmos,
                &mut commands,
                &asset_server,
                &mut polar_labels,
                refresh_labels,
                sweep.alpha_min_deg,
                sweep.alpha_max_deg,
            );
        }
        VisualMode::Panels => {
            crate::plotter::clear_cp_labels(
                &mut commands,
                &mut cp_labels,
            );
            crate::plotter::clear_polar_labels(
                &mut commands,
                &mut polar_labels,
            );
            let key = (naca_key, alpha_bits);
            if cache.panel_key != Some(key) {
                let prims = compute_panel_primitives(&cache.body_world);
                cache.panel_key = Some(key);
                cache.panel_prims = prims;
            }
            draw_panel_primitives(&cache.panel_prims, &mut gizmos);
        }
    }
}

/// Map body coordinates (x/c, y/c) to world coords.
fn body_to_world(p: Vec2, alpha_rad: f32, chord_px: f32) -> Vec2 {
    // Positive alpha should pitch the nose up (leading edge up), which is a
    // clockwise rotation in our coordinate system, hence the negative angle.
    let rot = Mat2::from_angle(-alpha_rad);
    let local = Vec2::new((p.x - 0.5) * chord_px, p.y * chord_px);
    rot * local
}

fn draw_grid(gizmos: &mut Gizmos) {
    let spacing = 80.0;
    let extent = 900.0;
    let color = Color::srgb(0.15, 0.15, 0.2).with_alpha(0.35);

    let mut x = -extent;
    while x <= extent {
        gizmos.line_2d(
            Vec2::new(x, -extent),
            Vec2::new(x, extent),
            color,
        );
        x += spacing;
    }

    let mut y = -extent;
    while y <= extent {
        gizmos.line_2d(
            Vec2::new(-extent, y),
            Vec2::new(extent, y),
            color,
        );
        y += spacing;
    }
}
