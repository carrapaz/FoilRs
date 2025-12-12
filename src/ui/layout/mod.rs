mod flow;
mod geometry;
mod numeric;
mod summary;
mod topbar;

use bevy::ecs::hierarchy::ChildSpawnerCommands;
use bevy::prelude::*;

use crate::state::{FlowSettings, NacaParams, VisualMode};

use super::style;
use super::types::{ModePanel, PanelSections, UiInputMode};

pub fn setup_ui(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    params: Res<NacaParams>,
    flow: Res<FlowSettings>,
    mode: Res<VisualMode>,
    sections: Res<PanelSections>,
    input_mode: Res<UiInputMode>,
) {
    commands
        .spawn(Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            ..default()
        })
        .with_children(|root| {
            topbar::spawn_top_bar(
                root,
                &asset_server,
                &params,
                *mode,
                *input_mode,
            );

            root.spawn(Node {
                width: Val::Percent(100.0),
                flex_grow: 1.0,
                flex_direction: FlexDirection::Row,
                ..default()
            })
            .with_children(|content| {
                content
                    .spawn((
                        Node {
                            width: Val::Percent(28.0),
                            min_width: Val::Px(180.0),
                            max_width: Val::Px(340.0),
                            height: Val::Percent(100.0),
                            flex_direction: FlexDirection::Column,
                            padding: UiRect::axes(
                                Val::Px(18.0),
                                Val::Px(18.0),
                            ),
                            row_gap: Val::Px(14.0),
                            border: UiRect::all(Val::Px(1.0)),
                            ..default()
                        },
                        BorderColor::all(Color::srgb(0.22, 0.22, 0.28)),
                        BorderRadius::all(Val::Px(14.0)),
                        BackgroundColor(style::panel_base_color(*mode)),
                        ModePanel,
                    ))
                    .with_children(|panel| {
                        spawn_left_panel(
                            panel,
                            &asset_server,
                            &params,
                            &flow,
                            &sections,
                        );
                    });

                summary::spawn_summary_panel(content);
            });
        });
}

fn spawn_left_panel(
    panel: &mut ChildSpawnerCommands<'_>,
    asset_server: &AssetServer,
    params: &NacaParams,
    flow: &FlowSettings,
    sections: &PanelSections,
) {
    geometry::spawn_geometry_section(
        panel,
        asset_server,
        params,
        sections,
    );
    flow::spawn_flow_section(panel, asset_server, flow, sections);
}
