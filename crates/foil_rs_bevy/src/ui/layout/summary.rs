use bevy::{
    ecs::hierarchy::ChildSpawnerCommands,
    feathers::{
        theme::{ThemeBackgroundColor, ThemeBorderColor},
        tokens,
    },
    prelude::*,
};

use crate::ui::types::TableField;

pub(super) fn spawn_summary_panel(root: &mut ChildSpawnerCommands<'_>) {
    root.spawn((
        Node {
            position_type: PositionType::Absolute,
            right: Val::Px(24.0),
            top: Val::Px(24.0),
            width: Val::Px(300.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(10.0),
            padding: UiRect::all(Val::Px(12.0)),
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
        ThemeBorderColor(tokens::CHECKBOX_BORDER),
        BorderRadius::all(Val::Px(12.0)),
        ThemeBackgroundColor(tokens::WINDOW_BG),
    ))
    .with_children(|panel| {
        panel.spawn(Text::new("Summary"));
        panel
            .spawn((
                Node {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(4.0),
                    padding: UiRect::all(Val::Px(10.0)),
                    ..default()
                },
                ThemeBackgroundColor(tokens::BUTTON_BG),
                BorderRadius::all(Val::Px(10.0)),
            ))
            .with_children(spawn_summary_table);
    });
}

fn spawn_summary_table(table: &mut ChildSpawnerCommands<'_>) {
    let mut row = |label: &str, field: TableField| {
        table
            .spawn(Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                ..default()
            })
            .with_children(|row| {
                row.spawn(Text::new(label.to_string()));
                row.spawn((Text::new("--".to_string()), field));
            });
    };

    row("NACA code", TableField::NacaCode);
    row("α (deg)", TableField::AlphaDeg);
    row("Mach", TableField::Mach);
    row("Re (×10⁶)", TableField::Reynolds);
    row("Viscosity", TableField::ViscosityMode);
    row("Transition", TableField::TransitionMode);
    row("CL (thin)", TableField::ClThin);
    row("CL (est.)", TableField::RefCl);
    row("Cm (est.)", TableField::RefCm);
    row("CDp (est.)", TableField::RefCdp);
    row("Flow state", TableField::FlowState);
}
