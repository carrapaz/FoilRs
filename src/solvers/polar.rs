use crate::state::{FlowSettings, NacaParams};

use super::panel::PanelLuSystem;
use super::{
    BoundaryLayerInputs, PanelSolution, compute_panel_solution,
    estimate_boundary_layer,
};

const DEFAULT_ALPHA_MIN_DEG: f32 = -10.0;
const DEFAULT_ALPHA_MAX_DEG: f32 = 15.0;
const DEFAULT_ALPHA_STEP_DEG: f32 = 0.5;
const DEFAULT_FORCED_TRIP_X: f32 = 0.05;

#[derive(Clone, Debug)]
pub struct PolarRow {
    pub alpha_deg: f32,
    pub cl: f32,
    pub cm_c4: f32,
    pub cd_profile: Option<f32>,
    pub probable_stall: bool,
}

pub fn default_polar_sweep() -> (f32, f32, f32) {
    (
        DEFAULT_ALPHA_MIN_DEG,
        DEFAULT_ALPHA_MAX_DEG,
        DEFAULT_ALPHA_STEP_DEG,
    )
}

pub fn compute_polar_sweep(
    params: &NacaParams,
    flow: &FlowSettings,
    alpha_min_deg: f32,
    alpha_max_deg: f32,
    alpha_step_deg: f32,
) -> Vec<PolarRow> {
    let step = alpha_step_deg.abs().max(1e-3);
    let (a0, a1) = if alpha_min_deg <= alpha_max_deg {
        (alpha_min_deg, alpha_max_deg)
    } else {
        (alpha_max_deg, alpha_min_deg)
    };

    let approx_steps = ((a1 - a0) / step).max(0.0);
    let capacity = approx_steps.floor() as usize + 2;
    let mut rows = Vec::with_capacity(capacity);

    let system = PanelLuSystem::new(params);

    for i in 0..capacity {
        let a = a0 + step * i as f32;
        if a > a1 + 1e-6 {
            break;
        }
        let sol = system
            .as_ref()
            .map(|sys| sys.panel_solution(params, a))
            .unwrap_or_else(|| compute_panel_solution(params, a));
        rows.push(polar_row(&sol, flow, a));
    }
    rows
}

fn polar_row(
    sol: &PanelSolution,
    flow: &FlowSettings,
    alpha_deg: f32,
) -> PolarRow {
    let beta = (1.0 - flow.mach * flow.mach).clamp(0.05, 1.0).sqrt();
    let cl = sol.cl().unwrap_or(f32::NAN) / beta;
    let cm_c4 = sol.cm_c4().unwrap_or(f32::NAN);

    let bl_inputs = BoundaryLayerInputs::new(
        flow.reynolds,
        flow.mach,
        flow.viscous,
        flow.free_transition,
        DEFAULT_FORCED_TRIP_X,
    );
    let boundary_layer = estimate_boundary_layer(sol, &bl_inputs);

    PolarRow {
        alpha_deg,
        cl,
        cm_c4,
        cd_profile: boundary_layer.as_ref().map(|b| b.cd_profile),
        probable_stall: boundary_layer
            .as_ref()
            .map(|b| b.probable_stall)
            .unwrap_or(false),
    }
}
