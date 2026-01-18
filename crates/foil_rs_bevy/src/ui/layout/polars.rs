use bevy::{
    ecs::hierarchy::ChildSpawnerCommands,
    feathers::controls::{SliderProps, slider},
    feathers::{
        theme::{ThemeBackgroundColor, ThemeFontColor, ThemedText},
        tokens,
    },
    prelude::*,
    ui_widgets::{
        SliderPrecision, SliderStep, ValueChange, observe,
        slider_self_update,
    },
};

use super::super::config;
use super::super::types::{
    InputSlider, PanelSection, PanelSections, SectionContent,
    SectionToggle,
};
use super::super::types::{NumericField, PolarSweepSettings};
use super::numeric::spawn_numeric_input;

pub(super) fn spawn_polars_section(
    panel: &mut ChildSpawnerCommands<'_>,
    asset_server: &AssetServer,
    sweep: &PolarSweepSettings,
    sections: &PanelSections,
    theme_mode: super::super::types::UiColorThemeMode,
) {
    let open = sections.is_open(PanelSection::Polars);
    panel
        .spawn((
            Node {
                width: Val::Percent(100.0),
                padding: UiRect::axes(Val::Px(6.0), Val::Px(4.0)),
                justify_content: JustifyContent::FlexStart,
                align_items: AlignItems::Center,
                border_radius: BorderRadius::all(Val::Px(
                    config::BUTTON_RADIUS,
                )),
                ..default()
            },
            ThemeBackgroundColor(if open {
                tokens::BUTTON_BG_HOVER
            } else {
                tokens::BUTTON_BG
            }),
            ThemeFontColor(tokens::TEXT_MAIN),
            Button,
            SectionToggle {
                section: PanelSection::Polars,
            },
        ))
        .with_children(|btn| {
            btn.spawn((
                Text::new(super::super::style::section_header_label(
                    PanelSection::Polars,
                    open,
                )),
                ThemedText,
            ));
        });

    panel
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(6.0),
                display: if open { Display::Flex } else { Display::None },
                ..default()
            },
            SectionContent { section: PanelSection::Polars },
        ))
        .with_children(|pol| {
            let max_threads = std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(1)
                .min(32) as f32;

            pol.spawn(Text::new("α min (deg)"));
            pol.spawn((
                slider(
                    SliderProps {
                        value: sweep.alpha_min_deg,
                        min: -30.0,
                        max: 30.0,
                    },
                    (SliderStep(0.5), SliderPrecision(1)),
                ),
                InputSlider,
                observe(slider_self_update),
                observe(|change: On<ValueChange<f32>>, mut s: ResMut<PolarSweepSettings>| {
                    s.alpha_min_deg = change.value.clamp(-30.0, 30.0);
                }),
            ));
            spawn_numeric_input(
                pol,
                asset_server,
                theme_mode,
                NumericField::PolarAlphaMinDeg,
                format!("{:.1}", sweep.alpha_min_deg),
                -30.0,
                30.0,
                1,
                false,
            );

            pol.spawn(Text::new("α max (deg)"));
            pol.spawn((
                slider(
                    SliderProps {
                        value: sweep.alpha_max_deg,
                        min: -30.0,
                        max: 30.0,
                    },
                    (SliderStep(0.5), SliderPrecision(1)),
                ),
                InputSlider,
                observe(slider_self_update),
                observe(|change: On<ValueChange<f32>>, mut s: ResMut<PolarSweepSettings>| {
                    s.alpha_max_deg = change.value.clamp(-30.0, 30.0);
                }),
            ));
            spawn_numeric_input(
                pol,
                asset_server,
                theme_mode,
                NumericField::PolarAlphaMaxDeg,
                format!("{:.1}", sweep.alpha_max_deg),
                -30.0,
                30.0,
                1,
                false,
            );

            pol.spawn(Text::new("α step (deg)"));
            pol.spawn((
                slider(
                    SliderProps {
                        value: sweep.alpha_step_deg,
                        min: 0.1,
                        max: 5.0,
                    },
                    (SliderStep(0.1), SliderPrecision(1)),
                ),
                InputSlider,
                observe(slider_self_update),
                observe(|change: On<ValueChange<f32>>, mut s: ResMut<PolarSweepSettings>| {
                    s.alpha_step_deg = change.value.clamp(0.1, 5.0);
                }),
            ));
            spawn_numeric_input(
                pol,
                asset_server,
                theme_mode,
                NumericField::PolarAlphaStepDeg,
                format!("{:.1}", sweep.alpha_step_deg),
                0.1,
                5.0,
                1,
                false,
            );

            pol.spawn(Text::new("Threads (0 = auto)"));
            pol.spawn((
                slider(
                    SliderProps {
                        value: sweep.threads as f32,
                        min: 0.0,
                        max: max_threads,
                    },
                    (SliderStep(1.0), SliderPrecision(0)),
                ),
                InputSlider,
                observe(slider_self_update),
                observe(|change: On<ValueChange<f32>>, mut s: ResMut<PolarSweepSettings>| {
                    let v = change.value.round().clamp(0.0, 32.0) as u8;
                    s.threads = v;
                }),
            ));
            spawn_numeric_input(
                pol,
                asset_server,
                theme_mode,
                NumericField::PolarThreads,
                format!("{}", sweep.threads),
                0.0,
                32.0,
                0,
                true,
            );
        });
}
