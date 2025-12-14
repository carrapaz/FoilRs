use bevy::{
    ecs::hierarchy::ChildSpawnerCommands,
    feathers::constants::fonts,
    prelude::*,
    text::{TextColor, TextFont},
};

use crate::state::{NacaParams, VisualMode};

use super::super::style;
use super::super::types::{
    ExportPolarsButton, ExportStatusText, InputModeButton, NacaHeading,
    ThemeToggleButton, TopBar, UiColorThemeMode, UiInputMode,
    ViewButton,
};

const TOP_BAR_HEIGHT: f32 = 56.0;

pub(super) fn spawn_top_bar(
    root: &mut ChildSpawnerCommands<'_>,
    asset_server: &AssetServer,
    params: &NacaParams,
    mode: VisualMode,
    input_mode: UiInputMode,
    theme_mode: UiColorThemeMode,
    export_status: &str,
) {
    root.spawn((
        Node {
            width: Val::Percent(100.0),
            height: Val::Px(TOP_BAR_HEIGHT),
            padding: UiRect::axes(Val::Px(18.0), Val::Px(12.0)),
            border: UiRect::bottom(Val::Px(1.0)),
            flex_direction: FlexDirection::Row,
            justify_content: JustifyContent::SpaceBetween,
            align_items: AlignItems::Center,
            ..default()
        },
        BorderColor::all(Color::srgb(0.18, 0.18, 0.22)),
        BackgroundColor(style::top_bar_color(mode, theme_mode)),
        TopBar,
    ))
    .with_children(|bar| {
        let title_font = asset_server.load(fonts::BOLD);
        let ui_font = asset_server.load(fonts::REGULAR);
        let mono_font = asset_server.load(fonts::MONO);

        bar.spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Baseline,
            column_gap: Val::Px(12.0),
            ..default()
        })
        .with_children(|left| {
            left.spawn((
                Text::new("FoilRs"),
                TextFont {
                    font: title_font.clone(),
                    font_size: 18.0,
                    ..default()
                },
                TextColor(Color::srgb(0.92, 0.92, 0.96)),
            ));

            left.spawn((
                Text::new(params.code()),
                NacaHeading,
                TextFont {
                    font: mono_font,
                    font_size: 14.0,
                    ..default()
                },
                TextColor(Color::srgb(0.72, 0.72, 0.78)),
            ));
        });

        bar.spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(12.0),
            ..default()
        })
        .with_children(|tabs| {
            tabs.spawn((
                Text::new("View"),
                TextFont {
                    font: ui_font.clone(),
                    font_size: 13.0,
                    ..default()
                },
                TextColor(Color::srgb(0.70, 0.70, 0.76)),
            ));

            for &view in &[
                VisualMode::Field,
                VisualMode::Cp,
                VisualMode::Polars,
                VisualMode::Panels,
            ] {
                tabs.spawn((
                    Node {
                        padding: UiRect::axes(
                            Val::Px(12.0),
                            Val::Px(8.0),
                        ),
                        border: UiRect::all(Val::Px(1.0)),
                        ..default()
                    },
                    BorderColor::all(Color::srgb(0.25, 0.25, 0.32)),
                    BorderRadius::all(Val::Px(999.0)),
                    BackgroundColor(style::view_button_color(
                        mode, view, theme_mode,
                    )),
                    Button,
                    ViewButton { mode: view },
                ))
                .with_children(|btn| {
                    btn.spawn((
                        Text::new(
                            style::view_button_label(view).to_string(),
                        ),
                        TextFont {
                            font: ui_font.clone(),
                            font_size: 13.0,
                            ..default()
                        },
                        TextColor(Color::srgb(0.86, 0.86, 0.92)),
                    ));
                });
            }

            tabs.spawn((
                Text::new("Inputs"),
                TextFont {
                    font: ui_font.clone(),
                    font_size: 13.0,
                    ..default()
                },
                TextColor(Color::srgb(0.70, 0.70, 0.76)),
            ));

            for &mode_option in
                &[UiInputMode::SliderOnly, UiInputMode::TypeOnly]
            {
                let label = match mode_option {
                    UiInputMode::SliderOnly => "Slider",
                    UiInputMode::TypeOnly => "Type",
                };

                tabs.spawn((
                    Node {
                        padding: UiRect::axes(
                            Val::Px(12.0),
                            Val::Px(8.0),
                        ),
                        border: UiRect::all(Val::Px(1.0)),
                        ..default()
                    },
                    BorderColor::all(Color::srgb(0.25, 0.25, 0.32)),
                    BorderRadius::all(Val::Px(999.0)),
                    BackgroundColor(style::input_mode_button_color(
                        mode_option == input_mode,
                        theme_mode,
                    )),
                    Button,
                    InputModeButton { mode: mode_option },
                ))
                .with_children(|btn| {
                    btn.spawn((
                        Text::new(label.to_string()),
                        TextFont {
                            font: ui_font.clone(),
                            font_size: 13.0,
                            ..default()
                        },
                        TextColor(Color::srgb(0.86, 0.86, 0.92)),
                    ));
                });
            }
        });

        bar.spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(10.0),
            ..default()
        })
        .with_children(|right| {
            right
                .spawn((
                    Node {
                        padding: UiRect::axes(
                            Val::Px(12.0),
                            Val::Px(8.0),
                        ),
                        border: UiRect::all(Val::Px(1.0)),
                        ..default()
                    },
                    BorderColor::all(Color::srgb(0.25, 0.25, 0.32)),
                    BorderRadius::all(Val::Px(999.0)),
                    BackgroundColor(style::top_right_button_color(
                        theme_mode == UiColorThemeMode::XFoilMono,
                        theme_mode,
                    )),
                    Button,
                    ThemeToggleButton,
                ))
                .with_children(|btn| {
                    btn.spawn((
                        Text::new(theme_mode.label().to_string()),
                        TextFont {
                            font: ui_font.clone(),
                            font_size: 13.0,
                            ..default()
                        },
                        TextColor(Color::srgb(0.86, 0.86, 0.92)),
                    ));
                });

            right
                .spawn((
                    Node {
                        padding: UiRect::axes(
                            Val::Px(12.0),
                            Val::Px(8.0),
                        ),
                        border: UiRect::all(Val::Px(1.0)),
                        ..default()
                    },
                    BorderColor::all(Color::srgb(0.25, 0.25, 0.32)),
                    BorderRadius::all(Val::Px(999.0)),
                    BackgroundColor(style::top_right_button_color(
                        false, theme_mode,
                    )),
                    Button,
                    ExportPolarsButton,
                ))
                .with_children(|btn| {
                    btn.spawn((
                        Text::new("Export CSV".to_string()),
                        TextFont {
                            font: ui_font.clone(),
                            font_size: 13.0,
                            ..default()
                        },
                        TextColor(Color::srgb(0.86, 0.86, 0.92)),
                    ));
                });

            let _ = right.spawn((
                Text::new(export_status.to_string()),
                ExportStatusText,
                TextFont {
                    font: ui_font.clone(),
                    font_size: 12.0,
                    ..default()
                },
                TextColor(Color::srgb(0.65, 0.65, 0.72)),
            ));
        });
    });
}
