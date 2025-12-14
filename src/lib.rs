pub mod airfoil;
pub mod math;
pub mod solvers;
pub mod state;

#[cfg(feature = "bevy")]
pub mod plotter;
#[cfg(feature = "bevy")]
pub mod ui;
#[cfg(feature = "bevy")]
pub mod views;
