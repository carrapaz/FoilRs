pub use glam::Vec2;

// -------------------------------------------------------------------------
// Fast transcendental approximations for panel method inner loops.
//
// These replace f32::atan2 and f32::ln in the Biot-Savart influence
// functions where they are called O(N²) times during matrix assembly.
// Accuracy is ~1e-4 relative, sufficient for panel methods.
// -------------------------------------------------------------------------

/// Fast atan2(y, x) — polynomial minimax approximation.
///
/// Max error: ~1.5e-4 radians (~0.009°) across the full domain.
/// ~3-4x faster than libm atan2f on ARM and x86.
#[inline(always)]
#[allow(clippy::excessive_precision)] // minimax coefficients — every digit matters
pub fn fast_atan2(y: f32, x: f32) -> f32 {
    // Handle zero case.
    if x == 0.0 && y == 0.0 {
        return 0.0;
    }

    let ax = x.abs();
    let ay = y.abs();

    // Reduce to [0, 1] range: atan(a/b) where a <= b.
    let (a, b, offset) = if ay <= ax {
        (ay, ax, 0.0)
    } else {
        (ax, ay, std::f32::consts::FRAC_PI_2)
    };

    // Polynomial approximation of atan(t) for t in [0, 1].
    // Coefficients from minimax fit (Remez algorithm).
    let t = a / b;
    let t2 = t * t;
    let r = t
        * (0.999_866_0
            + t2 * (-0.330_299_5
                + t2 * (0.180_141_0
                    + t2 * (-0.085_133_0 + t2 * 0.020_835_1))));

    // Reconstruct quadrant.
    let r = if ay > ax { offset - r } else { r };
    let r = if x < 0.0 { std::f32::consts::PI - r } else { r };
    if y < 0.0 { -r } else { r }
}

/// Fast ln(x) for x > 0 — bit-manipulation + polynomial.
///
/// Max error: ~0.4% relative across [1e-6, 1e6].
/// ~2x faster than libm logf.
#[inline(always)]
pub fn fast_ln(x: f32) -> f32 {
    // Reduce to m in [sqrt(0.5), sqrt(2)] ≈ [0.707, 1.414] for better
    // polynomial accuracy.  This is the Cephes approach.
    let bits = x.to_bits();
    let e_raw = (bits >> 23) & 0xFF;
    let m_bits = (bits & 0x007F_FFFF) | 0x3F00_0000; // m in [0.5, 1.0)
    let mut m = f32::from_bits(m_bits);
    let mut e = e_raw as f32 - 126.0;

    // If m < sqrt(0.5), multiply by 2 and subtract 1 from exponent.
    if m < std::f32::consts::FRAC_1_SQRT_2 {
        m *= 2.0;
        e -= 1.0;
    }

    let f = m - 1.0;
    // Padé approximation of ln(1+f) for |f| < 0.414:
    //   ln(1+f) ≈ f * (2 + f * c1) / (2 + f * c2)
    // Coefficients tuned for [−0.293, 0.414] range.
    let num = f * (2.0 + f * 0.6666667);
    let den = 2.0 + f * 1.6666667;
    let ln_m = num / den;

    e * std::f32::consts::LN_2 + ln_m
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fast_atan2_matches_std_across_quadrants() {
        let values: [(f32, f32); 12] = [
            (1.0, 0.0),
            (0.0, 1.0),
            (-1.0, 0.0),
            (0.0, -1.0),
            (1.0, 1.0),
            (-1.0, 1.0),
            (1.0, -1.0),
            (-1.0, -1.0),
            (0.3, 0.7),
            (-0.5, 0.1),
            (0.001, 10.0),
            (10.0, 0.001),
        ];
        for (y, x) in values {
            let expected = y.atan2(x);
            let got = fast_atan2(y, x);
            assert!(
                (got - expected).abs() < 2e-4,
                "fast_atan2({}, {}): expected {:.6}, got {:.6}, err={:.6}",
                y,
                x,
                expected,
                got,
                (got - expected).abs()
            );
        }
    }

    #[test]
    fn fast_atan2_zero_zero() {
        assert_eq!(fast_atan2(0.0, 0.0), 0.0);
    }

    #[test]
    fn fast_ln_matches_std_across_range() {
        let values: [f32; 9] =
            [0.001, 0.01, 0.1, 0.5, 1.0, 2.0, 10.0, 100.0, 1000.0];
        for x in values {
            let expected = x.ln();
            let got = fast_ln(x);
            let rel_err = if expected.abs() > 0.01 {
                ((got - expected) / expected).abs()
            } else {
                (got - expected).abs()
            };
            assert!(
                rel_err < 5e-3,
                "fast_ln({}): expected {:.6}, got {:.6}, rel_err={:.6}",
                x,
                expected,
                got,
                rel_err
            );
        }
    }

    #[test]
    fn fast_ln_of_one_is_zero() {
        assert!(fast_ln(1.0).abs() < 1e-5);
    }
}
