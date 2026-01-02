//! FoilRs core library (no Bevy dependency).
//!
//! This crate provides:
//! - NACA 4-digit airfoil geometry generation (`airfoil`)
//! - A lightweight vortex panel solver + Cp sampling (`solvers::panel`)
//! - Polar sweeps and CSV-friendly result structures (`solvers::polar`)
//! - A small boundary-layer estimate for profile drag (`solvers::boundary_layer`)
//!
//! For an interactive UI front-end, see the `foil_rs_bevy` crate.

pub mod airfoil;
pub mod math;
pub mod solvers;
pub mod state;
