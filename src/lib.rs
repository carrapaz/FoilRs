pub mod airfoil;
pub mod solvers;
pub mod state;
pub mod math;

#[cfg(feature = "bevy")]
pub mod plotter;
#[cfg(feature = "bevy")]
pub mod ui;
#[cfg(feature = "bevy")]
pub mod views;
