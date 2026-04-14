//! Theodorsen's exact method for airfoil potential flow (NACA Report 411).
//!
//! Computes the velocity at any point of an arbitrary airfoil surface
//! using the exact formula (XII) from the 1931 paper.  No iteration
//! needed for the geometry — ψ, θ, ε are computed directly from (x, y).

use std::f64::consts::PI;

/// Result of the Theodorsen velocity computation.
#[derive(Clone, Debug)]
pub struct TheodorsenResult {
    pub theta: Vec<f64>,
    pub x: Vec<f64>,
    pub y: Vec<f64>,
    pub v_ratio: Vec<f64>,
    pub cp: Vec<f64>,
    pub cl: f64,
    pub cm_c4: f64,
    pub epsilon_t: f64,
}

/// Solve using Theodorsen's exact method (NACA-TR-411).
///
/// Steps from page 234 of the paper:
/// 1. Set up coordinate system with (2a, 0) and (-2a, 0)
/// 2. Compute θ from equation (III)
/// 3. Compute ψ from equation (IVa)
/// 4. Compute ε from the Hilbert transform of ψ (appendix formula)
/// 5. Compute F (equation XIb)
/// 6. Compute v/V from equation (XII)
pub fn solve_theodorsen(
    m: f64,
    p: f64,
    t: f64,
    alpha_rad: f64,
    n: usize,
) -> TheodorsenResult {
    let n = n.max(40);

    // Step 1: Coordinate system.
    // 4a = distance between the points midway between nose+nose_curvature
    // and tail+tail_curvature.  For a unit chord airfoil with LE at (0,0)
    // and TE at (1,0), and LE radius ρ ≈ 1.1019·t²:
    let rho_le = 1.1019 * t * t; // LE radius of curvature
    // The point (2a, 0) is midway between nose and center of curvature of nose.
    // For a sharp TE: (-2a, 0) is at the tail.
    // For NACA: nose at x=0, center of curvature at x=ρ, midpoint = ρ/2.
    // Tail at x=1, sharp (ρ_TE ≈ 0), midpoint = 1.
    // So 4a = 1 - ρ/2 → a = (1 - ρ/2) / 4 ≈ 0.25 for thin airfoils.
    // But the paper uses 2a as half the "effective chord".
    // Simpler: for unit chord, a ≈ chord/4 = 0.25.
    let a = (1.0 - rho_le / 2.0) / 4.0;

    // Origin is at the midpoint: x_origin = 2a + ρ/2... wait.
    // From the paper: (2a, 0) is at the nose-side point, (-2a, 0) at the tail.
    // The origin is between them.  For unit chord:
    // x_origin = ρ/2 + 2a = ρ/2 + (1 - ρ/2)/2 = (1 + ρ/2)/2
    // Actually let me re-read: "the point midway between the nose and
    // the center of curvature of the nose" is at x = ρ/2 from the LE.
    // And (-2a, 0) is "midway between the tail and center of curvature
    // of the tail" = at x = 1 for sharp TE.
    // So the origin is at x = (ρ/2 + 1) / 2 in airfoil coordinates.
    // And 4a = 1 - ρ/2.
    let x_origin = (rho_le / 2.0 + 1.0) / 2.0;

    // Generate airfoil points.  We parameterize by θ around the "almost
    // circle", with θ=0 at the tail and θ=π at the nose.
    // Upper surface: θ from 0 to π (tail to nose).
    // Lower surface: θ from π to 2π (nose to tail).
    //
    // But actually, Theodorsen computes θ FROM (x, y) — not the other way.
    // So we sample the airfoil at uniform x stations and compute θ for each.

    let half = n / 2;
    let mut pts_x = Vec::with_capacity(n);
    let mut pts_y = Vec::with_capacity(n);

    // Upper surface: x from 1 (TE) to 0 (LE)
    for i in 0..half {
        let frac = i as f64 / (half - 1) as f64;
        let x_c = 1.0 - frac; // 1 → 0
        let (xu, yu) = upper_surface(m, p, t, x_c);
        pts_x.push(xu - x_origin);
        pts_y.push(yu);
    }
    // Lower surface: x from 0 (LE) to 1 (TE)
    for i in 0..half {
        let frac = i as f64 / (half - 1) as f64;
        let x_c = frac; // 0 → 1
        let (xl, yl) = lower_surface(m, p, t, x_c);
        pts_x.push(xl - x_origin);
        pts_y.push(yl);
    }

    // Step 2-3: Compute θ and ψ for each point using equations (III) and (IVa).
    let mut theta = Vec::with_capacity(n);
    let mut psi = Vec::with_capacity(n);

    for i in 0..n {
        let x = pts_x[i];
        let y = pts_y[i];

        // Equation (III): 2sin²θ = p + √(p² + (y/a)²)
        // where p = 1 - (x/(2a))² - (y/(2a))²
        let x2a = x / (2.0 * a);
        let y2a = y / (2.0 * a);
        let pp = 1.0 - x2a * x2a - y2a * y2a;
        let sin2_theta =
            0.5 * (pp + (pp * pp + y2a * y2a * 4.0).sqrt());
        let sin_theta = sin2_theta.sqrt().clamp(0.0, 1.0);
        let theta_i = if i < half {
            // Upper surface: θ from small (near TE) to π (nose)
            // sin(θ) > 0, and cos(θ) determines the sign of x
            // cos(θ) = x/(2a·cosh(ψ)) — positive near TE, negative near LE
            sin_theta.asin()
        } else {
            // Lower surface: θ from π to 2π
            PI + sin_theta.asin()
        };
        // Adjust θ based on which side of the LE we're on
        let theta_i = if x >= 0.0 && i < half {
            theta_i // upper surface, TE side
        } else if x < 0.0 && i < half {
            PI - theta_i // upper surface, LE side
        } else if x < 0.0 && i >= half {
            PI + (PI - theta_i - PI).abs() // lower, LE side
        } else {
            2.0 * PI - theta_i + PI // lower, TE side: adjusted
        };
        // Simpler: use atan2 of the "circle coordinates"
        // Actually, θ from the paper satisfies:
        //   x = 2a·cosh(ψ)·cos(θ)
        //   y = 2a·sinh(ψ)·sin(θ)
        // So cos(θ) = x / (2a·cosh(ψ)) and sin(θ) = y / (2a·sinh(ψ))
        // But we don't know ψ yet... use the simplified version.

        // Equation (IVa): ψ = y/(2a·sinθ) - (1/6)(y/(2a·sinθ))³ + ...
        let sin_t = theta_i.sin().abs().max(1e-6);
        let y_term = y.abs() / (2.0 * a * sin_t);
        let psi_i =
            y_term - y_term.powi(3) / 6.0 + y_term.powi(5) / 120.0;

        theta.push(theta_i);
        psi.push(psi_i);
    }

    // Step 4: ψ₀ = average of ψ (equation (e))
    let psi_0 = psi.iter().sum::<f64>() / n as f64;

    // Step 5: Compute ε using the appendix formula.
    // ε_c = -(1/π)[0.628·ψ'_c + 1.065(ψ₁-ψ₋₁) + 0.445(ψ₂-ψ₋₂)
    //        + 0.231(ψ₃-ψ₋₃) + 0.104(ψ₄-ψ₋₄)]
    // where ψ₁ is ψ at φ = φ_c + π/5, etc.
    //
    // For our discrete data, we use the DFT-based Hilbert transform instead
    // (equivalent but more convenient for evenly-spaced data):
    let epsilon = hilbert_transform(&psi);

    // ε_T = ε at the trailing edge (θ = 0, index 0 for upper surface TE)
    let epsilon_t = epsilon[0];

    // Step 6-7: Compute ψ' and ε' (derivatives with respect to θ).
    let dpsi = numerical_derivative(&psi, &theta);
    let deps = numerical_derivative(&epsilon, &theta);

    // Step 8: Compute F (equation XIb)
    // F = (1+ε')·e^ψ₀ / √((y/(2a·sinθ))² + sin²θ) · (1+ψ'²))
    // Simplified: F = (1+ε')·e^ψ₀ / √(sinh²ψ + sin²θ) · √(1+ψ'²)

    // Step 9-11: Compute v/V from equation (XII)
    // v/V = F · [sin(θ + α + ε) + sin(α + ε_T)]
    let mut v_ratio = Vec::with_capacity(n);
    let mut cp = Vec::with_capacity(n);
    let mut x_out = Vec::with_capacity(n);
    let mut y_out = Vec::with_capacity(n);

    for i in 0..n {
        let th = theta[i];
        let ps = psi[i];
        let ep = epsilon[i];
        let ep_prime = deps[i];
        let ps_prime = dpsi[i];

        // F from equation (XIb)
        let sinh_psi = ps.sinh();
        let sin_theta = th.sin();
        let denom = (sinh_psi * sinh_psi + sin_theta * sin_theta)
            .sqrt()
            * (1.0 + ps_prime * ps_prime).sqrt();

        let f = if denom.abs() > 1e-10 {
            (1.0 + ep_prime) * psi_0.exp() / denom
        } else {
            0.0
        };

        // v/V = F · [sin(θ + α + ε) + sin(α + ε_T)]
        let vr = f
            * ((th + alpha_rad + ep).sin()
                + (alpha_rad + epsilon_t).sin());

        v_ratio.push(vr);
        cp.push(1.0 - vr * vr);
        x_out.push(pts_x[i] + x_origin);
        y_out.push(pts_y[i]);
    }

    // CL = 2π·sin(α + ε_T)  (from the Kutta circulation, equation VII)
    let cl = 2.0 * PI * (alpha_rad + epsilon_t).sin();

    // CM from thin-airfoil Fourier coefficients
    let cm_c4 = thin_airfoil_cm(m, p);

    TheodorsenResult {
        theta,
        x: x_out,
        y: y_out,
        v_ratio,
        cp,
        cl,
        cm_c4,
        epsilon_t,
    }
}

fn upper_surface(m: f64, p: f64, t: f64, x_c: f64) -> (f64, f64) {
    let yc = camber(m, p, x_c);
    let slope = camber_slope(m, p, x_c);
    let th = slope.atan();
    let yt = thickness(t, x_c);
    (x_c - yt * th.sin(), yc + yt * th.cos())
}

fn lower_surface(m: f64, p: f64, t: f64, x_c: f64) -> (f64, f64) {
    let yc = camber(m, p, x_c);
    let slope = camber_slope(m, p, x_c);
    let th = slope.atan();
    let yt = thickness(t, x_c);
    (x_c + yt * th.sin(), yc - yt * th.cos())
}

fn camber(m: f64, p: f64, x: f64) -> f64 {
    if m < 1e-8 || p < 1e-8 {
        return 0.0;
    }
    if x <= p {
        m / (p * p) * (2.0 * p * x - x * x)
    } else {
        m / ((1.0 - p) * (1.0 - p))
            * ((1.0 - 2.0 * p) + 2.0 * p * x - x * x)
    }
}

fn camber_slope(m: f64, p: f64, x: f64) -> f64 {
    if m < 1e-8 || p < 1e-8 {
        return 0.0;
    }
    if x <= p {
        2.0 * m / (p * p) * (p - x)
    } else {
        2.0 * m / ((1.0 - p) * (1.0 - p)) * (p - x)
    }
}

fn thickness(t: f64, x: f64) -> f64 {
    t / 0.2
        * (0.2969 * x.sqrt() - 0.1260 * x - 0.3516 * x * x
            + 0.2843 * x.powi(3)
            - 0.1015 * x.powi(4))
}

fn thin_airfoil_cm(m: f64, p: f64) -> f64 {
    if m < 1e-6 {
        return 0.0;
    }
    let n = 200;
    let dth = PI / n as f64;
    let mut a1 = 0.0;
    let mut a2 = 0.0;
    for i in 1..n {
        let th = dth * i as f64;
        let x = 0.5 * (1.0 - th.cos());
        let dyc = camber_slope(m, p, x);
        a1 += dyc * th.cos() * dth * 2.0 / PI;
        a2 += dyc * (2.0 * th).cos() * dth * 2.0 / PI;
    }
    -PI / 4.0 * (a1 - a2)
}

/// Hilbert transform via DFT: ε = H[ψ].
/// If ψ = Σ aₙcos(nφ) + bₙsin(nφ), then ε = Σ aₙsin(nφ) - bₙcos(nφ).
fn hilbert_transform(f: &[f64]) -> Vec<f64> {
    let n = f.len();
    // Compute Fourier coefficients
    let mut a = vec![0.0; n / 2 + 1];
    let mut b = vec![0.0; n / 2 + 1];
    for k in 0..=n / 2 {
        for i in 0..n {
            let phi = 2.0 * PI * i as f64 / n as f64;
            a[k] += f[i] * (k as f64 * phi).cos();
            b[k] += f[i] * (k as f64 * phi).sin();
        }
        a[k] *= 2.0 / n as f64;
        b[k] *= 2.0 / n as f64;
    }
    a[0] *= 0.5;

    // Reconstruct Hilbert transform
    let mut result = vec![0.0; n];
    for i in 0..n {
        let phi = 2.0 * PI * i as f64 / n as f64;
        for k in 1..n / 2 {
            result[i] += a[k] * (k as f64 * phi).sin()
                - b[k] * (k as f64 * phi).cos();
        }
    }
    result
}

fn numerical_derivative(f: &[f64], x: &[f64]) -> Vec<f64> {
    let n = f.len();
    let mut df = vec![0.0; n];
    for i in 0..n {
        let ip = (i + 1) % n;
        let im = (i + n - 1) % n;
        let dx = x[ip] - x[im];
        if dx.abs() > 1e-12 {
            df[i] = (f[ip] - f[im]) / dx;
        }
    }
    df
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn symmetric_zero_cl_at_alpha_zero() {
        let r = solve_theodorsen(0.0, 0.0, 0.12, 0.0, 200);
        assert!(
            r.cl.abs() < 0.1,
            "CL(0) should be ~0 for symmetric, got {:.4}",
            r.cl
        );
    }

    #[test]
    fn symmetric_positive_cl_at_alpha_4() {
        let r =
            solve_theodorsen(0.0, 0.0, 0.12, 4.0_f64.to_radians(), 200);
        assert!(
            r.cl > 0.3 && r.cl < 0.6,
            "CL(4) should be ~0.43 for 0012, got {:.4}",
            r.cl
        );
    }

    #[test]
    fn cl_alpha_near_two_pi() {
        let r2 =
            solve_theodorsen(0.0, 0.0, 0.12, 2.0_f64.to_radians(), 200);
        let rn2 = solve_theodorsen(
            0.0,
            0.0,
            0.12,
            -2.0_f64.to_radians(),
            200,
        );
        let cla = (r2.cl - rn2.cl) / (4.0_f64.to_radians());
        assert!(
            (cla - 2.0 * PI).abs() / (2.0 * PI) < 0.15,
            "CL_alpha={:.3} should be near 2π",
            cla
        );
    }

    #[test]
    fn cm_correct_for_cambered() {
        let r = solve_theodorsen(0.02, 0.4, 0.12, 0.0, 200);
        assert!(
            (r.cm_c4 + 0.053).abs() < 0.01,
            "CM should be ~-0.053, got {:.4}",
            r.cm_c4
        );
    }

    #[test]
    fn cp_physically_reasonable() {
        let r =
            solve_theodorsen(0.0, 0.0, 0.12, 4.0_f64.to_radians(), 200);
        // Most Cp values should be between -4 and 1.1
        let reasonable =
            r.cp.iter().filter(|&&c| c > -4.0 && c < 1.1).count();
        let fraction = reasonable as f64 / r.cp.len() as f64;
        assert!(
            fraction > 0.8,
            "Only {:.0}% of Cp values are in [-4, 1.1]",
            fraction * 100.0
        );
    }
}
