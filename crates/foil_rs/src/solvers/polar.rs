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
    compute_polar_sweep_with_system(
        params,
        flow,
        alpha_min_deg,
        alpha_max_deg,
        alpha_step_deg,
        None,
    )
}

pub fn compute_polar_sweep_parallel(
    params: &NacaParams,
    flow: &FlowSettings,
    alpha_min_deg: f32,
    alpha_max_deg: f32,
    alpha_step_deg: f32,
) -> Vec<PolarRow> {
    compute_polar_sweep_parallel_with_threads(
        params,
        flow,
        alpha_min_deg,
        alpha_max_deg,
        alpha_step_deg,
        None,
    )
}

pub fn compute_polar_sweep_parallel_with_threads(
    params: &NacaParams,
    flow: &FlowSettings,
    alpha_min_deg: f32,
    alpha_max_deg: f32,
    alpha_step_deg: f32,
    threads: Option<usize>,
) -> Vec<PolarRow> {
    compute_polar_sweep_parallel_with_system(
        params,
        flow,
        alpha_min_deg,
        alpha_max_deg,
        alpha_step_deg,
        None,
        threads,
    )
}

pub fn compute_multi_polar_sweeps(
    params: &NacaParams,
    flows: &[FlowSettings],
    alpha_min_deg: f32,
    alpha_max_deg: f32,
    alpha_step_deg: f32,
    threads: Option<usize>,
) -> Vec<(FlowSettings, Vec<PolarRow>)> {
    let mut out = Vec::with_capacity(flows.len());
    for flow in flows {
        let rows = if threads.unwrap_or(1) <= 1 {
            compute_polar_sweep(
                params,
                flow,
                alpha_min_deg,
                alpha_max_deg,
                alpha_step_deg,
            )
        } else {
            compute_polar_sweep_parallel_with_threads(
                params,
                flow,
                alpha_min_deg,
                alpha_max_deg,
                alpha_step_deg,
                threads,
            )
        };
        out.push((flow.clone(), rows));
    }
    out
}

pub(crate) fn compute_polar_sweep_with_system(
    params: &NacaParams,
    flow: &FlowSettings,
    alpha_min_deg: f32,
    alpha_max_deg: f32,
    alpha_step_deg: f32,
    system: Option<&PanelLuSystem>,
) -> Vec<PolarRow> {
    let (alphas, capacity) =
        alpha_samples(alpha_min_deg, alpha_max_deg, alpha_step_deg);
    if alphas.is_empty() {
        return Vec::new();
    }

    let owned_system;
    let system = match system {
        Some(sys) => Some(sys),
        None => {
            owned_system = PanelLuSystem::new(params);
            owned_system.as_ref()
        }
    };

    let beta = (1.0 - flow.mach * flow.mach).clamp(0.05, 1.0).sqrt();
    let bl_inputs = BoundaryLayerInputs::new(
        flow.reynolds,
        flow.mach,
        flow.viscous,
        flow.free_transition,
        DEFAULT_FORCED_TRIP_X,
    );
    let bl_inputs = &bl_inputs;

    let mut rows = Vec::with_capacity(capacity);
    for &a in &alphas {
        let sol = system
            .as_ref()
            .map(|sys| sys.panel_solution(params, a))
            .unwrap_or_else(|| compute_panel_solution(params, a));
        rows.push(polar_row(&sol, a, beta, bl_inputs));
    }
    rows
}

fn polar_row(
    sol: &PanelSolution,
    alpha_deg: f32,
    beta: f32,
    bl_inputs: &BoundaryLayerInputs,
) -> PolarRow {
    let cl = sol.cl().unwrap_or(f32::NAN) / beta;
    let cm_c4 = sol.cm_c4().unwrap_or(f32::NAN);
    let boundary_layer = estimate_boundary_layer(sol, bl_inputs);

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

pub fn compute_polar_sweep_parallel_with_system(
    params: &NacaParams,
    flow: &FlowSettings,
    alpha_min_deg: f32,
    alpha_max_deg: f32,
    alpha_step_deg: f32,
    system: Option<&PanelLuSystem>,
    threads: Option<usize>,
) -> Vec<PolarRow> {
    let (alphas, capacity) =
        alpha_samples(alpha_min_deg, alpha_max_deg, alpha_step_deg);
    if alphas.is_empty() {
        return Vec::new();
    }

    let owned_system;
    let system = match system {
        Some(sys) => Some(sys),
        None => {
            owned_system = PanelLuSystem::new(params);
            owned_system.as_ref()
        }
    };

    // If we couldn't build a cached system, fall back to the single-threaded
    // path (which already has a non-system fallback per alpha).
    let Some(system) = system else {
        return compute_polar_sweep_with_system(
            params,
            flow,
            alpha_min_deg,
            alpha_max_deg,
            alpha_step_deg,
            None,
        );
    };

    let beta = (1.0 - flow.mach * flow.mach).clamp(0.05, 1.0).sqrt();
    let bl_inputs = BoundaryLayerInputs::new(
        flow.reynolds,
        flow.mach,
        flow.viscous,
        flow.free_transition,
        DEFAULT_FORCED_TRIP_X,
    );
    let bl_inputs = &bl_inputs;

    let available = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);
    let mut thread_count = threads.unwrap_or(available).max(1);
    thread_count = thread_count.min(alphas.len());

    if thread_count <= 1 {
        let mut rows = Vec::with_capacity(capacity);
        for &a in &alphas {
            let sol = system.panel_solution(params, a);
            rows.push(polar_row(&sol, a, beta, bl_inputs));
        }
        return rows;
    }

    let chunk_size = (alphas.len() + thread_count - 1) / thread_count;
    let mut chunks: Vec<Vec<PolarRow>> =
        Vec::with_capacity(thread_count);

    std::thread::scope(|scope| {
        let mut handles = Vec::with_capacity(thread_count);
        for chunk_idx in 0..thread_count {
            let start = chunk_idx * chunk_size;
            if start >= alphas.len() {
                break;
            }
            let end = ((chunk_idx + 1) * chunk_size).min(alphas.len());
            let alpha_slice = &alphas[start..end];

            handles.push(scope.spawn(move || {
                let mut rows = Vec::with_capacity(alpha_slice.len());
                for &a in alpha_slice {
                    let sol = system.panel_solution(params, a);
                    rows.push(polar_row(&sol, a, beta, bl_inputs));
                }
                rows
            }));
        }

        for h in handles {
            chunks.push(h.join().unwrap_or_default());
        }
    });

    let total: usize = chunks.iter().map(|c| c.len()).sum();
    let mut rows = Vec::with_capacity(total);
    for mut c in chunks {
        rows.append(&mut c);
    }
    rows
}

fn alpha_samples(
    alpha_min_deg: f32,
    alpha_max_deg: f32,
    alpha_step_deg: f32,
) -> (Vec<f32>, usize) {
    let step = alpha_step_deg.abs().max(1e-3);
    let (a0, a1) = if alpha_min_deg <= alpha_max_deg {
        (alpha_min_deg, alpha_max_deg)
    } else {
        (alpha_max_deg, alpha_min_deg)
    };

    let approx_steps = ((a1 - a0) / step).max(0.0);
    let capacity = approx_steps.floor() as usize + 2;
    let mut alphas = Vec::with_capacity(capacity);
    for i in 0..capacity {
        let a = a0 + step * i as f32;
        if a > a1 + 1e-6 {
            break;
        }
        alphas.push(a);
    }
    (alphas, capacity)
}
