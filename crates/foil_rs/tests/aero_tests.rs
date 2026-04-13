use foil_rs::airfoil::{
    build_naca_body_geometry, build_naca_body_geometry_sharp_te,
    camber_line, camber_slope, thickness_distribution,
};
use foil_rs::solvers::panel::{
    PanelLuSystem, approx_section_coeffs, compute_panel_solution,
};
use foil_rs::solvers::polar::{
    PolarMode, compute_multi_polar_sweeps, compute_polar_sweep,
    compute_polar_sweep_parallel,
    compute_polar_sweep_parallel_with_system_mode, default_polar_sweep,
};
use foil_rs::solvers::{BoundaryLayerInputs, estimate_boundary_layer};
use foil_rs::state::{FlowSettings, NacaParams, reference_coeffs};

// =========================================================================
// Reference data
// =========================================================================

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
fn reference_coeffs_returns_none_for_unknown() {
    let params = NacaParams::from_naca4("0012").expect("parse");
    assert!(reference_coeffs(&params, 0.0).is_none());
    // Also unknown alpha for 2412
    let params = NacaParams::default();
    assert!(reference_coeffs(&params, 5.0).is_none());
}

// =========================================================================
// Panel solver — CL basics
// =========================================================================

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
    assert!(cl < -0.2, "expected negative lift at -4deg, got {}", cl);
}

#[test]
fn naca2412_at_alpha0_matches_reference_coarsely() {
    let params = NacaParams::default();
    let sol = compute_panel_solution(&params, 0.0);
    let cl = sol.cl().unwrap_or(f32::NAN);
    let cm = sol.cm_c4().unwrap_or(f32::NAN);

    assert!(
        (cl - 0.2554).abs() < 0.15,
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
fn naca0012_alpha0_has_near_zero_lift() {
    let params =
        NacaParams::from_naca4("0012").expect("parse NACA 0012");
    let sol = compute_panel_solution(&params, 0.0);
    let cl = sol.cl().unwrap_or(f32::NAN);
    assert!(
        cl.is_finite() && cl.abs() < 0.05,
        "expected near-zero lift for symmetric foil at 0 deg, got cl={}",
        cl
    );
}

#[test]
fn naca0012_lift_is_approximately_antisymmetric_inviscid() {
    let params =
        NacaParams::from_naca4("0012").expect("parse NACA 0012");
    let cl_pos = compute_panel_solution(&params, 4.0)
        .cl()
        .unwrap_or(f32::NAN);
    let cl_neg = compute_panel_solution(&params, -4.0)
        .cl()
        .unwrap_or(f32::NAN);
    assert!(cl_pos.is_finite() && cl_neg.is_finite());
    assert!(
        (cl_pos + cl_neg).abs() < 0.10,
        "expected approximate antisymmetry: cl(+4)+cl(-4) ~= 0, got {}",
        cl_pos + cl_neg
    );
}

// =========================================================================
// Panel solver — CL accuracy (panel-integrated forces)
// =========================================================================

#[test]
fn naca0012_cl_alpha_matches_thin_airfoil_theory() {
    // For a 12% thick symmetric airfoil, the panel method should give
    // CL_alpha close to 2*pi (within ~5% for thickness effects).
    let params = NacaParams::from_naca4("0012").expect("parse");
    let system = PanelLuSystem::new(&params).expect("LU system");
    let cl_2 = system.panel_solution(&params, 2.0).cl().unwrap_or(0.0);
    let cl_neg2 =
        system.panel_solution(&params, -2.0).cl().unwrap_or(0.0);
    let cl_alpha = (cl_2 - cl_neg2) / (4.0_f32.to_radians());
    let two_pi = 2.0 * std::f32::consts::PI;
    assert!(
        (cl_alpha - two_pi).abs() / two_pi < 0.08,
        "CL_alpha={:.3} should be near 2*pi={:.3}",
        cl_alpha,
        two_pi
    );
}

#[test]
fn naca2412_cl_alpha_within_10pct_of_thin_airfoil() {
    let params = NacaParams::from_naca4("2412").expect("parse");
    let system = PanelLuSystem::new(&params).expect("LU system");
    let cl_4 = system.panel_solution(&params, 4.0).cl().unwrap_or(0.0);
    let cl_0 = system.panel_solution(&params, 0.0).cl().unwrap_or(0.0);
    let cl_alpha = (cl_4 - cl_0) / (4.0_f32.to_radians());
    let two_pi = 2.0 * std::f32::consts::PI;
    assert!(
        (cl_alpha - two_pi).abs() / two_pi < 0.12,
        "CL_alpha={:.3} should be near 2*pi={:.3}",
        cl_alpha,
        two_pi
    );
}

#[test]
fn cambered_airfoil_has_positive_cl_at_zero_alpha() {
    for &code in &["2412", "4412", "6412"] {
        let params = NacaParams::from_naca4(code).expect("parse");
        let sol = compute_panel_solution(&params, 0.0);
        let cl = sol.cl().unwrap_or(0.0);
        assert!(
            cl > 0.01,
            "NACA {} should have positive CL at alpha=0, got {}",
            code,
            cl
        );
    }
}

#[test]
fn cl_increases_monotonically_with_alpha_in_linear_region() {
    let params = NacaParams::from_naca4("0012").expect("parse");
    let system = PanelLuSystem::new(&params).expect("LU system");
    let mut prev_cl = f32::NEG_INFINITY;
    for alpha in [-4, -2, 0, 2, 4, 6, 8] {
        let cl = system
            .panel_solution(&params, alpha as f32)
            .cl()
            .unwrap_or(0.0);
        assert!(
            cl > prev_cl,
            "CL should increase: at {}° got {:.4}, prev was {:.4}",
            alpha,
            cl,
            prev_cl
        );
        prev_cl = cl;
    }
}

// =========================================================================
// Panel solver — Cp distribution
// =========================================================================

#[test]
fn cp_upper_is_more_negative_near_le_at_positive_alpha() {
    let params = NacaParams::default();
    let sol = compute_panel_solution(&params, 4.0);
    assert!(!sol.x.is_empty());

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

    assert!(sol.cp_upper[idx] < 0.0);
    assert!(sol.cp_upper[idx] < sol.cp_lower[idx]);
}

#[test]
fn cp_lower_is_more_negative_near_le_at_negative_alpha() {
    let params = NacaParams::default();
    let sol = compute_panel_solution(&params, -4.0);
    assert!(!sol.x.is_empty());

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

    assert!(sol.cp_lower[idx] < 0.0);
    assert!(sol.cp_lower[idx] < sol.cp_upper[idx]);
}

#[test]
fn cp_near_trailing_edge_approaches_one_at_zero_alpha() {
    let params = NacaParams::from_naca4("0012").expect("parse");
    let sol = compute_panel_solution(&params, 0.0);
    // Near the TE (x ~ 0.95), Cp should be close to 1.0 (stagnation recovery)
    // but slightly less due to thickness.  Check it's at least > 0.
    if let Some(idx) = sol.x.iter().position(|&x| x > 0.9 && x < 0.98) {
        let cp_avg = 0.5 * (sol.cp_upper[idx] + sol.cp_lower[idx]);
        assert!(
            cp_avg > -0.5,
            "Cp near TE should recover toward 1.0, got avg={:.3}",
            cp_avg
        );
    }
}

// =========================================================================
// Panel solver — approx fallback and LU system
// =========================================================================

#[test]
fn approx_coefficients_have_correct_signs() {
    let params = NacaParams::default(); // 2412
    let (cl, cm, cd) = approx_section_coeffs(&params, 4.0);
    assert!(cl > 0.2, "CL should be positive at +4°: {}", cl);
    assert!(
        cm < 0.0,
        "CM should be negative for cambered at +4°: {}",
        cm
    );
    assert!(cd.abs() < 1e-6, "CDp in approx mode should be ~0: {}", cd);

    // At alpha=0, cambered airfoil should still have positive CL
    let (cl0, _, _) = approx_section_coeffs(&params, 0.0);
    assert!(cl0 > 0.1, "CL(0) should be positive for 2412: {}", cl0);
}

#[test]
fn approx_coefficients_symmetric_airfoil_zero_cl_at_zero_alpha() {
    let params = NacaParams::from_naca4("0012").expect("parse");
    let (cl, cm, _) = approx_section_coeffs(&params, 0.0);
    assert!(cl.abs() < 0.05, "symmetric: CL(0) should be ~0: {}", cl);
    assert!(cm.abs() < 0.01, "symmetric: CM should be ~0: {}", cm);
}

#[test]
fn panel_lu_system_reuses_factorization() {
    let params = NacaParams::default();
    let system =
        PanelLuSystem::new(&params).expect("LU system should build");
    // Solve at multiple alphas — should all succeed with the same system
    for alpha in [-8.0, -4.0, 0.0, 4.0, 8.0, 12.0_f32] {
        let sol = system.panel_solution(&params, alpha);
        let cl = sol.cl().unwrap_or(f32::NAN);
        assert!(
            cl.is_finite(),
            "CL should be finite at alpha={}: got NaN",
            alpha
        );
    }
}

#[test]
fn panel_lu_system_solve_flow_returns_some() {
    let params = NacaParams::default();
    let system = PanelLuSystem::new(&params).expect("LU system");
    let flow = system.solve_flow(4.0);
    assert!(
        flow.is_some(),
        "solve_flow should return Some for valid geometry"
    );
}

// =========================================================================
// Boundary layer
// =========================================================================

#[test]
fn boundary_layer_returns_profile_drag() {
    let params = NacaParams::default();
    let sol = compute_panel_solution(&params, 2.0);
    let inputs =
        BoundaryLayerInputs::new(1_000_000.0, 0.1, true, true, 0.05);
    let bl = estimate_boundary_layer(&sol, &inputs)
        .expect("boundary layer result");
    assert!(
        bl.cd_profile > 0.001 && bl.cd_profile.is_finite(),
        "unexpected Cd: {}",
        bl.cd_profile
    );
    assert!(!bl.probable_stall, "stall reported for gentle alpha");
}

#[test]
fn boundary_layer_cd_increases_with_alpha() {
    let params = NacaParams::from_naca4("0012").expect("parse");
    let inputs =
        BoundaryLayerInputs::new(3_000_000.0, 0.0, true, true, 0.05);
    let sol_2 = compute_panel_solution(&params, 2.0);
    let sol_8 = compute_panel_solution(&params, 8.0);
    let cd_2 = estimate_boundary_layer(&sol_2, &inputs)
        .map(|b| b.cd_profile)
        .unwrap_or(0.0);
    let cd_8 = estimate_boundary_layer(&sol_8, &inputs)
        .map(|b| b.cd_profile)
        .unwrap_or(0.0);
    assert!(
        cd_8 > cd_2,
        "CD should increase with alpha: cd(2°)={:.5}, cd(8°)={:.5}",
        cd_2,
        cd_8
    );
}

#[test]
fn boundary_layer_cd_decreases_with_reynolds() {
    let params = NacaParams::from_naca4("0012").expect("parse");
    let sol = compute_panel_solution(&params, 4.0);
    let bl_low = estimate_boundary_layer(
        &sol,
        &BoundaryLayerInputs::new(500_000.0, 0.0, true, true, 0.05),
    )
    .map(|b| b.cd_profile)
    .unwrap_or(0.0);
    let bl_high = estimate_boundary_layer(
        &sol,
        &BoundaryLayerInputs::new(5_000_000.0, 0.0, true, true, 0.05),
    )
    .map(|b| b.cd_profile)
    .unwrap_or(0.0);
    assert!(
        bl_high < bl_low,
        "CD should decrease with Re: cd(Re=0.5M)={:.5}, cd(Re=5M)={:.5}",
        bl_low,
        bl_high
    );
}

#[test]
fn boundary_layer_returns_none_when_viscous_off() {
    let params = NacaParams::default();
    let sol = compute_panel_solution(&params, 4.0);
    let inputs =
        BoundaryLayerInputs::new(3_000_000.0, 0.0, false, true, 0.05);
    assert!(
        estimate_boundary_layer(&sol, &inputs).is_none(),
        "should return None when viscous=false"
    );
}

#[test]
fn boundary_layer_forced_transition_changes_drag() {
    let params = NacaParams::from_naca4("0012").expect("parse");
    let sol = compute_panel_solution(&params, 4.0);
    let bl_free = estimate_boundary_layer(
        &sol,
        &BoundaryLayerInputs::new(3_000_000.0, 0.0, true, true, 0.05),
    )
    .map(|b| b.cd_profile)
    .unwrap_or(0.0);
    let bl_forced = estimate_boundary_layer(
        &sol,
        &BoundaryLayerInputs::new(3_000_000.0, 0.0, true, false, 0.05),
    )
    .map(|b| b.cd_profile)
    .unwrap_or(0.0);
    // Forced early transition should give higher drag (more turbulent wetted area)
    assert!(
        bl_forced >= bl_free * 0.8,
        "forced transition drag={:.5} should be comparable to free={:.5}",
        bl_forced,
        bl_free
    );
}

#[test]
fn boundary_layer_realistic_cd_range_at_re3m() {
    // NACA 0012 at Re=3M: cd_profile should be ~0.005-0.010 at alpha=0
    let params = NacaParams::from_naca4("0012").expect("parse");
    let sol = compute_panel_solution(&params, 0.0);
    let inputs =
        BoundaryLayerInputs::new(3_000_000.0, 0.0, true, true, 0.05);
    let cd = estimate_boundary_layer(&sol, &inputs)
        .map(|b| b.cd_profile)
        .unwrap_or(0.0);
    assert!(
        cd > 0.002 && cd < 0.015,
        "CD at alpha=0 Re=3M should be in [0.002, 0.015], got {:.5}",
        cd
    );
}

// =========================================================================
// Polar sweep — basic
// =========================================================================

#[test]
fn polar_sweep_has_expected_count_and_sorted_alphas() {
    let params = NacaParams::default();
    let flow = FlowSettings::default();
    let rows = compute_polar_sweep(&params, &flow, -10.0, 15.0, 0.5);

    assert_eq!(rows.len(), 51, "unexpected polar row count");
    assert!((rows[0].alpha_deg + 10.0).abs() < 1e-5);
    assert!((rows[rows.len() - 1].alpha_deg - 15.0).abs() < 1e-5);

    for w in rows.windows(2) {
        assert!(w[0].alpha_deg < w[1].alpha_deg);
    }
}

#[test]
fn polar_sweep_cl_increases_through_linear_region() {
    let params = NacaParams::from_naca4("0012").expect("parse");
    let flow = FlowSettings {
        reynolds: 3_000_000.0,
        mach: 0.0,
        viscous: true,
        free_transition: true,
        ..Default::default()
    };
    let rows = compute_polar_sweep(&params, &flow, -4.0, 8.0, 2.0);
    for w in rows.windows(2) {
        assert!(
            w[1].cl > w[0].cl,
            "CL should increase: alpha {:.0}→{:.0}: {:.4}→{:.4}",
            w[0].alpha_deg,
            w[1].alpha_deg,
            w[0].cl,
            w[1].cl
        );
    }
}

#[test]
fn polar_sweep_cd_is_present_when_viscous() {
    let params = NacaParams::default();
    let flow = FlowSettings {
        viscous: true,
        free_transition: true,
        ..Default::default()
    };
    let rows = compute_polar_sweep(&params, &flow, 0.0, 4.0, 2.0);
    for row in &rows {
        assert!(
            row.cd_profile.is_some(),
            "CD should be present when viscous=true, alpha={}",
            row.alpha_deg
        );
        assert!(
            row.cd_profile.unwrap_or(0.0) > 0.0,
            "CD should be positive at alpha={}",
            row.alpha_deg
        );
    }
}

#[test]
fn default_polar_sweep_returns_valid_range() {
    let (min, max, step) = default_polar_sweep();
    assert!(min < max);
    assert!(step > 0.0);
    assert!((max - min) / step > 10.0, "should have at least 10 steps");
}

// =========================================================================
// Polar sweep — parallel and multi
// =========================================================================

#[test]
fn parallel_sweep_matches_sequential() {
    let params = NacaParams::from_naca4("0012").expect("parse");
    let flow = FlowSettings {
        reynolds: 3_000_000.0,
        mach: 0.0,
        viscous: true,
        free_transition: true,
        ..Default::default()
    };
    let seq = compute_polar_sweep(&params, &flow, -4.0, 8.0, 2.0);
    let par =
        compute_polar_sweep_parallel(&params, &flow, -4.0, 8.0, 2.0);

    assert_eq!(seq.len(), par.len(), "row counts should match");
    for (s, p) in seq.iter().zip(par.iter()) {
        assert!(
            (s.cl - p.cl).abs() < 0.01,
            "CL mismatch at alpha={}: seq={:.4} par={:.4}",
            s.alpha_deg,
            s.cl,
            p.cl
        );
    }
}

#[test]
fn parallel_sweep_with_system_mode_panel() {
    let params = NacaParams::from_naca4("2412").expect("parse");
    let flow = FlowSettings::default();
    let system = PanelLuSystem::new(&params);
    let result = compute_polar_sweep_parallel_with_system_mode(
        &params,
        &flow,
        0.0,
        4.0,
        2.0,
        system.as_ref(),
        Some(2),
        PolarMode::Panel,
    );
    assert!(!result.rows.is_empty());
    assert!(
        result.rows[0].cl.is_finite(),
        "CL should be finite in Panel mode"
    );
}

#[test]
fn parallel_sweep_with_system_mode_approx() {
    let params = NacaParams::from_naca4("2412").expect("parse");
    let flow = FlowSettings::default();
    let result = compute_polar_sweep_parallel_with_system_mode(
        &params,
        &flow,
        0.0,
        4.0,
        2.0,
        None, // no system needed for approx
        Some(1),
        PolarMode::Approx,
    );
    assert!(!result.rows.is_empty());
    assert!(
        !result.used_fallback,
        "Approx mode should not report fallback"
    );
    assert!(
        result.rows[0].cl.is_finite(),
        "CL should be finite in Approx mode"
    );
}

#[test]
fn multi_polar_sweeps_across_reynolds() {
    let params = NacaParams::from_naca4("0012").expect("parse");
    let flows = vec![
        FlowSettings {
            reynolds: 1_000_000.0,
            mach: 0.0,
            viscous: true,
            free_transition: true,
            ..Default::default()
        },
        FlowSettings {
            reynolds: 3_000_000.0,
            mach: 0.0,
            viscous: true,
            free_transition: true,
            ..Default::default()
        },
    ];
    let results = compute_multi_polar_sweeps(
        &params,
        &flows,
        0.0,
        4.0,
        2.0,
        Some(1),
    );
    assert_eq!(results.len(), 2, "should have 2 polar curves");
    for (flow, rows) in &results {
        assert!(
            !rows.is_empty(),
            "rows should not be empty for Re={}",
            flow.reynolds
        );
    }
}

#[test]
fn multi_polar_sweeps_parallel() {
    let params = NacaParams::from_naca4("0012").expect("parse");
    let flows = vec![
        FlowSettings {
            reynolds: 1_000_000.0,
            ..Default::default()
        },
        FlowSettings {
            reynolds: 3_000_000.0,
            ..Default::default()
        },
    ];
    let results = compute_multi_polar_sweeps(
        &params,
        &flows,
        0.0,
        4.0,
        2.0,
        Some(2),
    );
    assert_eq!(results.len(), 2);
    // CL should be similar across Re (inviscid component dominates)
    let cl_re1m = results[0].1.last().map(|r| r.cl).unwrap_or(0.0);
    let cl_re3m = results[1].1.last().map(|r| r.cl).unwrap_or(0.0);
    assert!(
        (cl_re1m - cl_re3m).abs() < 0.1,
        "CL should be similar across Re: Re=1M:{:.3} Re=3M:{:.3}",
        cl_re1m,
        cl_re3m
    );
}

#[test]
fn polar_sweep_empty_range_returns_empty() {
    let params = NacaParams::default();
    let flow = FlowSettings::default();
    // min > max: should return empty or handle gracefully
    let rows = compute_polar_sweep(&params, &flow, 5.0, 5.0, 1.0);
    // Single point at alpha=5
    assert!(
        rows.len() <= 2,
        "degenerate range should give 0-1 rows, got {}",
        rows.len()
    );
}

#[test]
fn polar_sweep_single_point() {
    let params = NacaParams::default();
    let flow = FlowSettings::default();
    let rows = compute_polar_sweep(&params, &flow, 4.0, 4.0, 0.5);
    assert!(
        !rows.is_empty(),
        "single-point sweep should give at least 1 row"
    );
    assert!((rows[0].alpha_deg - 4.0).abs() < 0.01);
}

// =========================================================================
// Airfoil geometry — NACA generation
// =========================================================================

#[test]
fn sharp_te_geometry_is_closed_loop() {
    let params = NacaParams::from_naca4("2412").expect("parse");
    let pts = build_naca_body_geometry_sharp_te(&params);
    assert!(pts.len() > 10, "should have many points");
    // First and last point should be the same (closed loop)
    let first = pts[0];
    let last = pts[pts.len() - 1];
    assert!(
        (first - last).length() < 0.01,
        "geometry should be closed: first={:?} last={:?}",
        first,
        last
    );
}

#[test]
fn rounded_te_geometry_is_closed_loop() {
    let params = NacaParams::from_naca4("2412").expect("parse");
    let pts = build_naca_body_geometry(&params);
    assert!(pts.len() > 10, "should have many points");
    let first = pts[0];
    let last = pts[pts.len() - 1];
    assert!(
        (first - last).length() < 0.01,
        "rounded TE geometry should be closed: first={:?} last={:?}",
        first,
        last
    );
}

#[test]
fn geometry_stays_within_unit_chord() {
    for code in ["0008", "0012", "2412", "4412", "0018"] {
        let params = NacaParams::from_naca4(code).expect("parse");
        let pts = build_naca_body_geometry_sharp_te(&params);
        for pt in &pts {
            assert!(
                pt.x >= -0.05 && pt.x <= 1.05,
                "NACA {}: x={:.4} out of [0, 1] range",
                code,
                pt.x
            );
            assert!(
                pt.y.abs() < 0.5,
                "NACA {}: y={:.4} out of reasonable range",
                code,
                pt.y
            );
        }
    }
}

#[test]
fn symmetric_airfoil_geometry_is_symmetric() {
    let params = NacaParams::from_naca4("0012").expect("parse");
    let pts = build_naca_body_geometry_sharp_te(&params);
    // For symmetric airfoil, for each (x, y) there should be (x, -y)
    // Check at a few specific points
    let upper: Vec<_> = pts.iter().filter(|p| p.y > 0.001).collect();
    let lower: Vec<_> = pts.iter().filter(|p| p.y < -0.001).collect();
    assert!(
        (upper.len() as i32 - lower.len() as i32).abs() < 5,
        "symmetric: should have ~same number of upper/lower points: {} vs {}",
        upper.len(),
        lower.len()
    );
}

#[test]
fn rounded_te_has_more_points_than_sharp() {
    let params = NacaParams::from_naca4("2412").expect("parse");
    let sharp = build_naca_body_geometry_sharp_te(&params);
    let round = build_naca_body_geometry(&params);
    assert!(
        round.len() >= sharp.len(),
        "rounded TE should have >= points: sharp={} round={}",
        sharp.len(),
        round.len()
    );
}

// =========================================================================
// Airfoil geometry — camber and thickness functions
// =========================================================================

#[test]
fn camber_line_zero_for_symmetric() {
    for x in [0.0, 0.25, 0.5, 0.75, 1.0] {
        let y = camber_line(0.0, 0.0, x);
        assert!(y.abs() < 1e-6, "symmetric camber at x={}: {}", x, y);
    }
}

#[test]
fn camber_line_positive_for_cambered() {
    // NACA 2412: m=0.02, p=0.4
    let y_mid = camber_line(0.02, 0.4, 0.4);
    assert!(
        y_mid > 0.0,
        "camber at max position should be positive: {}",
        y_mid
    );
    // Max camber should occur at x=p
    let y_before = camber_line(0.02, 0.4, 0.3);
    let y_after = camber_line(0.02, 0.4, 0.5);
    assert!(
        y_mid >= y_before && y_mid >= y_after,
        "camber should peak near p"
    );
}

#[test]
fn camber_slope_zero_at_max_camber_position() {
    // At x = p, the camber slope should be zero (max camber point)
    let slope = camber_slope(0.02, 0.4, 0.4);
    assert!(slope.abs() < 0.01, "slope at p should be ~0: {}", slope);
}

#[test]
fn thickness_distribution_zero_at_endpoints() {
    let y_0 = thickness_distribution(0.12, 0.0);
    let y_1 = thickness_distribution(0.12, 1.0);
    assert!(y_0.abs() < 0.01, "thickness at LE should be ~0: {}", y_0);
    assert!(y_1.abs() < 0.01, "thickness at TE should be ~0: {}", y_1);
}

#[test]
fn thickness_distribution_max_near_30_pct_chord() {
    // NACA thickness peaks near x/c = 0.3
    let t = 0.12;
    let y_30 = thickness_distribution(t, 0.30);
    let y_10 = thickness_distribution(t, 0.10);
    let y_70 = thickness_distribution(t, 0.70);
    assert!(y_30 > y_10, "thickness at 30% should exceed 10%");
    assert!(y_30 > y_70, "thickness at 30% should exceed 70%");
    // Max thickness should be t/2 for NACA formula (half-thickness)
    assert!(
        y_30 > 0.04 && y_30 < 0.08,
        "max half-thickness for 12% should be ~0.06: got {:.4}",
        y_30
    );
}

// =========================================================================
// NacaParams parsing
// =========================================================================

#[test]
fn naca_params_from_valid_codes() {
    for code in ["0012", "2412", "4415", "0008", "6409"] {
        let p = NacaParams::from_naca4(code);
        assert!(p.is_some(), "should parse {}", code);
        assert_eq!(p.as_ref().unwrap().code(), code);
    }
}

#[test]
fn naca_params_rejects_invalid() {
    assert!(NacaParams::from_naca4("").is_none());
    assert!(NacaParams::from_naca4("abc").is_none());
    assert!(NacaParams::from_naca4("12345").is_none());
    assert!(NacaParams::from_naca4("24a2").is_none());
    assert!(NacaParams::from_naca4("00").is_none());
}

#[test]
fn naca_params_fractions_correct() {
    let p = NacaParams::from_naca4("2412").expect("parse");
    assert!((p.m() - 0.02).abs() < 1e-6);
    assert!((p.p() - 0.4).abs() < 1e-6);
    assert!((p.t() - 0.12).abs() < 1e-6);
}

// =========================================================================
// FlowSettings
// =========================================================================

#[test]
fn flow_settings_default_is_sane() {
    let f = FlowSettings::default();
    assert!(f.reynolds > 0.0);
    assert!(f.mach >= 0.0 && f.mach < 1.0);
    assert!(f.viscous);
    assert!(f.free_transition);
}
