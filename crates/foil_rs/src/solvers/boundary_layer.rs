use crate::math::Vec2;

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

/// Per-station BL state — used by the V-I coupling iteration.
#[derive(Clone, Debug, Default)]
pub struct BoundaryLayerStation {
    /// Arc-length position along surface.
    pub s: f32,
    /// x-coordinate on the airfoil.
    pub x: f32,
    /// Momentum thickness θ.
    pub theta: f32,
    /// Shape factor H = δ*/θ.
    pub h: f32,
    /// Displacement thickness δ* = H × θ.
    pub delta_star: f32,
    /// Edge velocity (normalised to freestream).
    pub ue: f32,
}

/// Full BL solution for one surface — includes the per-station distributions
/// needed for V-I coupling.
#[derive(Clone, Debug)]
pub struct SurfaceBLResult {
    pub cd_squire_young: f32,
    pub transition_x: Option<f32>,
    pub separation_x: Option<f32>,
    pub probable_stall: bool,
    /// Per-station BL state.  Stations correspond to the input Cp/coord
    /// arrays (one per panel midpoint or sample point).
    pub stations: Vec<BoundaryLayerStation>,
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

/// One-shot BL estimate from a PanelSolution (backward-compatible API).
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

    let cd = upper.cd_squire_young + lower.cd_squire_young;

    Some(BoundaryLayerResult {
        cd_profile: cd.max(0.0),
        transition_upper: upper.transition_x,
        transition_lower: lower.transition_x,
        separation_upper: upper.separation_x,
        separation_lower: lower.separation_x,
        probable_stall: upper.probable_stall || lower.probable_stall,
    })
}

/// Integrate one surface from Cp distribution.
pub fn integrate_surface(
    coords: &[Vec2],
    cp: &[f32],
    inputs: &BoundaryLayerInputs,
) -> SurfaceBLResult {
    let ue: Vec<f32> = cp
        .iter()
        .map(|&c| speed_from_cp(c, inputs).max(1e-4))
        .collect();
    integrate_surface_from_ue(coords, &ue, inputs)
}

/// Integrate one surface from edge velocity distribution directly.
///
/// This avoids the Cp→ue conversion and is needed for the linear
/// panel method where V_t at panel midpoints is the natural output.
pub fn integrate_surface_from_ue(
    coords: &[Vec2],
    ue: &[f32],
    inputs: &BoundaryLayerInputs,
) -> SurfaceBLResult {
    if coords.len() != ue.len() || coords.len() < 2 {
        return SurfaceBLResult {
            cd_squire_young: 0.0,
            transition_x: None,
            separation_x: None,
            probable_stall: false,
            stations: Vec::new(),
        };
    }

    let nu = 1.0 / inputs.reynolds;
    let mut u5_integral = 0.0;
    let mut transition_s: Option<f32> = None;
    let mut transition_x: Option<f32> = None;
    let mut separation_s: Option<f32> = None;
    let mut separation_x: Option<f32> = None;

    if !inputs.free_transition {
        transition_s = Some(inputs.forced_transition_x);
        transition_x = Some(inputs.forced_transition_x);
    }

    let mut s_prev = 0.0;
    let mut ue_prev = ue[0].abs().max(1e-4);

    let mut theta_te = 0.0_f32;
    let mut ue_te = 1.0_f32;
    let mut h_te = 2.5_f32;

    let mut stations = Vec::with_capacity(coords.len());
    // First station
    stations.push(BoundaryLayerStation {
        s: 0.0,
        x: coords[0].x,
        theta: 0.0,
        h: 2.59,
        delta_star: 0.0,
        ue: ue_prev,
    });

    for i in 1..coords.len() {
        let ds = coords[i].distance(coords[i - 1]).max(1e-5);
        let s_curr = s_prev + ds;
        let x_curr = coords[i].x;
        let ue_curr = ue[i].abs().max(1e-4);

        let u5_avg = 0.5 * (ue_prev.powi(5) + ue_curr.powi(5));
        u5_integral += u5_avg * ds;

        let ue_curr6 = ue_curr.powi(6).max(1e-5);
        let theta_sq = 0.45 * nu * u5_integral / ue_curr6;
        let theta = theta_sq.sqrt();
        let ue_prime = (ue_curr - ue_prev) / ds;
        let lambda = theta_sq * ue_prime / nu;

        if separation_s.is_none() && lambda < -0.09 && s_curr > 0.02 {
            separation_s = Some(s_curr);
            separation_x = Some(x_curr);
        }

        if transition_s.is_none() {
            let re_theta = theta * inputs.reynolds;
            let re_x = inputs.reynolds * s_curr.max(1e-5);
            let crit = 1.174 * (1.0 + 22400.0 / re_x.max(1e3));
            if re_theta >= crit {
                transition_s = Some(s_curr);
                transition_x = Some(x_curr);
            }
        }

        let laminar =
            transition_s.map(|tr| s_curr <= tr).unwrap_or(true);
        let separated =
            separation_s.map(|sep| s_curr >= sep).unwrap_or(false);

        let h_local = if separated {
            4.0
        } else if laminar {
            let l = lambda.clamp(-0.09, 0.25);
            if l < 0.0 {
                2.61 - 3.75 * l - 5.24 * l * l
            } else {
                2.59 + 0.1 * l
            }
        } else {
            let l = lambda.clamp(-0.09, 0.1);
            if l < -0.04 {
                1.6 - 4.0 * (l + 0.04)
            } else {
                1.4
            }
        };

        let delta_star = theta * h_local;

        stations.push(BoundaryLayerStation {
            s: s_curr,
            x: x_curr,
            theta,
            h: h_local,
            delta_star,
            ue: ue_curr,
        });

        theta_te = theta;
        ue_te = ue_curr;
        h_te = h_local;

        s_prev = s_curr;
        ue_prev = ue_curr;
    }

    // Squire-Young: CD = 2θ_TE (u_TE/V∞)^((H+5)/2)
    // Clamp ue_te to prevent blow-up from panel Cp spikes at LE/TE.
    let ue_te_clamped = ue_te.clamp(0.01, 2.0);
    let h_exp = (h_te.clamp(1.2, 4.0) + 5.0) * 0.5;
    let cd_sy = (2.0 * theta_te * ue_te_clamped.powf(h_exp))
        .clamp(0.0, 0.10);

    let probable_stall =
        separation_x.map(|x| x > 0.2 && x < 0.95).unwrap_or(false);

    SurfaceBLResult {
        cd_squire_young: cd_sy,
        transition_x,
        separation_x,
        probable_stall,
        stations,
    }
}

/// Compute transpiration velocity at each station from the δ* distribution.
///
/// The transpiration (blowing) velocity represents the effective outflow
/// caused by the boundary layer displacement thickness:
///
///   V_n = d(δ* × V_e) / ds
///
/// This is added to the panel normal-velocity boundary condition in the
/// V-I coupling iteration.
pub fn transpiration_velocities(
    stations: &[BoundaryLayerStation],
) -> Vec<f32> {
    if stations.len() < 2 {
        return vec![0.0; stations.len()];
    }
    let mut vn = Vec::with_capacity(stations.len());
    // Forward difference at first station
    vn.push(0.0); // no transpiration at stagnation point

    for i in 1..stations.len() {
        let ds = (stations[i].s - stations[i - 1].s).max(1e-6);
        let m_curr = stations[i].delta_star * stations[i].ue;
        let m_prev = stations[i - 1].delta_star * stations[i - 1].ue;
        vn.push((m_curr - m_prev) / ds);
    }
    vn
}

pub fn speed_from_cp(cp: f32, inputs: &BoundaryLayerInputs) -> f32 {
    let cp_corr = (cp / inputs.beta).clamp(-5.0, 5.0);
    (1.0 - cp_corr).max(1e-4).sqrt()
}
