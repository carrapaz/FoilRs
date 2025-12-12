use bevy::math::Vec2;

use super::panel::PanelSolution;

const MIN_RE: f32 = 1e3;

#[derive(Clone, Debug)]
pub struct BoundaryLayerResult {
    pub cd_profile: f32,
    pub transition_upper: Option<f32>,
    pub transition_lower: Option<f32>,
    pub separation_upper: Option<f32>,
    pub separation_lower: Option<f32>,
    pub probable_stall: bool,
}

#[derive(Clone, Debug)]
pub struct BoundaryLayerInputs {
    pub reynolds: f32,
    pub mach: f32,
    pub viscous: bool,
    pub free_transition: bool,
    pub forced_transition_x: f32,
    pub beta: f32,
}

impl BoundaryLayerInputs {
    pub fn new(
        reynolds: f32,
        mach: f32,
        viscous: bool,
        free_transition: bool,
        forced_transition_x: f32,
    ) -> Self {
        let beta = (1.0 - mach * mach).clamp(0.05, 1.0).sqrt();
        Self {
            reynolds: reynolds.max(MIN_RE),
            mach,
            viscous,
            free_transition,
            forced_transition_x: forced_transition_x.clamp(0.001, 0.99),
            beta,
        }
    }
}

pub fn estimate_boundary_layer(
    solution: &PanelSolution,
    inputs: &BoundaryLayerInputs,
) -> Option<BoundaryLayerResult> {
    if !inputs.viscous {
        return None;
    }
    if solution.upper_coords.len() < 2
        || solution.lower_coords.len() < 2
    {
        return None;
    }

    let upper = integrate_surface(
        &solution.upper_coords,
        &solution.cp_upper,
        inputs,
    );
    let lower = integrate_surface(
        &solution.lower_coords,
        &solution.cp_lower,
        inputs,
    );

    Some(BoundaryLayerResult {
        cd_profile: (upper.cd + lower.cd).max(0.0),
        transition_upper: upper.transition_x,
        transition_lower: lower.transition_x,
        separation_upper: upper.separation_x,
        separation_lower: lower.separation_x,
        probable_stall: upper.probable_stall || lower.probable_stall,
    })
}

struct SurfaceResult {
    cd: f32,
    transition_x: Option<f32>,
    separation_x: Option<f32>,
    probable_stall: bool,
}

fn integrate_surface(
    coords: &[Vec2],
    cp: &[f32],
    inputs: &BoundaryLayerInputs,
) -> SurfaceResult {
    if coords.len() != cp.len() || coords.len() < 2 {
        return SurfaceResult {
            cd: 0.0,
            transition_x: None,
            separation_x: None,
            probable_stall: false,
        };
    }

    let samples = build_samples(coords, cp, inputs);
    let nu = 1.0 / inputs.reynolds;
    let mut u5_integral = 0.0;
    let mut transition_s: Option<f32> = None;
    let mut transition_x: Option<f32> = None;
    let mut separation_s: Option<f32> = None;
    let mut separation_x: Option<f32> = None;
    let mut cd = 0.0;

    if !inputs.free_transition {
        transition_s = Some(inputs.forced_transition_x);
        transition_x = Some(inputs.forced_transition_x);
    }

    for i in 1..samples.len() {
        let prev = &samples[i - 1];
        let curr = &samples[i];
        let ds = (curr.s - prev.s).abs().max(1e-5);
        let ue_prev = prev.ue.max(1e-4);
        let ue_curr = curr.ue.max(1e-4);

        let u5_avg = 0.5 * (ue_prev.powi(5) + ue_curr.powi(5));
        u5_integral += u5_avg * ds;

        let ue_curr6 = ue_curr.powi(6).max(1e-5);
        let theta_sq = 0.45 * nu * u5_integral / ue_curr6;
        let theta = theta_sq.sqrt();
        let ue_prime = (ue_curr - ue_prev) / ds;
        let lambda = theta_sq * ue_prime / nu;

        if separation_s.is_none() && lambda < -0.09 && curr.s > 0.02 {
            separation_s = Some(curr.s);
            separation_x = Some(curr.x);
        }

        if transition_s.is_none() {
            let re_theta = theta * inputs.reynolds;
            let re_x = inputs.reynolds * curr.s.max(1e-5);
            let crit = 1.174 * (1.0 + 22400.0 / re_x.max(1e3));
            if re_theta >= crit {
                transition_s = Some(curr.s);
                transition_x = Some(curr.x);
            }
        }

        let s_mid = 0.5 * (prev.s + curr.s);
        let re_x_mid = inputs.reynolds * s_mid.max(1e-5);
        let laminar =
            transition_s.map(|tr| s_mid <= tr).unwrap_or(true);
        let separated =
            separation_s.map(|sep| s_mid >= sep).unwrap_or(false);

        let mut cf = if laminar {
            laminar_cf(re_x_mid)
        } else {
            turbulent_cf(re_x_mid)
        };

        if separated {
            cf = 0.0;
        }

        cd += cf * ds;
    }

    let probable_stall =
        separation_x.map(|x| x > 0.2 && x < 0.95).unwrap_or(false);

    SurfaceResult {
        cd,
        transition_x,
        separation_x,
        probable_stall,
    }
}

struct SurfaceSample {
    pub s: f32,
    pub x: f32,
    pub ue: f32,
}

fn build_samples(
    coords: &[Vec2],
    cp: &[f32],
    inputs: &BoundaryLayerInputs,
) -> Vec<SurfaceSample> {
    let mut samples = Vec::with_capacity(coords.len());
    let mut s = 0.0;
    samples.push(SurfaceSample {
        s,
        x: coords[0].x,
        ue: speed_from_cp(cp[0], inputs),
    });
    for i in 1..coords.len() {
        s += coords[i].distance(coords[i - 1]);
        samples.push(SurfaceSample {
            s,
            x: coords[i].x,
            ue: speed_from_cp(cp[i], inputs),
        });
    }
    samples
}

fn speed_from_cp(cp: f32, inputs: &BoundaryLayerInputs) -> f32 {
    let cp_corr = (cp / inputs.beta).clamp(-5.0, 5.0);
    (1.0 - cp_corr).max(1e-4).sqrt()
}

fn laminar_cf(re_x: f32) -> f32 {
    let re = re_x.max(MIN_RE);
    0.664 / re.sqrt()
}

fn turbulent_cf(re_x: f32) -> f32 {
    let re = re_x.max(5.0e4);
    let log_term = re.log10().max(1.0);
    0.455 / log_term.powf(2.58)
}
