use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use foil_rs::solvers::{
    compute_polar_sweep, compute_polar_sweep_parallel_with_threads,
    default_polar_sweep,
};
use foil_rs::state::{FlowSettings, NacaParams};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if matches!(args.get(0).map(|s| s.as_str()), Some("-h" | "--help"))
    {
        print_help_and_exit();
    }

    let mut it = args.into_iter();

    let naca = it.next().unwrap_or_else(|| "2412".to_string());
    let reynolds: f32 = it
        .next()
        .as_deref()
        .unwrap_or("1000000")
        .parse()
        .unwrap_or(1_000_000.0);
    let mach: f32 = it
        .next()
        .as_deref()
        .unwrap_or("0.10")
        .parse()
        .unwrap_or(0.10);
    let viscous: bool = parse_bool(it.next().as_deref(), true);
    let free_transition: bool = parse_bool(it.next().as_deref(), true);

    let (default_a0, default_a1, default_da) = default_polar_sweep();
    let alpha_min_deg: f32 =
        it.next().and_then(|s| s.parse().ok()).unwrap_or(default_a0);
    let alpha_max_deg: f32 =
        it.next().and_then(|s| s.parse().ok()).unwrap_or(default_a1);
    let alpha_step_deg: f32 =
        it.next().and_then(|s| s.parse().ok()).unwrap_or(default_da);

    let threads: usize =
        it.next().as_deref().unwrap_or("1").parse().unwrap_or(1);

    let out_path = it.next().map(PathBuf::from);

    let mut params = NacaParams::from_naca4(&naca)
        .unwrap_or_else(NacaParams::default);
    params.num_points = 160;

    let flow = FlowSettings {
        alpha_deg: 0.0,
        reynolds,
        mach,
        viscous,
        free_transition,
    };

    let rows = if threads <= 1 {
        compute_polar_sweep(
            &params,
            &flow,
            alpha_min_deg,
            alpha_max_deg,
            alpha_step_deg,
        )
    } else {
        compute_polar_sweep_parallel_with_threads(
            &params,
            &flow,
            alpha_min_deg,
            alpha_max_deg,
            alpha_step_deg,
            Some(threads),
        )
    };

    let path = match out_path {
        Some(p) => p,
        None => default_export_path(&params, &flow),
    };

    if let Some(parent) = path.parent() {
        if let Err(err) = std::fs::create_dir_all(parent) {
            eprintln!(
                "failed to create output directory {}: {err}",
                parent.display()
            );
            std::process::exit(1);
        }
    }

    if let Err(err) = write_polar_csv(&path, &rows, &flow) {
        eprintln!("failed to write {}: {err}", path.display());
        std::process::exit(1);
    }

    println!("saved {} rows to {}", rows.len(), path.display());
}

fn print_help_and_exit() -> ! {
    eprintln!(
        "Export polar CSV (headless)\n\
\n\
Usage:\n\
  cargo run --example export_polar_csv --no-default-features --release -- \\\n\
    [NACA] [REYNOLDS] [MACH] [VISCOUS] [FREE_TRANSITION] \\\n\
    [ALPHA_MIN] [ALPHA_MAX] [ALPHA_STEP] [THREADS] [OUT_PATH]\n\
\n\
Defaults:\n\
  NACA=2412 REYNOLDS=1000000 MACH=0.10 VISCOUS=1 FREE_TRANSITION=1\n\
  ALPHA_MIN=-10 ALPHA_MAX=15 ALPHA_STEP=0.5 THREADS=1\n\
  OUT_PATH=exports/polar_<...>.csv (auto)\n"
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

fn default_export_path(
    params: &NacaParams,
    flow: &FlowSettings,
) -> PathBuf {
    let visc_tag = if flow.viscous { "visc" } else { "invisc" };
    let tr_tag = if flow.free_transition {
        "auto"
    } else {
        "forced"
    };
    let re_m = flow.reynolds / 1_000_000.0;

    let dir = Path::new("exports");
    let base = format!(
        "polar_{}_Re{:.2}e6_M{:.2}_{}_{}.csv",
        params.code(),
        re_m,
        flow.mach,
        visc_tag,
        tr_tag,
    );

    let mut path = dir.join(base);
    if !path.exists() {
        return path;
    }

    for i in 1..1000u32 {
        let name = format!(
            "polar_{}_Re{:.2}e6_M{:.2}_{}_{}_{}.csv",
            params.code(),
            re_m,
            flow.mach,
            visc_tag,
            tr_tag,
            i
        );
        path = dir.join(name);
        if !path.exists() {
            return path;
        }
    }

    dir.join("polar_export.csv")
}

fn write_polar_csv(
    path: &Path,
    rows: &[foil_rs::solvers::PolarRow],
    flow: &FlowSettings,
) -> std::io::Result<()> {
    let file = File::create(path)?;
    let mut out = BufWriter::new(file);

    out.write_all(b"alpha_deg,cl,cm_c4,cd_profile,mach,reynolds,viscous,free_transition,probable_stall\n")?;
    for r in rows {
        let cd = r.cd_profile.unwrap_or(f32::NAN);
        writeln!(
            out,
            "{:.3},{:.6},{:.6},{:.6},{:.4},{:.0},{},{},{}",
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
    out.flush()
}
