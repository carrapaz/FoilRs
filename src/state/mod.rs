use bevy::prelude::*;
use std::f32::consts::PI;

/// Parameters for a NACA 4-digit airfoil.
#[derive(Resource, Clone)]
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
}

/// Angle of attack (deg).
#[derive(Resource, Clone)]
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

/// Which visualization we’re showing.
#[derive(Resource, Clone, Copy, PartialEq, Eq)]
pub enum VisualMode {
    /// Velocity field + streamlines.
    Field,
    /// Cp(x) distribution.
    Cp,
    /// Polar curves (CL/CD/CM vs α).
    Polars,
    /// Panel discretization visualization.
    Panels,
}

/// Which value a text cell in the table represents.
#[derive(Component, Clone, Copy)]
pub enum TableField {
    NacaCode,
    AlphaDeg,
    Reynolds,
    Mach,
    ClThin,
    RefCl,
    RefCm,
    RefCdp,
    FlowState,
    ViscosityMode,
    TransitionMode,
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
