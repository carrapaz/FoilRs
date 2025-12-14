use foil_rs::solvers::{
    BoundaryLayerInputs, compute_panel_solution,
    estimate_boundary_layer,
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

    let mut params = NacaParams::from_naca4(&naca)
        .unwrap_or_else(NacaParams::default);
    params.num_points = 160;

    let flow = FlowSettings {
        alpha_deg,
        reynolds: 1_000_000.0,
        mach: 0.10,
        viscous: true,
        free_transition: true,
    };

    let sol = compute_panel_solution(&params, flow.alpha_deg);
    let bl_inputs = BoundaryLayerInputs::new(
        flow.reynolds,
        flow.mach,
        true,
        true,
        0.05,
    );
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

fn fmt_opt(v: Option<f32>) -> String {
    v.map(|x| x.to_string())
        .unwrap_or_else(|| "none".to_string())
}
