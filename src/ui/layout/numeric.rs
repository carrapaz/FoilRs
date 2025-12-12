use bevy::{
    ecs::hierarchy::ChildSpawnerCommands,
    feathers::constants::fonts,
    prelude::*,
    text::{TextColor, TextFont},
};

use super::super::types::{
    NumericField, NumericInput, NumericInputRow, NumericInputText,
};
use super::super::{config, style};

pub(super) fn spawn_numeric_input(
    parent: &mut ChildSpawnerCommands<'_>,
    asset_server: &AssetServer,
    field: NumericField,
    initial_text: String,
    min: f32,
    max: f32,
    precision: u8,
    integer: bool,
) {
    let font = asset_server.load(fonts::MONO);

    parent
        .spawn((
            Node {
                width: Val::Percent(100.0),
                justify_content: JustifyContent::FlexEnd,
                display: Display::None,
                ..default()
            },
            NumericInputRow,
            Name::new("NumericInputRow"),
        ))
        .with_children(|row| {
            let mut input = row.spawn((
                Node {
                    width: Val::Px(92.0),
                    padding: UiRect::axes(Val::Px(10.0), Val::Px(6.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                BorderColor::all(style::input_border(false)),
                BorderRadius::all(Val::Px(config::BUTTON_RADIUS)),
                BackgroundColor(style::input_bg(false)),
                Button,
                NumericInput {
                    field,
                    min,
                    max,
                    precision,
                    integer,
                },
            ));
            let owner = input.id();
            input.with_children(|box_node| {
                box_node.spawn((
                    Text::new(initial_text),
                    NumericInputText { owner },
                    TextFont {
                        font,
                        font_size: 13.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.88, 0.88, 0.92)),
                ));
            });
        });
}
