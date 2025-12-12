use bevy::{
    ecs::hierarchy::ChildSpawnerCommands,
    feathers::controls::{SliderProps, slider},
    prelude::*,
    ui_widgets::{
        SliderPrecision, SliderStep, ValueChange, observe,
        slider_self_update,
    },
};

use crate::{airfoil::build_naca_body_geometry, state::NacaParams};

use super::super::config;
use super::super::types::PanelCountText;

pub(super) fn spawn_panel_settings(
    panel: &mut ChildSpawnerCommands<'_>,
    params: &NacaParams,
) {
    panel
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(8.0),
            ..default()
        })
        .with_children(|root| {
            root.spawn(Text::new("Panel discretization"));

            root.spawn((
                Text::new(panel_count_label(params)),
                PanelCountText,
            ));

            root.spawn((
                slider(
                    SliderProps {
                        value: params.num_points as f32,
                        min: 40.0,
                        max: 400.0,
                    },
                    (SliderStep(10.0), SliderPrecision(0)),
                ),
                observe(slider_self_update),
                observe(
                    |change: On<ValueChange<f32>>,
                     mut p: ResMut<NacaParams>| {
                        let v = change.value.round().clamp(40.0, 400.0);
                        p.num_points = v as usize;
                    },
                ),
            ));

            root.spawn((
                Text::new(
                    "Tip: fewer panels makes polars much faster.",
                ),
                TextColor(Color::srgb(0.70, 0.70, 0.76)),
            ));

            root.spawn((
                Node {
                    width: Val::Percent(100.0),
                    padding: UiRect::axes(Val::Px(8.0), Val::Px(6.0)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.12, 0.12, 0.16)),
                BorderRadius::all(Val::Px(config::BUTTON_RADIUS)),
            ))
            .with_children(|note| {
                note.spawn(Text::new(
                    "Panels view shows discretization only.",
                ));
            });
        });
}

fn panel_count_label(params: &NacaParams) -> String {
    let total_panels =
        build_naca_body_geometry(params).len().saturating_sub(1);
    format!(
        "Points per surface: {}  |  total panels: {}",
        params.num_points, total_panels
    )
}
