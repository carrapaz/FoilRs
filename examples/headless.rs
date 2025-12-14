use foil_rs::solvers::{
    BoundaryLayerInputs, compute_panel_solution, estimate_boundary_layer,
};
use foil_rs::state::{FlowSettings, NacaParams};

fn main() {
    let mut args = std::env::args().skip(1);
    let naca = args.next().unwrap_or_else(|| "2412".to_string());
    let alpha_deg: f32 = args
        .next()
        .as_deref()
        .unwrap_or("4.0")
        .parse()
        .unwrap_or(4.0);

    let params = parse_naca_4(&naca).unwrap_or_else(|| NacaParams {
        m_digit: 2.0,
        p_digit: 4.0,
        t_digits: 12.0,
        num_points: 160,
    });

    let flow = FlowSettings {
        alpha_deg,
        reynolds: 1_000_000.0,
        mach: 0.10,
        viscous: true,
        free_transition: true,
    };

    let sol = compute_panel_solution(&params, flow.alpha_deg);
    let bl_inputs =
        BoundaryLayerInputs::new(flow.reynolds, flow.mach, true, true, 0.05);
    let bl = estimate_boundary_layer(&sol, &bl_inputs);

    println!("naca={}", params.code());
    println!("alpha_deg={}", alpha_deg);
    println!("cl={}", sol.cl().unwrap_or(f32::NAN));
    println!("cm_c4={}", sol.cm_c4().unwrap_or(f32::NAN));
    println!(
        "cd_profile={}",
        bl.as_ref().map(|b| b.cd_profile).unwrap_or(f32::NAN)
    );
    println!(
        "transition_upper={}",
        fmt_opt(bl.as_ref().and_then(|b| b.transition_upper))
    );
    println!(
        "transition_lower={}",
        fmt_opt(bl.as_ref().and_then(|b| b.transition_lower))
    );
}

fn parse_naca_4(code: &str) -> Option<NacaParams> {
    let code = code.trim();
    if code.len() != 4 || !code.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let digits: Vec<u32> = code.chars().map(|c| c.to_digit(10).unwrap()).collect();
    Some(NacaParams {
        m_digit: digits[0] as f32,
        p_digit: digits[1] as f32,
        t_digits: (digits[2] * 10 + digits[3]) as f32,
        num_points: 160,
    })
}

fn fmt_opt(v: Option<f32>) -> String {
    v.map(|x| x.to_string())
        .unwrap_or_else(|| "none".to_string())
}
