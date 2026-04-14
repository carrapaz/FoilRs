use bevy::prelude::{Deref, DerefMut, Resource};

pub use foil_rs::state::{cl_thin, reference_coeffs};

#[derive(Default, Resource, Clone, Deref, DerefMut)]
pub struct NacaParams(pub foil_rs::state::NacaParams);

#[derive(Default, Resource, Clone, Deref, DerefMut)]
pub struct FlowSettings(pub foil_rs::state::FlowSettings);
