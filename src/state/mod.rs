#[cfg(feature = "bevy")]
use bevy::prelude::*;
use std::f32::consts::PI;

/// Parameters for a NACA 4-digit airfoil.
#[cfg_attr(feature = "bevy", derive(Resource))]
#[derive(Clone)]
pub struct NacaParams {
    /// First digit: maximum camber in % of chord (0–9).
    pub m_digit: f32,
    /// Second digit: position of max camber in tenths of chord (0–9 → 0.0–0.9).
    pub p_digit: f32,
    /// Last two digits: thickness in % of chord (01–40).
    pub t_digits: f32,
    /// Sampling points per surface (for geometry).
    pub num_points: usize,
}

impl Default for NacaParams {
    fn default() -> Self {
        // Classic test case: NACA 2412
        Self {
            m_digit: 2.0,
            p_digit: 4.0,
            t_digits: 12.0,
            num_points: 160,
        }
    }
}

impl NacaParams {
    /// Maximum camber (m) in chord fractions.
    pub fn m(&self) -> f32 {
        self.m_digit / 100.0
    }
    /// Position of maximum camber (p) in chord fractions.
    pub fn p(&self) -> f32 {
        self.p_digit / 10.0
    }
    /// Thickness (t) in chord fractions.
    pub fn t(&self) -> f32 {
        self.t_digits / 100.0
    }
    pub fn code(&self) -> String {
        // e.g. "2412"
        format!(
            "{:.0}{:.0}{:02.0}",
            self.m_digit, self.p_digit, self.t_digits
        )
    }

    /// Parse a 4-digit NACA code (e.g. `"2412"`) into parameters.
    ///
    /// Returns `None` if the string is not exactly 4 ASCII digits.
    pub fn from_naca4(code: &str) -> Option<Self> {
        let code = code.trim();
        let bytes = code.as_bytes();
        if bytes.len() != 4 {
            return None;
        }
        if !bytes.iter().all(|b| b.is_ascii_digit()) {
            return None;
        }

        let m = (bytes[0] - b'0') as f32;
        let p = (bytes[1] - b'0') as f32;
        let t = ((bytes[2] - b'0') as u16 * 10
            + (bytes[3] - b'0') as u16) as f32;

        Some(Self {
            m_digit: m,
            p_digit: p,
            t_digits: t,
            ..Self::default()
        })
    }
}

/// Angle of attack (deg).
#[cfg_attr(feature = "bevy", derive(Resource))]
#[derive(Clone)]
pub struct FlowSettings {
    pub alpha_deg: f32,
    pub reynolds: f32,
    pub mach: f32,
    pub viscous: bool,
    pub free_transition: bool,
}

impl Default for FlowSettings {
    fn default() -> Self {
        Self {
            alpha_deg: 4.0,
            reynolds: 1_000_000.0,
            mach: 0.10,
            viscous: true,
            free_transition: true,
        }
    }
}

/// Thin airfoil theory: CL ≈ 2π α (α in radians) for small α.
pub fn cl_thin(alpha_deg: f32) -> f32 {
    let alpha_rad = alpha_deg.to_radians();
    2.0 * PI * alpha_rad
}

/// Reference coefficients from XFoil or known data for specific cases.
pub fn reference_coeffs(
    params: &NacaParams,
    alpha_deg: f32,
) -> Option<(f32, f32, f32)> {
    // Known: NACA 2412 at α = 0
    if params.code() == "2412" && alpha_deg.abs() < 1e-3 {
        return Some((0.2554, -0.0557, -0.00119));
    }
    None
}
