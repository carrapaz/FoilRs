use bevy::{
    ecs::hierarchy::ChildSpawnerCommands,
    feathers::controls::{SliderProps, slider},
    prelude::*,
    ui_widgets::{
        SliderPrecision, SliderStep, ValueChange, observe,
        slider_self_update,
    },
};

use crate::state::FlowSettings;

use super::super::types::NumericField;
use super::super::types::{
    FlowToggleKind, InputSlider, PanelSection, PanelSections,
    SectionContent, SectionToggle,
};
use super::super::{config, style};
use super::numeric::spawn_numeric_input;

pub(super) fn spawn_flow_section(
    panel: &mut ChildSpawnerCommands<'_>,
    asset_server: &AssetServer,
    flow: &FlowSettings,
    sections: &PanelSections,
    theme_mode: super::super::types::UiColorThemeMode,
) {
    let flow_open = sections.is_open(PanelSection::Flow);
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
                flow_open, theme_mode,
            )),
            BorderRadius::all(Val::Px(config::BUTTON_RADIUS)),
            Button,
            SectionToggle {
                section: PanelSection::Flow,
            },
        ))
        .with_children(|btn| {
            btn.spawn(Text::new(style::section_header_label(
                PanelSection::Flow,
                flow_open,
            )));
        });

    panel
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(6.0),
                display: if flow_open {
                    Display::Flex
                } else {
                    Display::None
                },
                ..default()
            },
            SectionContent {
                section: PanelSection::Flow,
            },
        ))
        .with_children(|flow_panel| {
            flow_panel.spawn(Text::new("Angle of attack α (deg)"));
            flow_panel.spawn((
                slider(
                    SliderProps {
                        value: flow.alpha_deg,
                        min: -10.0,
                        max: 15.0,
                    },
                    (SliderStep(0.5), SliderPrecision(1)),
                ),
                InputSlider,
                observe(slider_self_update),
                observe(
                    |change: On<ValueChange<f32>>,
                     mut f: ResMut<FlowSettings>| {
                        f.alpha_deg = change.value;
                    },
                ),
            ));
            spawn_numeric_input(
                flow_panel,
                asset_server,
                theme_mode,
                NumericField::AlphaDeg,
                format!("{:.2}", flow.alpha_deg),
                -10.0,
                15.0,
                2,
                false,
            );

            flow_panel.spawn(Text::new("Reynolds (×10⁶)"));
            flow_panel.spawn((
                slider(
                    SliderProps {
                        value: flow.reynolds / 1_000_000.0,
                        min: 0.1,
                        max: 10.0,
                    },
                    (SliderStep(0.05), SliderPrecision(2)),
                ),
                InputSlider,
                observe(slider_self_update),
                observe(
                    |change: On<ValueChange<f32>>,
                     mut f: ResMut<FlowSettings>| {
                        let millions = change.value.clamp(0.1, 10.0);
                        f.reynolds = millions * 1_000_000.0;
                    },
                ),
            ));
            spawn_numeric_input(
                flow_panel,
                asset_server,
                theme_mode,
                NumericField::ReynoldsMillions,
                format!("{:.2}", flow.reynolds / 1_000_000.0),
                0.1,
                10.0,
                2,
                false,
            );

            flow_panel.spawn(Text::new("Mach number"));
            flow_panel.spawn((
                slider(
                    SliderProps {
                        value: flow.mach,
                        min: 0.0,
                        max: 0.85,
                    },
                    (SliderStep(0.01), SliderPrecision(2)),
                ),
                InputSlider,
                observe(slider_self_update),
                observe(
                    |change: On<ValueChange<f32>>,
                     mut f: ResMut<FlowSettings>| {
                        f.mach = change.value.clamp(0.0, 0.85);
                    },
                ),
            ));
            spawn_numeric_input(
                flow_panel,
                asset_server,
                theme_mode,
                NumericField::Mach,
                format!("{:.2}", flow.mach),
                0.0,
                0.85,
                2,
                false,
            );

            flow_panel.spawn(Text::new("Viscosity model"));
            flow_panel
                .spawn((
                    Node {
                        width: Val::Percent(100.0),
                        padding: UiRect::axes(
                            Val::Px(8.0),
                            Val::Px(6.0),
                        ),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    BackgroundColor(style::toggle_color(
                        flow.viscous,
                        theme_mode,
                    )),
                    BorderRadius::all(Val::Px(config::BUTTON_RADIUS)),
                    Button,
                    FlowToggleKind::Viscosity,
                ))
                .with_children(|btn| {
                    btn.spawn(Text::new(
                        style::flow_toggle_label(
                            FlowToggleKind::Viscosity,
                            flow.viscous,
                        )
                        .to_string(),
                    ));
                });

            flow_panel.spawn(Text::new("Transition model"));
            flow_panel
                .spawn((
                    Node {
                        width: Val::Percent(100.0),
                        padding: UiRect::axes(
                            Val::Px(8.0),
                            Val::Px(6.0),
                        ),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    BackgroundColor(style::toggle_color(
                        flow.free_transition,
                        theme_mode,
                    )),
                    BorderRadius::all(Val::Px(config::BUTTON_RADIUS)),
                    Button,
                    FlowToggleKind::Transition,
                ))
                .with_children(|btn| {
                    btn.spawn(Text::new(
                        style::flow_toggle_label(
                            FlowToggleKind::Transition,
                            flow.free_transition,
                        )
                        .to_string(),
                    ));
                });
        });
}
