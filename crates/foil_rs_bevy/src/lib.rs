//! FoilRs Bevy frontend.
//!
//! This crate contains the Bevy UI + visualization app. The core solver and
//! geometry code lives in the `foil_rs` crate.

pub use foil_rs::{airfoil, math, solvers};

pub mod plotter;
pub mod state;
pub mod ui;
pub mod views;
