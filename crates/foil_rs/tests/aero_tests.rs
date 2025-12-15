use foil_rs::solvers::{
    BoundaryLayerInputs, compute_panel_solution,
    estimate_boundary_layer,
};
use foil_rs::state::{NacaParams, reference_coeffs};

#[test]
fn reference_values_match_xfoil() {
    let params = NacaParams::default();
    let (cl_ref, cm_ref, cdp_ref): (f32, f32, f32) =
        reference_coeffs(&params, 0.0)
            .expect("missing reference for NACA 2412 @ 0 deg");

    assert!((cl_ref - 0.2554).abs() < 1e-4);
    assert!((cm_ref + 0.0557).abs() < 1e-4);
    assert!((cdp_ref + 0.00119).abs() < 1e-5);
}

#[test]
fn panel_solver_reports_lift_positive_at_positive_alpha() {
    let params = NacaParams::default();
    let sol = compute_panel_solution(&params, 4.0);
    let cl = sol.cl().unwrap_or(0.0);
    assert!(cl > 0.05, "expected positive lift at +4deg, got {}", cl);
}

#[test]
fn panel_solver_reports_lift_negative_at_negative_alpha() {
    let params = NacaParams::default();
    let sol = compute_panel_solution(&params, -4.0);
    let cl = sol.cl().unwrap_or(0.0);
    assert!(cl < -0.3, "expected negative lift at -4deg, got {}", cl);
}

#[test]
fn naca2412_at_alpha0_matches_reference_coarsely() {
    let params = NacaParams::default();
    let sol = compute_panel_solution(&params, 0.0);
    let cl = sol.cl().unwrap_or(f32::NAN);
    let cm = sol.cm_c4().unwrap_or(f32::NAN);

    assert!(
        (cl - 0.2554).abs() < 0.05,
        "CL off from reference: got {}",
        cl
    );
    assert!(
        (cm + 0.0557).abs() < 0.1,
        "Cm off from reference: got {}",
        cm
    );
}

#[test]
fn boundary_layer_returns_profile_drag() {
    let params = NacaParams::default();
    let sol = compute_panel_solution(&params, 2.0);
    let inputs =
        BoundaryLayerInputs::new(1_000_000.0, 0.1, true, true, 0.05);
    let bl = estimate_boundary_layer(&sol, &inputs)
        .expect("boundary layer result");
    assert!(
        bl.cd_profile > 0.0005 && bl.cd_profile.is_finite(),
        "unexpected Cd: {}",
        bl.cd_profile
    );
    assert!(!bl.probable_stall, "stall reported for gentle alpha");
}

#[test]
fn cp_upper_is_more_negative_near_le_at_positive_alpha() {
    let params = NacaParams::default();
    let sol = compute_panel_solution(&params, 4.0);
    assert!(
        !sol.x.is_empty(),
        "expected Cp samples, got empty solution"
    );

    let target_x = 0.1;
    let (idx, _) = sol
        .x
        .iter()
        .copied()
        .enumerate()
        .min_by(|(_, a), (_, b)| {
            (a - target_x).abs().total_cmp(&(b - target_x).abs())
        })
        .expect("non-empty");

    let cp_u = sol.cp_upper[idx];
    let cp_l = sol.cp_lower[idx];
    assert!(
        cp_u.is_finite() && cp_l.is_finite(),
        "expected finite Cp values: cp_u={}, cp_l={}",
        cp_u,
        cp_l
    );
    assert!(
        cp_u < 0.0,
        "expected upper-surface suction (Cp<0) near LE, got cp_u={} cp_l={}",
        cp_u,
        cp_l
    );
    assert!(
        cp_u < cp_l,
        "expected upper Cp more negative than lower near LE: cp_u={}, cp_l={}",
        cp_u,
        cp_l
    );
}

#[test]
fn cp_lower_is_more_negative_near_le_at_negative_alpha() {
    let params = NacaParams::default();
    let sol = compute_panel_solution(&params, -4.0);
    assert!(
        !sol.x.is_empty(),
        "expected Cp samples, got empty solution"
    );

    let target_x = 0.1;
    let (idx, _) = sol
        .x
        .iter()
        .copied()
        .enumerate()
        .min_by(|(_, a), (_, b)| {
            (a - target_x).abs().total_cmp(&(b - target_x).abs())
        })
        .expect("non-empty");

    let cp_u = sol.cp_upper[idx];
    let cp_l = sol.cp_lower[idx];
    assert!(
        cp_u.is_finite() && cp_l.is_finite(),
        "expected finite Cp values: cp_u={}, cp_l={}",
        cp_u,
        cp_l
    );
    assert!(
        cp_l < 0.0,
        "expected lower-surface suction (Cp<0) near LE at negative alpha, got cp_u={} cp_l={}",
        cp_u,
        cp_l
    );
    assert!(
        cp_l < cp_u,
        "expected lower Cp more negative than upper near LE at negative alpha: cp_u={}, cp_l={}",
        cp_u,
        cp_l
    );
}
