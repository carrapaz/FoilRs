use crate::{state::FlowSettings, views::CHORD_PX};
use bevy::{
    math::{Mat2, Vec2},
    prelude::*,
};

const BODY_FADE_INNER: f32 = 0.01;
const BODY_FADE_OUTER: f32 = 0.18;
const STREAM_STEP: f32 = 0.03;
const STREAM_MAX_STEPS: usize = 320;

#[derive(Default, Clone)]
pub(super) struct FieldPrimitives {
    pub arrow_lines: Vec<(Vec2, Vec2, Color)>,
    pub streamlines: Vec<Vec<Vec2>>,
}

pub(super) fn compute_field_primitives(
    flow: &FlowSettings,
    body: &[Vec2],
    panel_system: Option<&crate::solvers::panel::PanelLuSystem>,
) -> FieldPrimitives {
    let mut prims = FieldPrimitives::default();
    let alpha_rad = flow.alpha_deg.to_radians();
    let panel_flow =
        panel_system.and_then(|sys| sys.solve_flow(flow.alpha_deg));
    compute_arrow_lines(
        alpha_rad,
        flow,
        body,
        panel_flow.as_ref(),
        &mut prims.arrow_lines,
    );
    compute_streamlines(
        alpha_rad,
        flow,
        body,
        panel_flow.as_ref(),
        &mut prims.streamlines,
    );
    prims
}

pub(super) fn draw_field_primitives(
    prims: &FieldPrimitives,
    gizmos: &mut Gizmos,
) {
    for (a, b, c) in &prims.arrow_lines {
        gizmos.line_2d(*a, *b, *c);
    }
    for pts in &prims.streamlines {
        if pts.len() >= 2 {
            gizmos.linestrip_2d(
                pts.iter().copied(),
                Color::srgb(0.5, 0.9, 1.0),
            );
        }
    }
}

fn compute_arrow_lines(
    alpha_rad: f32,
    flow_settings: &FlowSettings,
    body: &[Vec2],
    flow: Option<&crate::solvers::panel::PanelFlow<'_>>,
    out: &mut Vec<(Vec2, Vec2, Color)>,
) {
    let nx = 24;
    let ny = 16;
    let x_min = -1.2;
    let x_max = 2.0;
    let y_min = -0.8;
    let y_max = 0.8;

    for iy in 0..ny {
        let ty = iy as f32 / (ny - 1) as f32;
        let y = y_min + (y_max - y_min) * ty;

        for ix in 0..nx {
            let tx = ix as f32 / (nx - 1) as f32;
            let x = x_min + (x_max - x_min) * tx;

            let p_world = Vec2::new((x - 0.5) * CHORD_PX, y * CHORD_PX);
            let p_body = world_to_body(p_world, alpha_rad);
            if polygon_contains_point(p_body, body) {
                continue;
            }
            let boundary_dist = distance_to_airfoil(p_body, body);
            let mut inner = BODY_FADE_INNER;
            let mut outer = BODY_FADE_OUTER;
            if flow_settings.viscous {
                let re = flow_settings.reynolds.max(1e3);
                let x = p_body.x.clamp(0.05, 1.0);
                let delta =
                    (5.0 * x.sqrt() / re.sqrt()).clamp(0.001, 0.06);
                inner += delta;
                outer += 2.5 * delta;
            }

            if boundary_dist < inner {
                continue;
            }
            let fade = ((boundary_dist - inner) / (outer - inner))
                .clamp(0.0, 1.0);

            let v_body = match flow {
                Some(flow) => {
                    flow.velocity_body_pg(p_body, flow_settings.mach)
                }
                None => toy_flow_velocity_body(p_body, alpha_rad),
            };
            let v_world = body_vec_to_world(v_body, alpha_rad);
            let speed = v_world.length();
            if speed < 1e-3 || fade <= 0.02 {
                continue;
            }

            let dir_world = v_world / speed;
            let start_world = p_world;

            let arrow_len = 40.0
                * (0.35 + 0.65 * (speed / 3.0).clamp(0.0, 1.0))
                * fade;
            let end_world = start_world + dir_world * arrow_len;
            let head = start_world + dir_world * (arrow_len * 0.2);
            let tail = start_world - dir_world * (arrow_len * 0.1);

            let t = (speed / 3.0).clamp(0.0, 1.0) * fade;
            let color = Color::hsl(205.0 - 70.0 * t, 0.75, 0.55);

            out.push((tail, end_world, color.with_alpha(0.6)));

            let ortho = Vec2::new(-dir_world.y, dir_world.x)
                .normalize_or_zero();
            let head_left = head + ortho * (arrow_len * 0.12);
            let head_right = head - ortho * (arrow_len * 0.12);
            out.push((end_world, head_left, color.with_alpha(0.8)));
            out.push((end_world, head_right, color.with_alpha(0.8)));
        }
    }
}

fn compute_streamlines(
    alpha_rad: f32,
    flow_settings: &FlowSettings,
    body: &[Vec2],
    flow: Option<&crate::solvers::panel::PanelFlow<'_>>,
    out: &mut Vec<Vec<Vec2>>,
) {
    let seeds_y = [-0.6, -0.4, -0.2, 0.0, 0.2, 0.4, 0.6];
    let x_max = 2.0;

    for &y0 in &seeds_y {
        let mut pts = Vec::new();
        let mut p_world =
            Vec2::new((-1.3 - 0.5) * CHORD_PX, y0 * CHORD_PX);

        for _ in 0..STREAM_MAX_STEPS {
            let p_body = world_to_body(p_world, alpha_rad);
            if polygon_contains_point(p_body, body) {
                break;
            }
            pts.push(p_world);

            let v_body = match flow {
                Some(flow) => {
                    flow.velocity_body_pg(p_body, flow_settings.mach)
                }
                None => toy_flow_velocity_body(p_body, alpha_rad),
            };
            let v_world = body_vec_to_world(v_body, alpha_rad);
            let speed = v_world.length();
            if speed < 1e-4 {
                break;
            }
            let dir = v_world / speed;
            p_world += dir * (STREAM_STEP * CHORD_PX);

            let mut inner = BODY_FADE_INNER;
            if flow_settings.viscous {
                let re = flow_settings.reynolds.max(1e3);
                let x = p_body.x.clamp(0.05, 1.0);
                let delta =
                    (5.0 * x.sqrt() / re.sqrt()).clamp(0.001, 0.06);
                inner += delta;
            }

            if p_body.x > x_max + 0.1
                || p_body.y.abs() > 1.5
                || distance_to_airfoil(p_body, body) < inner
            {
                break;
            }
        }

        if pts.len() >= 2 {
            out.push(pts);
        }
    }
}

fn world_to_body(p_world: Vec2, alpha_rad: f32) -> Vec2 {
    let rot_inv = Mat2::from_angle(alpha_rad);
    let local = rot_inv * p_world;
    Vec2::new(local.x / CHORD_PX + 0.5, local.y / CHORD_PX)
}

fn body_vec_to_world(v_body: Vec2, alpha_rad: f32) -> Vec2 {
    let rot = Mat2::from_angle(-alpha_rad);
    rot * v_body
}

/// Fallback if the panel flow solve fails.
fn toy_flow_velocity_body(p: Vec2, alpha_rad: f32) -> Vec2 {
    use std::f32::consts::PI;

    // Keep world freestream fixed while the airfoil rotates: in body coordinates
    // this means the freestream rotates with +alpha.
    let u_inf = Vec2::new(alpha_rad.cos(), alpha_rad.sin());

    let vortex_pos = Vec2::new(0.25, 0.0);
    let r = p - vortex_pos;
    let r2 = r.length_squared().max(1e-4);
    let r_len = r2.sqrt();

    // Circulation roughly proportional to α
    let gamma = 4.0 * PI * alpha_rad;

    let tangential_dir = if r_len > 0.0 {
        Vec2::new(-r.y, r.x) / r_len
    } else {
        Vec2::ZERO
    };

    let v_vortex = tangential_dir * (gamma / (2.0 * PI * r_len));

    u_inf + v_vortex
}

fn polygon_contains_point(p: Vec2, poly: &[Vec2]) -> bool {
    if poly.len() < 3 {
        return false;
    }
    let mut inside = false;
    let mut j = poly.len() - 1;
    for i in 0..poly.len() {
        let pi = poly[i];
        let pj = poly[j];
        let intersects = ((pi.y > p.y) != (pj.y > p.y))
            && (p.x
                < (pj.x - pi.x) * (p.y - pi.y)
                    / (pj.y - pi.y + f32::EPSILON)
                    + pi.x);
        if intersects {
            inside = !inside;
        }
        j = i;
    }
    inside
}

fn distance_to_airfoil(p: Vec2, poly: &[Vec2]) -> f32 {
    if poly.len() < 2 {
        return f32::MAX;
    }
    let mut min_dist = f32::MAX;
    for i in 0..poly.len() - 1 {
        let a = poly[i];
        let b = poly[i + 1];
        let dist = distance_point_segment(p, a, b);
        if dist < min_dist {
            min_dist = dist;
        }
    }
    min_dist
}

fn distance_point_segment(p: Vec2, a: Vec2, b: Vec2) -> f32 {
    let ab = b - a;
    let t = ((p - a).dot(ab)) / ab.length_squared().max(1e-6);
    let t_clamped = t.clamp(0.0, 1.0);
    (a + ab * t_clamped - p).length()
}
