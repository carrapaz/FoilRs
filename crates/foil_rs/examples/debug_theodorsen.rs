use foil_rs::solvers::theodorsen::solve_theodorsen;

fn main() {
    println!("=== NACA 0012 α=4° Cp ===");
    let r = solve_theodorsen(0.0, 0.0, 0.12, 4.0_f64.to_radians(), 200);
    println!("CL={:.4} (ref 0.433)", r.cl);
    println!("x\t\ty\t\tv/V\t\tCp");
    for i in (0..r.x.len()).step_by(5) {
        println!(
            "{:.4}\t\t{:+.4}\t\t{:.4}\t\t{:+.4}",
            r.x[i], r.y[i], r.v_ratio[i], r.cp[i]
        );
    }

    println!("\n=== NACA 2412 α=0° Cp ===");
    let r = solve_theodorsen(0.02, 0.4, 0.12, 0.0, 200);
    println!("CL={:.4} (ref 0.23), CM={:.4} (ref -0.053)", r.cl, r.cm_c4);
    println!("x\t\ty\t\tv/V\t\tCp");
    for i in (0..r.x.len()).step_by(5) {
        println!(
            "{:.4}\t\t{:+.4}\t\t{:.4}\t\t{:+.4}",
            r.x[i], r.y[i], r.v_ratio[i], r.cp[i]
        );
    }

    println!("\n=== Summary ===");
    for (name, m, p, t) in [
        ("0008", 0.0, 0.0, 0.08),
        ("0012", 0.0, 0.0, 0.12),
        ("2412", 0.02, 0.4, 0.12),
        ("4412", 0.04, 0.4, 0.12),
    ] {
        let r0 = solve_theodorsen(m, p, t, 0.0, 200);
        let r4 = solve_theodorsen(m, p, t, 4.0_f64.to_radians(), 200);
        println!(
            "NACA {}: CL(0)={:+.4}  CL(4)={:+.4}  CM={:+.4}  ε_T={:+.2}°",
            name,
            r0.cl,
            r4.cl,
            r0.cm_c4,
            r0.epsilon_t.to_degrees()
        );
    }
}
