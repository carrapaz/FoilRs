use bevy::{
    ecs::hierarchy::ChildSpawnerCommands,
    feathers::{
        constants::fonts,
        theme::{
            ThemeBackgroundColor, ThemeBorderColor, ThemeFontColor,
            ThemedText,
        },
        tokens,
    },
    prelude::*,
    text::TextFont,
};

use crate::state::NacaParams;

use super::super::types::{
    CoeffModeButton, ExportPolarsButton, ExportStatusText,
    FallbackWarningBadge, FallbackWarningText, InputModeButton,
    NacaHeading, ThemeToggleButton, TopBar, UiCoeffMode,
    UiColorThemeMode, UiInputMode, ViewButton, VisualMode,
};

const TOP_BAR_HEIGHT: f32 = 56.0;

pub(super) fn spawn_top_bar(
    root: &mut ChildSpawnerCommands<'_>,
    asset_server: &AssetServer,
    params: &NacaParams,
    mode: VisualMode,
    input_mode: UiInputMode,
    coeff_mode: UiCoeffMode,
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
        ThemeBorderColor(tokens::CHECKBOX_BORDER),
        ThemeBackgroundColor(tokens::WINDOW_BG),
        TopBar,
    ))
    .with_children(|bar| {
        let title_font = asset_server.load(fonts::BOLD);
        let ui_font = asset_server.load(fonts::REGULAR);
        let mono_font = asset_server.load(fonts::MONO);

        bar.spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Baseline,
                column_gap: Val::Px(12.0),
                ..default()
            },
            ThemeFontColor(tokens::TEXT_MAIN),
        ))
        .with_children(|left| {
            left.spawn((
                Text::new("FoilRs"),
                TextFont {
                    font: title_font.clone(),
                    font_size: 18.0,
                    ..default()
                },
                ThemedText,
            ));

            left.spawn((
                Node::default(),
                ThemeFontColor(tokens::TEXT_DIM),
            ))
            .with_children(|dim| {
                dim.spawn((
                    Text::new(params.code()),
                    NacaHeading,
                    TextFont {
                        font: mono_font,
                        font_size: 14.0,
                        ..default()
                    },
                    ThemedText,
                ));
            });
        });

        bar.spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(12.0),
            ..default()
        })
        .with_children(|tabs| {
            spawn_themed_text(
                tabs,
                "View",
                ui_font.clone(),
                13.0,
                tokens::TEXT_DIM,
                (),
            );

            for &view in &[
                VisualMode::Field,
                VisualMode::Cp,
                VisualMode::Polars,
                VisualMode::Panels,
            ] {
                spawn_pill_button(
                    tabs,
                    view.label(),
                    ui_font.clone(),
                    mode == view,
                    ViewButton { mode: view },
                );
            }

            spawn_themed_text(
                tabs,
                "Inputs",
                ui_font.clone(),
                13.0,
                tokens::TEXT_DIM,
                (),
            );

            for &mode_option in
                &[UiInputMode::SliderOnly, UiInputMode::TypeOnly]
            {
                let label = match mode_option {
                    UiInputMode::SliderOnly => "Slider",
                    UiInputMode::TypeOnly => "Type",
                };
                spawn_pill_button(
                    tabs,
                    label,
                    ui_font.clone(),
                    mode_option == input_mode,
                    InputModeButton { mode: mode_option },
                );
            }

            spawn_themed_text(
                tabs,
                "Coeffs",
                ui_font.clone(),
                13.0,
                tokens::TEXT_DIM,
                (),
            );
            for &coeff_option in
                &[UiCoeffMode::Panel, UiCoeffMode::Approx]
            {
                spawn_pill_button(
                    tabs,
                    coeff_option.label(),
                    ui_font.clone(),
                    coeff_option == coeff_mode,
                    CoeffModeButton { mode: coeff_option },
                );
            }

            tabs.spawn((
                Node {
                    padding: UiRect::axes(Val::Px(6.0), Val::Px(0.0)),
                    display: Display::None,
                    ..default()
                },
                FallbackWarningBadge,
            ))
            .with_children(|warn| {
                warn.spawn((
                    Text::new("fallback"),
                    TextFont {
                        font: ui_font.clone(),
                        font_size: 12.0,
                        ..default()
                    },
                    TextColor(Color::srgb(1.0, 0.7, 0.3)),
                    FallbackWarningText,
                ));
            });
        });

        bar.spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(10.0),
            ..default()
        })
        .with_children(|right| {
            let mono_active = theme_mode == UiColorThemeMode::XFoilMono;
            spawn_pill_button(
                right,
                theme_mode.label(),
                ui_font.clone(),
                mono_active,
                ThemeToggleButton,
            );

            spawn_pill_button(
                right,
                "Export CSV",
                ui_font.clone(),
                false,
                ExportPolarsButton,
            );

            spawn_themed_text(
                right,
                export_status,
                ui_font.clone(),
                12.0,
                tokens::TEXT_DIM,
                ExportStatusText,
            );
        });
    });
}

fn spawn_pill_button<B: Bundle>(
    parent: &mut ChildSpawnerCommands<'_>,
    label: impl Into<String>,
    font: Handle<Font>,
    selected: bool,
    extra: B,
) {
    let (bg, fg) = if selected {
        (tokens::BUTTON_PRIMARY_BG, tokens::BUTTON_PRIMARY_TEXT)
    } else {
        (tokens::BUTTON_BG, tokens::BUTTON_TEXT)
    };

    parent
        .spawn((
            Node {
                padding: UiRect::axes(Val::Px(12.0), Val::Px(8.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(999.0)),
                ..default()
            },
            ThemeBorderColor(tokens::CHECKBOX_BORDER),
            ThemeBackgroundColor(bg),
            ThemeFontColor(fg),
            Button,
            extra,
        ))
        .with_children(|btn| {
            btn.spawn((
                Text::new(label.into()),
                TextFont {
                    font,
                    font_size: 13.0,
                    ..default()
                },
                ThemedText,
            ));
        });
}

fn spawn_themed_text<B: Bundle>(
    parent: &mut ChildSpawnerCommands<'_>,
    text: impl Into<String>,
    font: Handle<Font>,
    font_size: f32,
    color: bevy::feathers::theme::ThemeToken,
    extra: B,
) {
    parent
        .spawn((Node::default(), ThemeFontColor(color)))
        .with_children(|node| {
            node.spawn((
                Text::new(text.into()),
                TextFont {
                    font,
                    font_size,
                    ..default()
                },
                ThemedText,
                extra,
            ));
        });
}
