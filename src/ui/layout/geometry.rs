use bevy::{
    ecs::hierarchy::ChildSpawnerCommands,
    feathers::controls::{SliderProps, slider},
    prelude::*,
    ui_widgets::{
        SliderPrecision, SliderStep, ValueChange, observe,
        slider_self_update,
    },
};

use crate::state::NacaParams;

use super::super::types::NumericField;
use super::super::types::{
    InputSlider, PanelSection, PanelSections, SectionContent,
    SectionToggle,
};
use super::super::{config, style};
use super::numeric::spawn_numeric_input;

pub(super) fn spawn_geometry_section(
    panel: &mut ChildSpawnerCommands<'_>,
    asset_server: &AssetServer,
    params: &NacaParams,
    sections: &PanelSections,
    theme_mode: super::super::types::UiColorThemeMode,
) {
    let geometry_open = sections.is_open(PanelSection::Geometry);
    panel
        .spawn((
            Node {
                width: Val::Percent(100.0),
                padding: UiRect::axes(Val::Px(6.0), Val::Px(4.0)),
                justify_content: JustifyContent::FlexStart,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(style::section_header_color(
                geometry_open,
                theme_mode,
            )),
            BorderRadius::all(Val::Px(config::BUTTON_RADIUS)),
            Button,
            SectionToggle {
                section: PanelSection::Geometry,
            },
        ))
        .with_children(|btn| {
            btn.spawn(Text::new(style::section_header_label(
                PanelSection::Geometry,
                geometry_open,
            )));
        });

    panel
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(6.0),
                display: if geometry_open {
                    Display::Flex
                } else {
                    Display::None
                },
                ..default()
            },
            SectionContent {
                section: PanelSection::Geometry,
            },
        ))
        .with_children(|geo| {
            geo.spawn(Text::new("Max camber m (%)"));
            geo.spawn((
                slider(
                    SliderProps {
                        value: params.m_digit,
                        min: 0.0,
                        max: 9.0,
                    },
                    (SliderStep(1.0), SliderPrecision(0)),
                ),
                InputSlider,
                observe(slider_self_update),
                observe(
                    |change: On<ValueChange<f32>>,
                     mut p: ResMut<NacaParams>| {
                        p.m_digit =
                            change.value.round().clamp(0.0, 9.0);
                    },
                ),
            ));
            spawn_numeric_input(
                geo,
                asset_server,
                theme_mode,
                NumericField::NacaMDigit,
                format!("{:.0}", params.m_digit),
                0.0,
                9.0,
                0,
                true,
            );

            geo.spawn(Text::new("Camber position p (x/c * 10)"));
            geo.spawn((
                slider(
                    SliderProps {
                        value: params.p_digit,
                        min: 0.0,
                        max: 9.0,
                    },
                    (SliderStep(1.0), SliderPrecision(0)),
                ),
                InputSlider,
                observe(slider_self_update),
                observe(
                    |change: On<ValueChange<f32>>,
                     mut p: ResMut<NacaParams>| {
                        p.p_digit =
                            change.value.round().clamp(0.0, 9.0);
                    },
                ),
            ));
            spawn_numeric_input(
                geo,
                asset_server,
                theme_mode,
                NumericField::NacaPDigit,
                format!("{:.0}", params.p_digit),
                0.0,
                9.0,
                0,
                true,
            );

            geo.spawn(Text::new("Thickness t (%)"));
            geo.spawn((
                slider(
                    SliderProps {
                        value: params.t_digits,
                        min: 1.0,
                        max: 40.0,
                    },
                    (SliderStep(1.0), SliderPrecision(0)),
                ),
                InputSlider,
                observe(slider_self_update),
                observe(
                    |change: On<ValueChange<f32>>,
                     mut p: ResMut<NacaParams>| {
                        p.t_digits =
                            change.value.round().clamp(1.0, 40.0);
                    },
                ),
            ));
            spawn_numeric_input(
                geo,
                asset_server,
                theme_mode,
                NumericField::NacaTDigits,
                format!("{:.0}", params.t_digits),
                1.0,
                40.0,
                0,
                true,
            );
        });
}
