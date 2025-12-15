mod config;
mod feathers_theme;
mod layout;
mod style;
mod systems;
mod types;

pub use feathers_theme::theme_props_for;
pub use layout::setup_ui;
pub use systems::{
    handle_coeff_mode_buttons, handle_export_polars_button,
    handle_flow_toggle_buttons, handle_input_mode_buttons,
    handle_numeric_input_edit, handle_numeric_input_focus,
    handle_section_toggle_buttons, handle_theme_toggle_button,
    handle_view_buttons, normalize_polar_sweep_settings,
    set_initial_ui_scale, slim_sliders, sync_numeric_inputs,
    update_coeff_mode_button_styles, update_export_status_text,
    update_fallback_warning_badge, update_input_mode_button_styles,
    update_left_panel_visibility, update_naca_heading,
    update_numeric_input_visibility, update_panel_count_text,
    update_table_text, update_theme_toggle_button,
    update_ui_scale_on_resize,
};
pub use types::{
    ExportStatus, NumericInputFocus, PanelSections, PolarSweepSettings,
    SolverDiagnostics, TableField, UiCoeffMode, UiColorThemeMode,
    UiInputMode, VisualMode,
};
