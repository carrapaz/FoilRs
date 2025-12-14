#[cfg(feature = "bevy")]
pub use bevy::math::Vec2;

#[cfg(not(feature = "bevy"))]
pub use glam::Vec2;

