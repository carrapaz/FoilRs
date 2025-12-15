use std::hint::black_box;
use std::time::{Duration, Instant};

use foil_rs::solvers::compute_panel_solution;
use foil_rs::solvers::compute_polar_sweep;
use foil_rs::solvers::compute_polar_sweep_parallel_with_threads;
use foil_rs::state::{FlowSettings, NacaParams};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    if matches!(args.get(0).map(|s| s.as_str()), Some("-h" | "--help"))
    {
        eprintln!(
            "Headless benchmark\n\
\n\
Usage:\n\
  cargo run --example bench_headless --no-default-features --release -- \\\n\
    [NACA] [ALPHA_DEG] [PANEL_ITERS] [POLAR_ITERS] [ALPHA_MIN] [ALPHA_MAX] [ALPHA_STEP] \\\n\
    [THREADS] [RUNS] [WARMUP_RUNS]\n\
\n\
Defaults:\n\
  NACA=2412 ALPHA_DEG=4.0 PANEL_ITERS=2000 POLAR_ITERS=30\n\
  ALPHA_MIN=-10 ALPHA_MAX=15 ALPHA_STEP=0.5 THREADS=1 RUNS=5 WARMUP_RUNS=1\n"
        );
        return;
    }

    let mut args = args.into_iter();

    let naca = args.next().unwrap_or_else(|| "2412".to_string());
    let alpha_deg: f32 = args
        .next()
        .as_deref()
        .unwrap_or("4.0")
        .parse()
        .unwrap_or(4.0);
    let panel_iters: usize = args
        .next()
        .as_deref()
        .unwrap_or("2000")
        .parse()
        .unwrap_or(2000);
    let polar_iters: usize =
        args.next().as_deref().unwrap_or("30").parse().unwrap_or(30);

    let alpha_min_deg: f32 = args
        .next()
        .as_deref()
        .unwrap_or("-10.0")
        .parse()
        .unwrap_or(-10.0);
    let alpha_max_deg: f32 = args
        .next()
        .as_deref()
        .unwrap_or("15.0")
        .parse()
        .unwrap_or(15.0);
    let alpha_step_deg: f32 = args
        .next()
        .as_deref()
        .unwrap_or("0.5")
        .parse()
        .unwrap_or(0.5);

    let threads: usize =
        args.next().as_deref().unwrap_or("1").parse().unwrap_or(1);

    let runs: usize =
        args.next().as_deref().unwrap_or("5").parse().unwrap_or(5);
    let warmup_runs: usize =
        args.next().as_deref().unwrap_or("1").parse().unwrap_or(1);

    let mut params = NacaParams::from_naca4(&naca)
        .unwrap_or_else(NacaParams::default);
    params.num_points = 160;

    println!("naca={}", params.code());
    println!("alpha_deg={}", alpha_deg);
    println!("panel_iters={} polar_iters={}", panel_iters, polar_iters);
    println!(
        "polar_sweep=[{}, {}, {}]",
        alpha_min_deg, alpha_max_deg, alpha_step_deg
    );
    println!("threads={}", threads);
    println!("runs={} warmup_runs={}", runs, warmup_runs);

    if panel_iters > 0 {
        let (summary, last) = bench_panel_solution(
            &params,
            alpha_deg,
            panel_iters,
            runs,
            warmup_runs,
        );
        print_summary("panel_solution", panel_iters, &summary);
        let _ = black_box(last.cl());
    }

    if polar_iters > 0 {
        let flow = FlowSettings {
            alpha_deg,
            reynolds: 1_000_000.0,
            mach: 0.10,
            viscous: true,
            free_transition: true,
        };
        let (summary, last_len) = bench_polar_sweep(
            &params,
            &flow,
            alpha_min_deg,
            alpha_max_deg,
            alpha_step_deg,
            polar_iters,
            threads,
            runs,
            warmup_runs,
        );
        print_summary("polar_sweep", polar_iters, &summary);
        println!("polar_rows={}", last_len);
    }
}

#[derive(Clone, Debug)]
struct Summary {
    total_ms_mean: f64,
    per_iter_us_mean: f64,
    per_iter_us_stddev: f64,
    per_iter_us_min: f64,
    per_iter_us_max: f64,
    runs: usize,
}

fn bench_panel_solution(
    params: &NacaParams,
    alpha_deg: f32,
    iters: usize,
    runs: usize,
    warmup_runs: usize,
) -> (Summary, foil_rs::solvers::PanelSolution) {
    let mut last = compute_panel_solution(params, alpha_deg);
    for _ in 0..warmup_runs {
        let _ =
            black_box(time_panel_solution(params, alpha_deg, iters));
    }

    let mut per_iter_us = Vec::with_capacity(runs.max(1));
    let mut total_ms = Vec::with_capacity(runs.max(1));
    for _ in 0..runs.max(1) {
        let (elapsed, sol) =
            time_panel_solution(params, alpha_deg, iters);
        last = sol;
        total_ms.push(elapsed.as_secs_f64() * 1e3);
        per_iter_us.push(elapsed.as_secs_f64() * 1e6 / iters as f64);
    }

    (summarize(&total_ms, &per_iter_us, runs.max(1)), last)
}

fn time_panel_solution(
    params: &NacaParams,
    alpha_deg: f32,
    iters: usize,
) -> (Duration, foil_rs::solvers::PanelSolution) {
    let mut last = compute_panel_solution(params, alpha_deg);
    let start = Instant::now();
    for _ in 0..iters {
        last = black_box(compute_panel_solution(params, alpha_deg));
    }
    (start.elapsed(), last)
}

fn bench_polar_sweep(
    params: &NacaParams,
    flow: &FlowSettings,
    alpha_min_deg: f32,
    alpha_max_deg: f32,
    alpha_step_deg: f32,
    iters: usize,
    threads: usize,
    runs: usize,
    warmup_runs: usize,
) -> (Summary, usize) {
    let mut last_len = 0usize;
    for _ in 0..warmup_runs {
        let _ = black_box(time_polar_sweep(
            params,
            flow,
            alpha_min_deg,
            alpha_max_deg,
            alpha_step_deg,
            iters,
            threads,
        ));
    }

    let mut per_iter_us = Vec::with_capacity(runs.max(1));
    let mut total_ms = Vec::with_capacity(runs.max(1));

    for _ in 0..runs.max(1) {
        let (elapsed, len) = time_polar_sweep(
            params,
            flow,
            alpha_min_deg,
            alpha_max_deg,
            alpha_step_deg,
            iters,
            threads,
        );
        last_len = len;
        total_ms.push(elapsed.as_secs_f64() * 1e3);
        per_iter_us.push(elapsed.as_secs_f64() * 1e6 / iters as f64);
    }

    (summarize(&total_ms, &per_iter_us, runs.max(1)), last_len)
}

fn time_polar_sweep(
    params: &NacaParams,
    flow: &FlowSettings,
    alpha_min_deg: f32,
    alpha_max_deg: f32,
    alpha_step_deg: f32,
    iters: usize,
    threads: usize,
) -> (Duration, usize) {
    let mut last_len = 0usize;
    let start = Instant::now();
    for _ in 0..iters {
        let rows = if threads <= 1 {
            black_box(compute_polar_sweep(
                params,
                flow,
                alpha_min_deg,
                alpha_max_deg,
                alpha_step_deg,
            ))
        } else {
            black_box(compute_polar_sweep_parallel_with_threads(
                params,
                flow,
                alpha_min_deg,
                alpha_max_deg,
                alpha_step_deg,
                Some(threads),
            ))
        };
        last_len = rows.len();
        black_box(&rows);
    }
    (start.elapsed(), last_len)
}

fn summarize(
    total_ms: &[f64],
    per_iter_us: &[f64],
    runs: usize,
) -> Summary {
    let total_ms_mean = mean(total_ms);
    let per_iter_us_mean = mean(per_iter_us);
    let per_iter_us_stddev = stddev(per_iter_us, per_iter_us_mean);
    let per_iter_us_min =
        per_iter_us.iter().copied().fold(f64::INFINITY, f64::min);
    let per_iter_us_max =
        per_iter_us.iter().copied().fold(0.0, f64::max);

    Summary {
        total_ms_mean,
        per_iter_us_mean,
        per_iter_us_stddev,
        per_iter_us_min,
        per_iter_us_max,
        runs,
    }
}

fn mean(values: &[f64]) -> f64 {
    if values.is_empty() {
        return f64::NAN;
    }
    values.iter().sum::<f64>() / values.len() as f64
}

fn stddev(values: &[f64], mean: f64) -> f64 {
    if values.len() < 2 {
        return 0.0;
    }
    let var = values
        .iter()
        .map(|v| {
            let d = v - mean;
            d * d
        })
        .sum::<f64>()
        / (values.len() - 1) as f64;
    var.sqrt()
}

fn print_summary(label: &str, iters: usize, s: &Summary) {
    println!(
        "{}: iters={} runs={} mean_total_ms={:.3} mean_per_iter_us={:.3} std_per_iter_us={:.3} min_per_iter_us={:.3} max_per_iter_us={:.3}",
        label,
        iters,
        s.runs,
        s.total_ms_mean,
        s.per_iter_us_mean,
        s.per_iter_us_stddev,
        s.per_iter_us_min,
        s.per_iter_us_max
    );
}
