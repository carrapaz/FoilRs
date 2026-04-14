use foil_rs::solvers::theodorsen::solve_theodorsen;

fn main() {
    println!("=== Accuracy Summary ===\n");

    // Symmetric
    for alpha in [0.0_f64, 2.0, 4.0, 6.0, 8.0] {
        let r = solve_theodorsen(
            0.0,
            0.0,
            0.12,
            alpha.to_radians(),
            256,
        );
        println!(
            "NACA 0012 α={:2}°: CL={:+.4} ε_T={:+.4}° iters={}",
            alpha,
            r.cl,
            r.epsilon_t.to_degrees(),
            r.iterations
        );
    }

    println!();

    // Cambered
    for (name, m, p) in
        [("2412", 0.02, 0.4), ("4412", 0.04, 0.4)]
    {
        for alpha in [0.0_f64, 2.0, 4.0] {
            let r = solve_theodorsen(
                m,
                p,
                0.12,
                alpha.to_radians(),
                256,
            );
            println!(
                "NACA {} α={:2}°: CL={:+.4} ε_T={:+.4}° CM={:+.4} iters={}",
                name, alpha, r.cl, r.epsilon_t.to_degrees(), r.cm_c4, r.iterations
            );
        }
        println!();
    }

    // Reference values from thin airfoil theory
    println!("=== Reference (thin airfoil) ===");
    println!("NACA 0012 CL_alpha = 2π = {:.4}", 2.0 * std::f64::consts::PI);
    println!("NACA 2412 α_L0 ≈ -2.1°, CL(0) ≈ 0.23");
    println!("NACA 4412 α_L0 ≈ -4.0°, CL(0) ≈ 0.44");
    println!("NACA 2412 CM_c/4 ≈ -0.053");
    println!("NACA 4412 CM_c/4 ≈ -0.104");
}
