mod flow;
mod geometry;
mod numeric;
mod panel_settings;
mod polars;
mod summary;
mod topbar;

use bevy::ecs::hierarchy::ChildSpawnerCommands;
use bevy::feathers::theme::{ThemeBackgroundColor, ThemeBorderColor};
use bevy::feathers::tokens;
use bevy::prelude::*;

use crate::state::{FlowSettings, NacaParams};

use super::types::{
    ExportStatus, LeftPanelMainControls, LeftPanelPanelControls,
    ModePanel, PanelSections, PolarSweepSettings, PolarsControls,
    UiColorThemeMode, UiInputMode, UiRoot, VisualMode,
};

pub fn setup_ui(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    params: Res<NacaParams>,
    flow: Res<FlowSettings>,
    sweep: Res<PolarSweepSettings>,
    mode: Res<VisualMode>,
    sections: Res<PanelSections>,
    input_mode: Res<UiInputMode>,
    theme_mode: Res<UiColorThemeMode>,
    export_status: Res<ExportStatus>,
) {
    let _ = spawn_ui_root(
        &mut commands,
        &asset_server,
        &params,
        &flow,
        &sweep,
        *mode,
        &sections,
        *input_mode,
        *theme_mode,
        &export_status,
    );
}

pub(super) fn spawn_ui_root(
    commands: &mut Commands,
    asset_server: &AssetServer,
    params: &NacaParams,
    flow: &FlowSettings,
    sweep: &PolarSweepSettings,
    mode: VisualMode,
    sections: &PanelSections,
    input_mode: UiInputMode,
    theme_mode: UiColorThemeMode,
    export_status: &ExportStatus,
) -> Entity {
    let mut root_cmd = commands.spawn((
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            ..default()
        },
        UiRoot,
        Name::new("UiRoot"),
    ));
    let root_entity = root_cmd.id();

    root_cmd.with_children(|root| {
        topbar::spawn_top_bar(
            root,
            asset_server,
            params,
            mode,
            input_mode,
            theme_mode,
            &export_status.message,
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
                    ThemeBorderColor(tokens::CHECKBOX_BORDER),
                    BorderRadius::all(Val::Px(14.0)),
                    ThemeBackgroundColor(tokens::WINDOW_BG),
                    ModePanel,
                ))
                .with_children(|panel| {
                    spawn_left_panel(
                        panel,
                        asset_server,
                        params,
                        flow,
                        sweep,
                        mode,
                        sections,
                        theme_mode,
                    );
                });

            summary::spawn_summary_panel(content);
        });
    });

    root_entity
}

fn spawn_left_panel(
    panel: &mut ChildSpawnerCommands<'_>,
    asset_server: &AssetServer,
    params: &NacaParams,
    flow: &FlowSettings,
    sweep: &PolarSweepSettings,
    mode: VisualMode,
    sections: &PanelSections,
    theme_mode: UiColorThemeMode,
) {
    panel
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(14.0),
                ..default()
            },
            LeftPanelMainControls,
            Name::new("LeftPanelMainControls"),
        ))
        .with_children(|main| {
            geometry::spawn_geometry_section(
                main,
                asset_server,
                params,
                sections,
                theme_mode,
            );
            flow::spawn_flow_section(
                main,
                asset_server,
                flow,
                sections,
                theme_mode,
            );

            main.spawn((
                Node {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(10.0),
                    display: if mode == VisualMode::Polars {
                        Display::Flex
                    } else {
                        Display::None
                    },
                    ..default()
                },
                PolarsControls,
                Name::new("PolarsControls"),
            ))
            .with_children(|polars_controls| {
                polars::spawn_polars_section(
                    polars_controls,
                    asset_server,
                    sweep,
                    sections,
                    theme_mode,
                );
            });
        });

    panel
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(14.0),
                display: Display::None,
                ..default()
            },
            LeftPanelPanelControls,
            Name::new("LeftPanelPanelControls"),
        ))
        .with_children(|panel_controls| {
            panel_settings::spawn_panel_settings(
                panel_controls,
                params,
            );
        });
}
