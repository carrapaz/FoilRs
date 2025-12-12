use bevy::{math::Vec2, prelude::*};

#[derive(Default, Clone)]
pub(super) struct PanelPrimitives {
    pub lines: Vec<(Vec2, Vec2, Color)>,
}

pub(super) fn compute_panel_primitives(
    body_world: &[Vec2],
) -> PanelPrimitives {
    let mut prims = PanelPrimitives::default();

    if body_world.len() < 2 {
        return prims;
    }

    let panel_color = Color::srgb(0.95, 0.85, 0.25).with_alpha(0.9);
    let normal_color = Color::srgb(0.35, 0.95, 0.55).with_alpha(0.8);
    let colloc_color = Color::srgb(0.85, 0.85, 0.9).with_alpha(0.75);

    let normal_len = 18.0;
    let colloc_offset = 2.0;
    let colloc_cross = 2.5;

    let area = polygon_signed_area(body_world);

    for i in 0..body_world.len() - 1 {
        let p0 = body_world[i];
        let p1 = body_world[i + 1];
        prims.lines.push((p0, p1, panel_color));

        let tang = p1 - p0;
        let len = tang.length();
        if len < 1e-6 {
            continue;
        }
        let tangent = tang / len;
        let normal = if area >= 0.0 {
            Vec2::new(tang.y, -tang.x) / len
        } else {
            Vec2::new(-tang.y, tang.x) / len
        };

        let mid = 0.5 * (p0 + p1);
        let n_end = mid + normal * normal_len;
        prims.lines.push((mid, n_end, normal_color));

        let colloc = mid + normal * colloc_offset;
        let ortho = Vec2::new(-tangent.y, tangent.x);
        prims.lines.push((
            colloc - tangent * colloc_cross,
            colloc + tangent * colloc_cross,
            colloc_color,
        ));
        prims.lines.push((
            colloc - ortho * colloc_cross,
            colloc + ortho * colloc_cross,
            colloc_color,
        ));
    }

    if let (Some(first), Some(last)) =
        (body_world.first(), body_world.last())
    {
        if (*first - *last).length_squared() > 1e-6 {
            prims.lines.push((*last, *first, panel_color));
        }
    }

    prims
}

pub(super) fn draw_panel_primitives(
    prims: &PanelPrimitives,
    gizmos: &mut Gizmos,
) {
    for (a, b, c) in &prims.lines {
        gizmos.line_2d(*a, *b, *c);
    }
}

fn polygon_signed_area(points: &[Vec2]) -> f32 {
    if points.len() < 2 {
        return 0.0;
    }
    let mut area = 0.0;
    for i in 0..points.len() - 1 {
        let p0 = points[i];
        let p1 = points[i + 1];
        area += p0.x * p1.y - p1.x * p0.y;
    }
    0.5 * area
}
