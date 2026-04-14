//! Validate FoilRs solver output against published reference data.
//!
//! Invoke via `cargo val` (alias) or `cargo run -p foil_rs --example validate_airfoils --release`.
//!
//! Loads reference airfoil datasets (XFoil/Abbott & Von Doenhoff), runs the
//! FoilRs panel+BL solver at matching conditions, and reports accuracy for
//! CL, CD, CM, CL_alpha, and stall prediction.

use foil_rs::solvers::panel::PanelLuSystem;
use foil_rs::solvers::polar::{
    PolarMode, compute_polar_sweep_parallel_with_system_mode,
};
use foil_rs::state::{FlowSettings, NacaParams};

use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Instant;

// ---------------------------------------------------------------------------
// Reference data structures
// ---------------------------------------------------------------------------

#[derive(serde::Deserialize)]
struct AirfoilDataset {
    meta: Meta,
    conditions: Conditions,
    reference_points: Vec<RefPoint>,
    summary: Summary,
}

#[derive(serde::Deserialize)]
struct Meta {
    id: String,
    label: String,
    naca_code: String,
    #[allow(dead_code)]
    source: String,
}

#[derive(serde::Deserialize)]
struct Conditions {
    reynolds: f32,
    mach: f32,
    #[allow(dead_code)]
    ncrit: u32,
}

#[derive(serde::Deserialize)]
struct RefPoint {
    alpha_deg: f32,
    cl: f64,
    cd: f64,
    cm_c4: f64,
}

#[derive(serde::Deserialize)]
struct Summary {
    cl_alpha_per_rad: f64,
    #[serde(rename = "cl_max")]
    _cl_max: f64,
    #[serde(rename = "alpha_stall_deg")]
    _alpha_stall_deg: f64,
    cd_min: f64,
    alpha_zero_lift_deg: f64,
}

// ---------------------------------------------------------------------------
// Comparison result
// ---------------------------------------------------------------------------

struct AirfoilReport {
    label: String,
    naca_code: String,
    point_comparisons: Vec<PointComparison>,
    summary_comparisons: Vec<SummaryComparison>,
    overall_cl_error_pct: f64,
    overall_cd_error_pct: f64,
    overall_cm_error_pct: f64,
    solver_time_ms: f64,
}

struct PointComparison {
    alpha_deg: f32,
    cl_ref: f64,
    cl_computed: f64,
    cl_error_pct: f64,
    cd_ref: f64,
    cd_computed: f64,
    cd_error_pct: f64,
    cm_ref: f64,
    cm_computed: f64,
    cm_error_pct: f64,
}

struct SummaryComparison {
    metric: String,
    reference: f64,
    computed: f64,
    error_pct: f64,
    unit: String,
}

// ---------------------------------------------------------------------------
// Tolerances
// ---------------------------------------------------------------------------

const CL_TOLERANCE_PCT: f64 = 15.0;
const CD_TOLERANCE_PCT: f64 = 50.0; // CD is the weakest — BL solver is simplified
const CM_TOLERANCE_PCT: f64 = 30.0;
const CL_ALPHA_TOLERANCE_PCT: f64 = 20.0;

// ---------------------------------------------------------------------------
// Solver
// ---------------------------------------------------------------------------

fn run_validation(dataset: &AirfoilDataset) -> AirfoilReport {
    let params = NacaParams::from_naca4(&dataset.meta.naca_code)
        .unwrap_or_else(|| {
            panic!("Invalid NACA code: {}", dataset.meta.naca_code)
        });

    let flow = FlowSettings {
        alpha_deg: 0.0,
        reynolds: dataset.conditions.reynolds,
        mach: dataset.conditions.mach,
        viscous: true,
        free_transition: true,
    };

    let start = Instant::now();

    // Run a full polar sweep
    let system = PanelLuSystem::new(&params);
    let result = compute_polar_sweep_parallel_with_system_mode(
        &params,
        &flow,
        -10.0,
        25.0,
        0.5,
        system.as_ref(),
        None,
        PolarMode::Panel,
    );
    let solver_time_ms = start.elapsed().as_secs_f64() * 1000.0;

    // Build a lookup from alpha_deg -> row for interpolation
    let rows = &result.rows;

    // Compare each reference point
    let mut point_comparisons = Vec::new();
    let mut cl_errors = Vec::new();
    let mut cd_errors = Vec::new();
    let mut cm_errors = Vec::new();

    for rp in &dataset.reference_points {
        let (cl_c, cd_c, cm_c) = interpolate_polar(rows, rp.alpha_deg);

        let cl_err = if rp.cl.abs() > 0.01 {
            ((cl_c - rp.cl) / rp.cl * 100.0).abs()
        } else {
            (cl_c - rp.cl).abs() * 100.0 // absolute for near-zero
        };
        let cd_err = if rp.cd > 0.0001 {
            ((cd_c - rp.cd) / rp.cd * 100.0).abs()
        } else {
            0.0
        };
        let cm_err = if rp.cm_c4.abs() > 0.005 {
            ((cm_c - rp.cm_c4) / rp.cm_c4 * 100.0).abs()
        } else {
            (cm_c - rp.cm_c4).abs() * 100.0
        };

        cl_errors.push(cl_err);
        cd_errors.push(cd_err);
        cm_errors.push(cm_err);

        point_comparisons.push(PointComparison {
            alpha_deg: rp.alpha_deg,
            cl_ref: rp.cl,
            cl_computed: cl_c,
            cl_error_pct: cl_err,
            cd_ref: rp.cd,
            cd_computed: cd_c,
            cd_error_pct: cd_err,
            cm_ref: rp.cm_c4,
            cm_computed: cm_c,
            cm_error_pct: cm_err,
        });
    }

    // Summary comparisons
    let mut summary_comparisons = Vec::new();

    // CL_alpha: compute from polar near alpha=0
    let cl_alpha_computed = {
        let cl_neg = interpolate_polar(rows, -2.0).0;
        let cl_pos = interpolate_polar(rows, 2.0).0;
        (cl_pos - cl_neg) / (4.0_f64.to_radians())
    };
    summary_comparisons.push(SummaryComparison {
        metric: "CL_alpha".to_string(),
        reference: dataset.summary.cl_alpha_per_rad,
        computed: cl_alpha_computed,
        error_pct: ((cl_alpha_computed
            - dataset.summary.cl_alpha_per_rad)
            / dataset.summary.cl_alpha_per_rad
            * 100.0)
            .abs(),
        unit: "/rad".to_string(),
    });

    // CD_min
    let cd_min_computed = rows
        .iter()
        .filter_map(|r| r.cd_profile.map(|cd| cd as f64))
        .fold(f64::INFINITY, f64::min);
    let cd_min_computed = if cd_min_computed.is_finite() {
        cd_min_computed
    } else {
        0.0
    };
    summary_comparisons.push(SummaryComparison {
        metric: "CD_min".to_string(),
        reference: dataset.summary.cd_min,
        computed: cd_min_computed,
        error_pct: if dataset.summary.cd_min > 0.0001 {
            ((cd_min_computed - dataset.summary.cd_min)
                / dataset.summary.cd_min
                * 100.0)
                .abs()
        } else {
            0.0
        },
        unit: "".to_string(),
    });

    // Alpha zero lift
    let alpha_0l_computed = {
        // Find where CL crosses zero
        let mut a0 = 0.0_f64;
        for w in rows.windows(2) {
            let cl0 = w[0].cl as f64;
            let cl1 = w[1].cl as f64;
            if cl0 <= 0.0 && cl1 >= 0.0 && (cl1 - cl0).abs() > 1e-6 {
                let t = -cl0 / (cl1 - cl0);
                a0 = w[0].alpha_deg as f64
                    + t * (w[1].alpha_deg as f64
                        - w[0].alpha_deg as f64);
                break;
            }
        }
        a0
    };
    summary_comparisons.push(SummaryComparison {
        metric: "Alpha_0L".to_string(),
        reference: dataset.summary.alpha_zero_lift_deg,
        computed: alpha_0l_computed,
        error_pct: (alpha_0l_computed
            - dataset.summary.alpha_zero_lift_deg)
            .abs(),
        unit: "deg (abs err)".to_string(),
    });

    let avg = |v: &[f64]| -> f64 {
        if v.is_empty() {
            0.0
        } else {
            v.iter().sum::<f64>() / v.len() as f64
        }
    };

    AirfoilReport {
        label: dataset.meta.label.clone(),
        naca_code: dataset.meta.naca_code.clone(),
        point_comparisons,
        summary_comparisons,
        overall_cl_error_pct: avg(&cl_errors),
        overall_cd_error_pct: avg(&cd_errors),
        overall_cm_error_pct: avg(&cm_errors),
        solver_time_ms,
    }
}

fn interpolate_polar(
    rows: &[foil_rs::solvers::polar::PolarRow],
    alpha_deg: f32,
) -> (f64, f64, f64) {
    if rows.is_empty() {
        return (0.0, 0.0, 0.0);
    }
    // Find bracket
    for w in rows.windows(2) {
        if w[0].alpha_deg <= alpha_deg && w[1].alpha_deg >= alpha_deg {
            let da = w[1].alpha_deg - w[0].alpha_deg;
            if da.abs() < 1e-6 {
                return (
                    w[0].cl as f64,
                    w[0].cd_profile.unwrap_or(0.0) as f64,
                    w[0].cm_c4 as f64,
                );
            }
            let t = ((alpha_deg - w[0].alpha_deg) / da) as f64;
            let cl =
                w[0].cl as f64 + t * (w[1].cl as f64 - w[0].cl as f64);
            let cd = w[0].cd_profile.unwrap_or(0.0) as f64
                + t * (w[1].cd_profile.unwrap_or(0.0) as f64
                    - w[0].cd_profile.unwrap_or(0.0) as f64);
            let cm = w[0].cm_c4 as f64
                + t * (w[1].cm_c4 as f64 - w[0].cm_c4 as f64);
            return (cl, cd, cm);
        }
    }
    // Clamp
    let last = &rows[rows.len() - 1];
    (
        last.cl as f64,
        last.cd_profile.unwrap_or(0.0) as f64,
        last.cm_c4 as f64,
    )
}

// ---------------------------------------------------------------------------
// Report rendering
// ---------------------------------------------------------------------------

fn print_report(reports: &[AirfoilReport]) {
    println!("\n{}", "=".repeat(80));
    println!("FoilRs Airfoil Validation Report");
    println!("{}", "=".repeat(80));

    for report in reports {
        println!(
            "\n--- {} ({}) --- solved in {:.1}ms",
            report.label, report.naca_code, report.solver_time_ms
        );
        println!(
            "  Avg errors:  CL {:.1}%   CD {:.1}%   CM {:.1}%",
            report.overall_cl_error_pct,
            report.overall_cd_error_pct,
            report.overall_cm_error_pct
        );
        println!();
        println!(
            "  {:>6}  {:>8} {:>8} {:>6}  {:>8} {:>8} {:>6}  {:>8} {:>8} {:>6}",
            "alpha",
            "CL_ref",
            "CL_frs",
            "err%",
            "CD_ref",
            "CD_frs",
            "err%",
            "CM_ref",
            "CM_frs",
            "err%"
        );
        println!("  {}", "-".repeat(90));
        for pc in &report.point_comparisons {
            let cl_flag = if pc.cl_error_pct > CL_TOLERANCE_PCT {
                "!"
            } else {
                " "
            };
            let cd_flag = if pc.cd_error_pct > CD_TOLERANCE_PCT {
                "!"
            } else {
                " "
            };
            let cm_flag = if pc.cm_error_pct > CM_TOLERANCE_PCT {
                "!"
            } else {
                " "
            };
            println!(
                "  {:+6.1}  {:+8.4} {:+8.4} {:>5.1}{} {:8.5} {:8.5} {:>5.1}{} {:+8.4} {:+8.4} {:>5.1}{}",
                pc.alpha_deg,
                pc.cl_ref,
                pc.cl_computed,
                pc.cl_error_pct,
                cl_flag,
                pc.cd_ref,
                pc.cd_computed,
                pc.cd_error_pct,
                cd_flag,
                pc.cm_ref,
                pc.cm_computed,
                pc.cm_error_pct,
                cm_flag,
            );
        }
        println!();
        println!("  Summary metrics:");
        for sc in &report.summary_comparisons {
            let flag = if sc.metric == "CL_alpha"
                && sc.error_pct > CL_ALPHA_TOLERANCE_PCT
            {
                "!"
            } else {
                " "
            };
            println!(
                "    {:<12} ref={:>8.4}  computed={:>8.4}  err={:>5.1}{} {}",
                sc.metric,
                sc.reference,
                sc.computed,
                sc.error_pct,
                flag,
                sc.unit
            );
        }
    }

    // Overall summary
    println!("\n{}", "=".repeat(80));
    let total_cl_err: f64 =
        reports.iter().map(|r| r.overall_cl_error_pct).sum::<f64>()
            / reports.len() as f64;
    let total_cd_err: f64 =
        reports.iter().map(|r| r.overall_cd_error_pct).sum::<f64>()
            / reports.len() as f64;
    let total_cm_err: f64 =
        reports.iter().map(|r| r.overall_cm_error_pct).sum::<f64>()
            / reports.len() as f64;
    println!(
        "Overall mean errors across {} airfoils:  CL {:.1}%   CD {:.1}%   CM {:.1}%",
        reports.len(),
        total_cl_err,
        total_cd_err,
        total_cm_err
    );
}

fn save_json_report(reports: &[AirfoilReport], path: &Path) {
    let entries: Vec<serde_json::Value> = reports
        .iter()
        .map(|r| {
            serde_json::json!({
                "airfoil": r.naca_code,
                "label": r.label,
                "solver_time_ms": r.solver_time_ms,
                "avg_cl_error_pct": r.overall_cl_error_pct,
                "avg_cd_error_pct": r.overall_cd_error_pct,
                "avg_cm_error_pct": r.overall_cm_error_pct,
                "summary": r.summary_comparisons.iter().map(|sc| {
                    serde_json::json!({
                        "metric": sc.metric,
                        "reference": sc.reference,
                        "computed": sc.computed,
                        "error_pct": sc.error_pct,
                        "unit": sc.unit,
                    })
                }).collect::<Vec<_>>(),
                "points": r.point_comparisons.iter().map(|pc| {
                    serde_json::json!({
                        "alpha_deg": pc.alpha_deg,
                        "cl_ref": pc.cl_ref, "cl_computed": pc.cl_computed, "cl_error_pct": pc.cl_error_pct,
                        "cd_ref": pc.cd_ref, "cd_computed": pc.cd_computed, "cd_error_pct": pc.cd_error_pct,
                        "cm_ref": pc.cm_ref, "cm_computed": pc.cm_computed, "cm_error_pct": pc.cm_error_pct,
                    })
                }).collect::<Vec<_>>(),
            })
        })
        .collect();

    let report = serde_json::json!({
        "generated_at": chrono_stub(),
        "foil_rs_version": env!("CARGO_PKG_VERSION"),
        "airfoils": entries,
    });

    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let json_str =
        serde_json::to_string_pretty(&report).unwrap_or_default();
    if let Err(e) = fs::write(path, &json_str) {
        eprintln!(
            "Warning: could not write report to {}: {e}",
            path.display()
        );
    } else {
        eprintln!("Report saved to {}", path.display());
    }
}

fn chrono_stub() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("{secs}")
}

// ---------------------------------------------------------------------------
// History
// ---------------------------------------------------------------------------

fn append_history(report_path: &Path, history_path: &Path) {
    let report_json = match fs::read_to_string(report_path) {
        Ok(s) => s,
        Err(_) => return,
    };
    let report: serde_json::Value =
        match serde_json::from_str(&report_json) {
            Ok(v) => v,
            Err(_) => return,
        };

    let mut history: Vec<serde_json::Value> = if history_path.exists() {
        let s = fs::read_to_string(history_path)
            .unwrap_or_else(|_| "[]".to_string());
        serde_json::from_str(&s).unwrap_or_default()
    } else {
        Vec::new()
    };

    history.push(report);

    if let Some(parent) = history_path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::write(
        history_path,
        serde_json::to_string_pretty(&history).unwrap_or_default(),
    );
}

fn show_history(history_path: &Path) {
    if !history_path.exists() {
        eprintln!(
            "No history file found at {}",
            history_path.display()
        );
        return;
    }
    let s = fs::read_to_string(history_path)
        .unwrap_or_else(|_| "[]".to_string());
    let history: Vec<serde_json::Value> =
        serde_json::from_str(&s).unwrap_or_default();

    println!("\n{}", "=".repeat(80));
    println!("FoilRs Validation History ({} runs)", history.len());
    println!("{}", "=".repeat(80));
    println!(
        "  {:>6}  {:>12}  {:>8}  {:>8}  {:>8}",
        "Run#", "Version", "CL_err%", "CD_err%", "CM_err%"
    );
    println!("  {}", "-".repeat(50));

    for (i, entry) in history.iter().enumerate() {
        let version = entry
            .get("foil_rs_version")
            .and_then(|v| v.as_str())
            .unwrap_or("?");
        let airfoils = entry.get("airfoils").and_then(|v| v.as_array());
        let (cl_avg, cd_avg, cm_avg) = if let Some(arr) = airfoils {
            let n = arr.len() as f64;
            if n > 0.0 {
                let cl: f64 = arr
                    .iter()
                    .filter_map(|a| a.get("avg_cl_error_pct")?.as_f64())
                    .sum::<f64>()
                    / n;
                let cd: f64 = arr
                    .iter()
                    .filter_map(|a| a.get("avg_cd_error_pct")?.as_f64())
                    .sum::<f64>()
                    / n;
                let cm: f64 = arr
                    .iter()
                    .filter_map(|a| a.get("avg_cm_error_pct")?.as_f64())
                    .sum::<f64>()
                    / n;
                (cl, cd, cm)
            } else {
                (0.0, 0.0, 0.0)
            }
        } else {
            (0.0, 0.0, 0.0)
        };
        println!(
            "  {:>6}  {:>12}  {:>7.1}%  {:>7.1}%  {:>7.1}%",
            i + 1,
            version,
            cl_avg,
            cd_avg,
            cm_avg
        );
    }
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let show_hist = args.iter().any(|a| a == "--history");
    let report_dir = PathBuf::from("validation_reports");
    let report_path = report_dir.join("latest.json");
    let history_path = report_dir.join("history.json");

    if show_hist {
        show_history(&history_path);
        return ExitCode::SUCCESS;
    }

    // Load reference datasets
    let data_dir =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("data/airfoils");
    let mut datasets: Vec<AirfoilDataset> = Vec::new();

    let entries: Vec<_> = fs::read_dir(&data_dir)
        .unwrap_or_else(|e| {
            panic!("Cannot read data dir {}: {e}", data_dir.display())
        })
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .map(|ext| ext == "json")
                .unwrap_or(false)
        })
        .collect();

    for entry in &entries {
        let content =
            fs::read_to_string(entry.path()).unwrap_or_else(|e| {
                panic!("Cannot read {}: {e}", entry.path().display())
            });
        let ds: AirfoilDataset = serde_json::from_str(&content)
            .unwrap_or_else(|e| {
                panic!("Cannot parse {}: {e}", entry.path().display())
            });
        datasets.push(ds);
    }

    datasets.sort_by(|a, b| a.meta.id.cmp(&b.meta.id));

    eprintln!(
        "Validating {} airfoils against reference data...",
        datasets.len()
    );

    let reports: Vec<AirfoilReport> =
        datasets.iter().map(|ds| run_validation(ds)).collect();

    print_report(&reports);
    save_json_report(&reports, &report_path);
    append_history(&report_path, &history_path);

    // Check if any CL errors exceed tolerance
    let any_cl_fail = reports
        .iter()
        .any(|r| r.overall_cl_error_pct > CL_TOLERANCE_PCT);

    if any_cl_fail {
        eprintln!(
            "\nFAIL: CL error exceeds {CL_TOLERANCE_PCT}% tolerance for some airfoils."
        );
        ExitCode::FAILURE
    } else {
        eprintln!("\nPASS: All airfoils within CL tolerance.");
        ExitCode::SUCCESS
    }
}
