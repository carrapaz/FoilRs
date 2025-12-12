pub mod naca;

pub use naca::{
    build_naca_body_geometry, build_naca_body_geometry_sharp_te,
    camber_line, camber_slope, thickness_distribution,
};
