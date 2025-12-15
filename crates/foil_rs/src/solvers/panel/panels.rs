use crate::math::Vec2;

pub(crate) struct Panel {
    pub(crate) start: Vec2,
    pub(crate) mid: Vec2,
    pub(crate) normal: Vec2,
    pub(crate) tangent: Vec2,
    pub(crate) length: f32,
}

pub(crate) fn build_panels(points: &[Vec2]) -> Vec<Panel> {
    let mut panels = Vec::with_capacity(points.len() - 1);
    let area = polygon_signed_area(points);

    for i in 0..points.len() - 1 {
        let p0 = points[i];
        let p1 = points[i + 1];
        let mid = 0.5 * (p0 + p1);
        let tang = p1 - p0;
        let len = tang.length().max(1e-6);
        let tangent = tang / len;
        let normal = if area >= 0.0 {
            Vec2::new(tang.y, -tang.x) / len
        } else {
            Vec2::new(-tang.y, tang.x) / len
        };
        panels.push(Panel {
            start: p0,
            mid,
            normal,
            tangent,
            length: len,
        });
    }

    panels
}

fn polygon_signed_area(points: &[Vec2]) -> f32 {
    let mut area = 0.0;
    for i in 0..points.len() - 1 {
        let p0 = points[i];
        let p1 = points[i + 1];
        area += p0.x * p1.y - p1.x * p0.y;
    }
    0.5 * area
}
