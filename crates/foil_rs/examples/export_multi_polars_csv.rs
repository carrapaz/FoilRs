use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use foil_rs::solvers::compute_multi_polar_sweeps;
use foil_rs::state::{FlowSettings, NacaParams};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if matches!(args.get(0).map(|s| s.as_str()), Some("-h" | "--help"))
    {
        print_help_and_exit();
    }

    let mut it = args.into_iter();
    let naca = it.next().unwrap_or_else(|| "2412".to_string());

    let reynolds_millions =
        it.next().unwrap_or_else(|| "1.0".to_string());
    let mach_list = it.next().unwrap_or_else(|| "0.10".to_string());

    let viscous = parse_bool(it.next().as_deref(), true);
    let free_transition = parse_bool(it.next().as_deref(), true);

    let alpha_min_deg: f32 =
        it.next().and_then(|s| s.parse().ok()).unwrap_or(-10.0);
    let alpha_max_deg: f32 =
        it.next().and_then(|s| s.parse().ok()).unwrap_or(15.0);
    let alpha_step_deg: f32 =
        it.next().and_then(|s| s.parse().ok()).unwrap_or(0.5);

    let threads: usize =
        it.next().and_then(|s| s.parse().ok()).unwrap_or(1);

    let out_path = it
        .next()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("exports/multi_polars.csv"));

    let reynolds_values = parse_csv_f32(&reynolds_millions)
        .into_iter()
        .map(|re_m| re_m.max(0.001) * 1_000_000.0)
        .collect::<Vec<f32>>();
    let mach_values = parse_csv_f32(&mach_list);

    if reynolds_values.is_empty() || mach_values.is_empty() {
        eprintln!(
            "expected non-empty RE/MACH lists; got RE='{}' MACH='{}'",
            reynolds_millions, mach_list
        );
        std::process::exit(2);
    }

    let params = NacaParams::from_naca4(&naca)
        .unwrap_or_else(NacaParams::default);

    let mut flows = Vec::new();
    for &re in &reynolds_values {
        for &mach in &mach_values {
            flows.push(FlowSettings {
                alpha_deg: 0.0,
                reynolds: re,
                mach,
                viscous,
                free_transition,
            });
        }
    }

    if let Some(parent) = out_path.parent() {
        if let Err(err) = std::fs::create_dir_all(parent) {
            eprintln!(
                "failed to create output directory {}: {err}",
                parent.display()
            );
            std::process::exit(1);
        }
    }

    let sweeps = compute_multi_polar_sweeps(
        &params,
        &flows,
        alpha_min_deg,
        alpha_max_deg,
        alpha_step_deg,
        Some(threads.max(1)),
    );

    if let Err(err) = write_multi_polar_csv(&out_path, &sweeps) {
        eprintln!("failed to write {}: {err}", out_path.display());
        std::process::exit(1);
    }

    let total_rows: usize = sweeps.iter().map(|(_, r)| r.len()).sum();
    println!(
        "saved {} curves / {} rows to {}",
        sweeps.len(),
        total_rows,
        out_path.display()
    );
}

fn print_help_and_exit() -> ! {
    eprintln!(
        "Export multi-polars CSV (headless)\n\
\n\
Computes a full alpha sweep for each (Re, Mach) pair and writes one combined CSV.\n\
\n\
Usage:\n\
  cargo run -p foil_rs --example export_multi_polars_csv --release -- \\\n\
    [NACA] [RE_MILLIONS_LIST] [MACH_LIST] [VISCOUS] [FREE_TRANSITION] \\\n\
    [ALPHA_MIN] [ALPHA_MAX] [ALPHA_STEP] [THREADS] [OUT_PATH]\n\
\n\
Defaults:\n\
  NACA=2412 RE_MILLIONS_LIST=1.0 MACH_LIST=0.10 VISCOUS=1 FREE_TRANSITION=1\n\
  ALPHA_MIN=-10 ALPHA_MAX=15 ALPHA_STEP=0.5 THREADS=1 OUT_PATH=exports/multi_polars.csv\n\
\n\
Lists are comma-separated, e.g. RE_MILLIONS_LIST=\"0.5,1.0,2.0\" MACH_LIST=\"0.0,0.1\".\n"
    );
    std::process::exit(0);
}

fn parse_bool(arg: Option<&str>, default: bool) -> bool {
    let Some(arg) = arg else { return default };
    match arg.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "t" | "yes" | "y" | "on" => true,
        "0" | "false" | "f" | "no" | "n" | "off" => false,
        _ => default,
    }
}

fn parse_csv_f32(text: &str) -> Vec<f32> {
    text.split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .filter_map(|s| s.parse::<f32>().ok())
        .collect()
}

fn write_multi_polar_csv(
    path: &Path,
    sweeps: &[(FlowSettings, Vec<foil_rs::solvers::PolarRow>)],
) -> std::io::Result<()> {
    let file = File::create(path)?;
    let mut out = BufWriter::new(file);

    out.write_all(b"curve_id,alpha_deg,cl,cm_c4,cd_profile,mach,reynolds,viscous,free_transition,probable_stall\n")?;
    for (curve_id, (flow, rows)) in sweeps.iter().enumerate() {
        for r in rows {
            let cd = r.cd_profile.unwrap_or(f32::NAN);
            writeln!(
                out,
                "{},{:.3},{:.6},{:.6},{:.6},{:.4},{:.0},{},{},{}",
                curve_id,
                r.alpha_deg,
                r.cl,
                r.cm_c4,
                cd,
                flow.mach,
                flow.reynolds,
                flow.viscous as u8,
                flow.free_transition as u8,
                r.probable_stall as u8,
            )?;
        }
    }
    out.flush()
}
