use bevy::feathers::{
    FeathersPlugins, dark_theme::create_dark_theme, theme::UiTheme,
};
use bevy::prelude::*;
use bevy::window::{Window, WindowPlugin, WindowResolution};

use foil_rs::{plotter, state, ui, views};
use state::{FlowSettings, NacaParams, VisualMode};

fn main() {
    App::new()
        // Black background, like XFoil.
        .insert_resource(ClearColor(Color::BLACK))
        // Feathers dark theme.
        .insert_resource(UiTheme(create_dark_theme()))
        // State
        .insert_resource(NacaParams::default())
        .insert_resource(FlowSettings::default())
        .insert_resource(VisualMode::Field)
        .init_resource::<ui::UiInputMode>()
        .insert_resource(ui::PanelSections::default())
        .init_resource::<ui::NumericInputFocus>()
        .init_resource::<ui::ExportStatus>()
        .init_resource::<plotter::CpPlotLabels>()
        .init_resource::<plotter::PolarPlotLabels>()
        // Plugins
        .add_plugins((
            DefaultPlugins.set(WindowPlugin {
                primary_window: Some(Window {
                    // Pick a nice default size
                    resolution: WindowResolution::new(1920, 1080)
                        // Important: keep logical pixels = physical pixels
                        .with_scale_factor_override(1.0),
                    ..default()
                }),
                ..default()
            }),
            FeathersPlugins,
        )) // Startup
        .add_systems(
            Startup,
            (setup_camera, ui::set_initial_ui_scale, ui::setup_ui),
        )
        // Update – ONLY systems, no slider_self_update here.
        .add_systems(
            Update,
            (
                ui::update_ui_scale_on_resize,
                views::draw_airfoil_and_visualization,
                ui::update_table_text,
                ui::update_naca_heading,
                ui::update_top_bar_tint,
                ui::update_mode_panel_tint,
                ui::update_left_panel_visibility,
                ui::update_panel_count_text,
                ui::handle_export_polars_button,
                ui::update_export_status_text,
                ui::handle_view_buttons,
                ui::handle_section_toggle_buttons,
                ui::handle_flow_toggle_buttons,
                ui::handle_input_mode_buttons,
                ui::update_input_mode_button_styles,
                ui::update_numeric_input_visibility,
                ui::handle_numeric_input_focus,
                ui::handle_numeric_input_edit,
                ui::sync_numeric_inputs,
            ),
        )
        // PostUpdate so our size tweaks win after any theme updates.
        .add_systems(PostUpdate, ui::slim_sliders)
        .run();
}

fn setup_camera(mut commands: Commands) {
    commands.spawn(Camera2d);
}
