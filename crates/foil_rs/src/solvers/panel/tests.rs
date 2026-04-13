use super::*;
use crate::state::NacaParams;

fn solve_cl(alpha_deg: f32) -> f32 {
    let params = NacaParams::default();
    let sol = compute_panel_solution(&params, alpha_deg);
    sol.cl().unwrap_or(0.0)
}

fn solve_linear_cl(params: &NacaParams, alpha_deg: f32) -> f32 {
    solve_linear_cl_with_method(
        params,
        alpha_deg,
        super::linear::ExperimentalMethod::NeumannLinearVortex,
    )
}

fn solve_linear_cl_with_method(
    params: &NacaParams,
    alpha_deg: f32,
    method: super::linear::ExperimentalMethod,
) -> f32 {
    super::linear::try_compute_solution_with_method(
        params, alpha_deg, method,
    )
    .and_then(|sol| sol.cl())
    .unwrap_or(f32::NAN)
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

#[test]
fn odd_even_panel_counts_should_match() {
    let even = NacaParams {
        num_points: 160,
        ..Default::default()
    };
    let mut odd = even.clone();
    odd.num_points = 161;

    let cl_even =
        compute_panel_solution(&even, 4.0).cl().unwrap_or(f32::NAN);
    let cl_odd =
        compute_panel_solution(&odd, 4.0).cl().unwrap_or(f32::NAN);
    let diff = (cl_even - cl_odd).abs();

    assert!(
        diff < 0.1,
        "odd/even panel counts diverged: cl_even={}, cl_odd={}, diff={}",
        cl_even,
        cl_odd,
        diff
    );
}

#[test]
fn experimental_linear_solver_stays_finite_for_symmetric_airfoil() {
    let params = NacaParams {
        m_digit: 0.0,
        p_digit: 0.0,
        t_digits: 12.0,
        num_points: 160,
    };

    let cl_pos = solve_linear_cl(&params, 4.0);
    let cl_zero = solve_linear_cl(&params, 0.0);
    let cl_neg = solve_linear_cl(&params, -4.0);

    println!(
        "experimental linear solver finite check: cl(-4)={}, cl(0)={}, cl(+4)={}",
        cl_neg, cl_zero, cl_pos
    );

    assert!(
        cl_neg.is_finite() && cl_neg.abs() < 20.0,
        "expected bounded lift from experimental linear solver, got {}",
        cl_neg
    );
    assert!(
        cl_zero.is_finite() && cl_zero.abs() < 2.0,
        "expected bounded zero-alpha lift from experimental linear solver, got {}",
        cl_zero
    );
    assert!(
        cl_pos.is_finite() && cl_pos.abs() < 20.0,
        "expected bounded lift from experimental linear solver, got {}",
        cl_pos
    );
}

#[test]
fn experimental_linear_solver_responds_to_alpha_changes() {
    let params = NacaParams {
        m_digit: 0.0,
        p_digit: 0.0,
        t_digits: 12.0,
        num_points: 160,
    };

    let cl_neg = solve_linear_cl(&params, -2.0);
    let cl_zero = solve_linear_cl(&params, 0.0);
    let cl_pos = solve_linear_cl(&params, 2.0);
    let delta = (cl_pos - cl_neg).abs();

    println!(
        "experimental linear solver alpha response: cl(-2)={}, cl(0)={}, cl(+2)={}, |delta|={}",
        cl_neg, cl_zero, cl_pos, delta
    );

    assert!(
        cl_zero.is_finite() && cl_zero.abs() < 2.0,
        "expected bounded zero-alpha lift for symmetric airfoil, got {}",
        cl_zero
    );
    assert!(
        delta.is_finite() && delta > 0.25,
        "expected the experimental linear solver to react to alpha changes, got |delta|={}",
        delta
    );
}

#[test]
#[ignore = "diagnostic snapshot for experimental linear-panel state"]
fn experimental_linear_solver_snapshot() {
    let params = NacaParams {
        m_digit: 0.0,
        p_digit: 0.0,
        t_digits: 12.0,
        num_points: 160,
    };

    for alpha_deg in [-4.0_f32, 0.0, 4.0] {
        let snapshot =
            super::linear::debug_snapshot(&params, alpha_deg);
        println!("snapshot alpha={} deg -> {:?}", alpha_deg, snapshot);
    }
}

#[test]
#[ignore = "diagnostic comparison of parallel experimental linear-panel methods"]
fn experimental_linear_solver_compare_parallel_methods() {
    let params = NacaParams {
        m_digit: 0.0,
        p_digit: 0.0,
        t_digits: 12.0,
        num_points: 160,
    };

    let methods = [
        super::linear::ExperimentalMethod::NeumannLinearVortex,
        super::linear::ExperimentalMethod::DirichletDoublet,
    ];

    for method in methods {
        println!("method={}", method.label());
        for alpha_deg in [-4.0_f32, 0.0, 4.0] {
            let snapshot = super::linear::debug_snapshot_for_method(
                &params, alpha_deg, method,
            );
            println!("  alpha={} deg -> {:?}", alpha_deg, snapshot);
        }
    }
}
