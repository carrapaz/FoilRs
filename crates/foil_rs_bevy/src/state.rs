use bevy::prelude::{Deref, DerefMut, Resource};

pub use foil_rs::state::{cl_thin, reference_coeffs};

#[derive(Resource, Clone, Deref, DerefMut)]
pub struct NacaParams(pub foil_rs::state::NacaParams);

impl Default for NacaParams {
    fn default() -> Self {
        Self(foil_rs::state::NacaParams::default())
    }
}

#[derive(Resource, Clone, Deref, DerefMut)]
pub struct FlowSettings(pub foil_rs::state::FlowSettings);

impl Default for FlowSettings {
    fn default() -> Self {
        Self(foil_rs::state::FlowSettings::default())
    }
}
