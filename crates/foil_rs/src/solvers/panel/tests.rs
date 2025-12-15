use super::*;
use crate::state::NacaParams;

fn solve_cl(alpha_deg: f32) -> f32 {
    let params = NacaParams::default();
    let sol = compute_panel_solution(&params, alpha_deg);
    sol.cl().unwrap_or(0.0)
}

#[test]
fn cl_sign_matches_alpha() {
    let cl_pos = solve_cl(4.0);
    let cl_neg = solve_cl(-4.0);
    assert!(cl_pos > 0.05, "expected positive lift, got {}", cl_pos);
    assert!(cl_neg < 0.0, "expected negative lift, got {}", cl_neg);
}

#[test]
fn cl_scales_with_alpha() {
    let cl_small = solve_cl(2.0);
    let cl_large = solve_cl(6.0);
    assert!(
        cl_large.abs() > cl_small.abs(),
        "expected |lift| to grow with alpha: cl_small={}, cl_large={}",
        cl_small,
        cl_large
    );
}

#[test]
#[ignore]
fn cl_snapshot_alpha0_print() {
    let params = NacaParams::default();
    let sol = compute_panel_solution(&params, 0.0);
    let cl = sol.cl().unwrap_or(0.0);
    println!("debug cl @ 0 deg: {}", cl);
}

#[test]
fn pg_scaling_increases_induced_velocity_with_mach() {
    let params = NacaParams::default();
    let alpha_deg: f32 = 4.0;
    let alpha_rad = alpha_deg.to_radians();
    let freestream = Vec2::new(alpha_rad.cos(), alpha_rad.sin());

    let system =
        PanelLuSystem::new(&params).expect("panel system should build");
    let flow = system
        .solve_flow(alpha_deg)
        .expect("panel flow should solve");
    let p = Vec2::new(0.3, 0.05);

    let v0 = flow.velocity_body_pg(p, 0.0);
    let v1 = flow.velocity_body_pg(p, 0.6);

    let induced0 = v0 - freestream;
    let induced1 = v1 - freestream;

    assert!(
        induced1.length() > induced0.length() * 1.05,
        "expected induced velocity to increase with Mach: |i0|={}, |i1|={}",
        induced0.length(),
        induced1.length()
    );
}
