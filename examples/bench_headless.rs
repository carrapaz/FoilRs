use std::hint::black_box;
use std::time::{Duration, Instant};

use foil_rs::solvers::compute_polar_sweep;
use foil_rs::solvers::compute_panel_solution;
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
    let panel_iters: usize = args
        .next()
        .as_deref()
        .unwrap_or("2000")
        .parse()
        .unwrap_or(2000);
    let polar_iters: usize = args
        .next()
        .as_deref()
        .unwrap_or("30")
        .parse()
        .unwrap_or(30);

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

    let params = parse_naca_4(&naca).unwrap_or_else(|| NacaParams {
        m_digit: 2.0,
        p_digit: 4.0,
        t_digits: 12.0,
        num_points: 160,
    });

    println!("naca={}", params.code());
    println!("alpha_deg={}", alpha_deg);
    println!(
        "panel_iters={} polar_iters={}",
        panel_iters, polar_iters
    );
    println!(
        "polar_sweep=[{}, {}, {}]",
        alpha_min_deg, alpha_max_deg, alpha_step_deg
    );

    if panel_iters > 0 {
        let (elapsed, last) = time_panel_solution(&params, alpha_deg, panel_iters);
        print_stats("panel_solution", panel_iters, elapsed);
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
        let (elapsed, last_len) = time_polar_sweep(
            &params,
            &flow,
            alpha_min_deg,
            alpha_max_deg,
            alpha_step_deg,
            polar_iters,
        );
        print_stats("polar_sweep", polar_iters, elapsed);
        println!("polar_rows={}", last_len);
    }
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

fn time_polar_sweep(
    params: &NacaParams,
    flow: &FlowSettings,
    alpha_min_deg: f32,
    alpha_max_deg: f32,
    alpha_step_deg: f32,
    iters: usize,
) -> (Duration, usize) {
    let mut last_len = 0usize;
    let start = Instant::now();
    for _ in 0..iters {
        let rows = black_box(compute_polar_sweep(
            params,
            flow,
            alpha_min_deg,
            alpha_max_deg,
            alpha_step_deg,
        ));
        last_len = rows.len();
        black_box(&rows);
    }
    (start.elapsed(), last_len)
}

fn print_stats(label: &str, iters: usize, elapsed: Duration) {
    let total_ms = elapsed.as_secs_f64() * 1e3;
    let per_iter_us = elapsed.as_secs_f64() * 1e6 / iters as f64;
    println!(
        "{}: total_ms={:.3} per_iter_us={:.3}",
        label, total_ms, per_iter_us
    );
}

fn parse_naca_4(code: &str) -> Option<NacaParams> {
    let code = code.trim();
    if code.len() != 4 || !code.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let digits: Vec<u32> =
        code.chars().map(|c| c.to_digit(10).unwrap()).collect();
    Some(NacaParams {
        m_digit: digits[0] as f32,
        p_digit: digits[1] as f32,
        t_digits: (digits[2] * 10 + digits[3]) as f32,
        num_points: 160,
    })
}

