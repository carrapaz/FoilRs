use bevy::prelude::MessageReader;
use bevy::{
    feathers::theme::{
        ThemeBackgroundColor, ThemeBorderColor, ThemeFontColor,
    },
    feathers::tokens,
    input::{ButtonInput, keyboard::KeyCode},
    log::{info, warn},
    prelude::*,
    ui_widgets::Slider,
    window::{PrimaryWindow, WindowResized},
};

use crate::solvers::panel::PanelLuSystem;
use crate::solvers::{
    BoundaryLayerInputs, compute_panel_solution,
    estimate_boundary_layer,
};
use crate::state::{FlowSettings, NacaParams, cl_thin};

use super::types::{
    CoeffModeButton, ExportPolarsButton, ExportStatus,
    ExportStatusText, FallbackWarningBadge, FlowAlphaControls,
    FlowToggleKind, InputModeButton, InputSlider,
    LeftPanelMainControls, LeftPanelPanelControls, NacaHeading,
    NumericField, NumericInput, NumericInputFocus, NumericInputRow,
    NumericInputText, PanelCountText, PanelSections,
    PolarSweepSettings, PolarsControls, SectionContent, SectionToggle,
    SolverDiagnostics, TableField, ThemeToggleButton, UiCoeffMode,
    UiColorThemeMode, UiInputMode, ViewButton, VisualMode,
};
use super::{config, feathers_theme, style};
use std::path::{Path, PathBuf};

pub fn set_initial_ui_scale(
    mut ui_scale: ResMut<UiScale>,
    windows: Query<&Window, With<PrimaryWindow>>,
) {
    if let Ok(window) = windows.single() {
        ui_scale.0 = config::target_ui_scale(
            window.resolution.width(),
            window.resolution.height(),
        );
    }
}

pub fn update_ui_scale_on_resize(
    mut ui_scale: ResMut<UiScale>,
    mut resize_events: MessageReader<WindowResized>,
) {
    for event in resize_events.read() {
        let new_scale =
            config::target_ui_scale(event.width, event.height);
        if (ui_scale.0 - new_scale).abs() > 0.01 {
            ui_scale.0 = new_scale;
        }
    }
}

pub fn slim_sliders(
    mut sliders: Query<&mut Node, With<Slider>>,
    ui_scale: Res<UiScale>,
) {
    let scale = ui_scale.0.max(0.001);
    let target_height_px = 14.0 / scale;
    let target_pad_px = 4.0 / scale;

    for mut node in &mut sliders {
        node.height = Val::Px(target_height_px);
        node.min_height = Val::Px(target_height_px);
        node.max_height = Val::Px(target_height_px);
        node.padding =
            UiRect::axes(Val::Px(target_pad_px), Val::Px(0.0));
    }
}

pub fn handle_theme_toggle_button(
    mut theme_mode: ResMut<UiColorThemeMode>,
    mut theme: ResMut<bevy::feathers::theme::UiTheme>,
    mut q: Query<
        &Interaction,
        (With<ThemeToggleButton>, Changed<Interaction>),
    >,
) {
    for interaction in &mut q {
        if !matches!(*interaction, Interaction::Pressed) {
            continue;
        }
        *theme_mode = theme_mode.toggle();
        *theme = bevy::feathers::theme::UiTheme(
            feathers_theme::theme_props_for(*theme_mode),
        );
    }
}

pub fn update_theme_toggle_button(
    theme_mode: Res<UiColorThemeMode>,
    mut commands: Commands,
    buttons: Query<
        (
            Entity,
            &Children,
            Option<&ThemeBackgroundColor>,
            Option<&ThemeFontColor>,
        ),
        With<ThemeToggleButton>,
    >,
    mut texts: Query<&mut Text>,
) {
    if !theme_mode.is_changed() {
        return;
    }

    let mono_active = *theme_mode == UiColorThemeMode::XFoilMono;
    let desired_bg = if mono_active {
        tokens::BUTTON_PRIMARY_BG
    } else {
        tokens::BUTTON_BG
    };
    let desired_font = if mono_active {
        tokens::BUTTON_PRIMARY_TEXT
    } else {
        tokens::BUTTON_TEXT
    };

    for (entity, children, bg, font) in &buttons {
        let bg_ok = bg.is_some_and(|t| t.0 == desired_bg);
        let font_ok = font.is_some_and(|t| t.0 == desired_font);
        if !bg_ok || !font_ok {
            commands.entity(entity).insert((
                ThemeBackgroundColor(desired_bg.clone()),
                ThemeFontColor(desired_font.clone()),
            ));
        }

        if let Some(&child) = children.first() {
            if let Ok(mut text) = texts.get_mut(child) {
                text.0 = theme_mode.label().to_string();
            }
        }
    }
}

pub fn handle_export_polars_button(
    mut status: ResMut<ExportStatus>,
    mut q: Query<
        &Interaction,
        (With<ExportPolarsButton>, Changed<Interaction>),
    >,
    params: Res<NacaParams>,
    flow: Res<FlowSettings>,
    sweep: Res<PolarSweepSettings>,
    coeff_mode: Res<UiCoeffMode>,
) {
    for interaction in &mut q {
        if !matches!(*interaction, Interaction::Pressed) {
            continue;
        }

        let threads = match sweep.threads {
            0 => None,
            n => Some(n as usize),
        };
        let mode = match *coeff_mode {
            UiCoeffMode::Panel => {
                crate::solvers::polar::PolarMode::Panel
            }
            UiCoeffMode::Approx => {
                crate::solvers::polar::PolarMode::Approx
            }
        };
        let res = crate::solvers::polar::compute_polar_sweep_parallel_with_system_mode(
            &params,
            &flow,
            sweep.alpha_min_deg,
            sweep.alpha_max_deg,
            sweep.alpha_step_deg,
            None,
            threads,
            mode,
        );
        let rows = res.rows;
        let used_fallback = res.used_fallback;

        if let Err(err) = std::fs::create_dir_all("exports") {
            warn!("failed to create exports/: {err}");
            status.message = "Export failed".into();
            return;
        }

        let path = next_available_export_path(&params, &flow);
        let mut out = String::new();
        out.push_str(
            "alpha_deg,cl,cm_c4,cd_profile,mach,reynolds,viscous,free_transition,probable_stall\n",
        );
        for r in rows {
            let cd = r.cd_profile.unwrap_or(f32::NAN);
            out.push_str(&format!(
                "{:.3},{:.6},{:.6},{:.6},{:.4},{:.0},{},{},{}\n",
                r.alpha_deg,
                r.cl,
                r.cm_c4,
                cd,
                flow.mach,
                flow.reynolds,
                flow.viscous as u8,
                flow.free_transition as u8,
                r.probable_stall as u8,
            ));
        }

        match std::fs::write(&path, out) {
            Ok(()) => {
                info!("exported polars to {}", path.display());
                status.message = if used_fallback {
                    format!("Saved (fallback used): {}", path.display())
                } else {
                    format!("Saved: {}", path.display())
                };
            }
            Err(err) => {
                warn!("failed to export polars: {err}");
                status.message = "Export failed".into();
            }
        }
    }
}

pub fn update_export_status_text(
    status: Res<ExportStatus>,
    mut texts: Query<&mut Text, With<ExportStatusText>>,
) {
    if !status.is_changed() {
        return;
    }
    for mut text in &mut texts {
        text.0 = status.message.clone();
    }
}

fn next_available_export_path(
    params: &NacaParams,
    flow: &FlowSettings,
) -> PathBuf {
    let visc_tag = if flow.viscous { "visc" } else { "invisc" };
    let tr_tag = if flow.free_transition {
        "auto"
    } else {
        "forced"
    };
    let re_m = flow.reynolds / 1_000_000.0;

    let base = format!(
        "polar_{}_Re{:.2}e6_M{:.2}_{}_{}.csv",
        params.code(),
        re_m,
        flow.mach,
        visc_tag,
        tr_tag,
    );
    let dir = Path::new("exports");
    let mut path = dir.join(base);
    if !path.exists() {
        return path;
    }

    for i in 1..1000 {
        let name = format!(
            "polar_{}_Re{:.2}e6_M{:.2}_{}_{}_{}.csv",
            params.code(),
            re_m,
            flow.mach,
            visc_tag,
            tr_tag,
            i
        );
        path = dir.join(name);
        if !path.exists() {
            return path;
        }
    }

    dir.join("polar_export.csv")
}

pub fn update_naca_heading(
    params: Res<NacaParams>,
    mut headings: Query<&mut Text, With<NacaHeading>>,
) {
    if !params.is_changed() {
        return;
    }
    for mut text in &mut headings {
        text.0 = params.code();
    }
}

pub fn update_left_panel_visibility(
    mode: Res<VisualMode>,
    mut main: Query<
        &mut Node,
        (With<LeftPanelMainControls>, Without<LeftPanelPanelControls>),
    >,
    mut panels: Query<
        &mut Node,
        (With<LeftPanelPanelControls>, Without<LeftPanelMainControls>),
    >,
    mut polars: Query<
        &mut Node,
        (
            With<PolarsControls>,
            Without<LeftPanelMainControls>,
            Without<LeftPanelPanelControls>,
        ),
    >,
    mut flow_alpha: Query<
        &mut Node,
        (
            With<FlowAlphaControls>,
            Without<LeftPanelMainControls>,
            Without<LeftPanelPanelControls>,
            Without<PolarsControls>,
        ),
    >,
) {
    if !mode.is_changed() {
        return;
    }
    let show_panels = *mode == VisualMode::Panels;
    for mut node in &mut main {
        node.display = if show_panels {
            Display::None
        } else {
            Display::Flex
        };
    }
    for mut node in &mut panels {
        node.display = if show_panels {
            Display::Flex
        } else {
            Display::None
        };
    }

    let show_polars = *mode == VisualMode::Polars;
    for mut node in &mut polars {
        node.display = if show_polars {
            Display::Flex
        } else {
            Display::None
        };
    }

    let show_alpha = *mode != VisualMode::Polars;
    for mut node in &mut flow_alpha {
        node.display = if show_alpha {
            Display::Flex
        } else {
            Display::None
        };
    }
}

pub fn update_panel_count_text(
    params: Res<NacaParams>,
    mut texts: Query<&mut Text, With<PanelCountText>>,
) {
    if !params.is_changed() {
        return;
    }
    let total_panels =
        crate::airfoil::build_naca_body_geometry(&params)
            .len()
            .saturating_sub(1);
    let label = format!(
        "Points per surface: {}  |  total panels: {}",
        params.num_points, total_panels
    );
    for mut text in &mut texts {
        text.0 = label.clone();
    }
}

pub fn handle_numeric_input_focus(
    mut focus: ResMut<NumericInputFocus>,
    mut q: Query<
        (Entity, &Interaction, &NumericInput),
        Changed<Interaction>,
    >,
    params: Res<NacaParams>,
    flow: Res<FlowSettings>,
    sweep: Res<PolarSweepSettings>,
    input_mode: Res<UiInputMode>,
) {
    if *input_mode != UiInputMode::TypeOnly {
        return;
    }
    for (entity, interaction, input) in &mut q {
        if matches!(*interaction, Interaction::Pressed) {
            focus.active = Some(entity);
            focus.buffer = format_numeric_value(
                input.field,
                &params,
                &flow,
                &sweep,
                input,
            );
        }
    }
}

pub fn handle_numeric_input_edit(
    mut focus: ResMut<NumericInputFocus>,
    keys: Res<ButtonInput<KeyCode>>,
    inputs: Query<&NumericInput>,
    mut params: ResMut<NacaParams>,
    mut flow: ResMut<FlowSettings>,
    mut sweep: ResMut<PolarSweepSettings>,
    input_mode: Res<UiInputMode>,
) {
    if *input_mode != UiInputMode::TypeOnly {
        if focus.active.is_some() {
            focus.active = None;
            focus.buffer.clear();
        }
        return;
    }
    let Some(active) = focus.active else {
        return;
    };
    let Ok(meta) = inputs.get(active) else {
        focus.active = None;
        focus.buffer.clear();
        return;
    };

    for key in keys.get_just_pressed() {
        let c = match key {
            KeyCode::Digit0 | KeyCode::Numpad0 => Some('0'),
            KeyCode::Digit1 | KeyCode::Numpad1 => Some('1'),
            KeyCode::Digit2 | KeyCode::Numpad2 => Some('2'),
            KeyCode::Digit3 | KeyCode::Numpad3 => Some('3'),
            KeyCode::Digit4 | KeyCode::Numpad4 => Some('4'),
            KeyCode::Digit5 | KeyCode::Numpad5 => Some('5'),
            KeyCode::Digit6 | KeyCode::Numpad6 => Some('6'),
            KeyCode::Digit7 | KeyCode::Numpad7 => Some('7'),
            KeyCode::Digit8 | KeyCode::Numpad8 => Some('8'),
            KeyCode::Digit9 | KeyCode::Numpad9 => Some('9'),
            KeyCode::Period | KeyCode::NumpadDecimal => Some('.'),
            KeyCode::Minus | KeyCode::NumpadSubtract => Some('-'),
            KeyCode::Equal => Some('+'),
            KeyCode::KeyE => Some('e'),
            _ => None,
        };
        if let Some(c) = c {
            focus.buffer.push(c);
        }
    }

    if keys.just_pressed(KeyCode::Backspace) {
        focus.buffer.pop();
    }

    if keys.just_pressed(KeyCode::Escape) {
        focus.active = None;
        focus.buffer.clear();
        return;
    }

    if keys.just_pressed(KeyCode::Enter) {
        if let Ok(v) = focus.buffer.trim().parse::<f32>() {
            set_numeric_value(
                meta.field,
                v,
                &mut params,
                &mut flow,
                &mut sweep,
                meta,
            );
        }
        focus.active = None;
        focus.buffer.clear();
    }
}

pub fn sync_numeric_inputs(
    mut commands: Commands,
    focus: Res<NumericInputFocus>,
    mut inputs: Query<(
        Entity,
        &NumericInput,
        Option<&ThemeBackgroundColor>,
        Option<&ThemeBorderColor>,
    )>,
    mut texts: Query<(&NumericInputText, &mut Text)>,
    params: Res<NacaParams>,
    flow: Res<FlowSettings>,
    sweep: Res<PolarSweepSettings>,
    input_mode: Res<UiInputMode>,
) {
    if *input_mode != UiInputMode::TypeOnly {
        return;
    }

    if focus.is_changed() || input_mode.is_changed() {
        for (entity, _input, bg, border) in &mut inputs {
            let focused = focus.active == Some(entity);

            let desired_bg = if focused {
                tokens::BUTTON_BG_HOVER
            } else {
                tokens::BUTTON_BG
            };
            let desired_border = if focused {
                tokens::CHECKBOX_BORDER_HOVER
            } else {
                tokens::CHECKBOX_BORDER
            };

            let bg_ok = bg.is_some_and(|t| t.0 == desired_bg);
            let border_ok =
                border.is_some_and(|t| t.0 == desired_border);
            if !bg_ok || !border_ok {
                commands.entity(entity).insert((
                    ThemeBackgroundColor(desired_bg),
                    ThemeBorderColor(desired_border),
                ));
            }
        }
    }

    for (owner, mut text) in &mut texts {
        let Ok((entity, input, _, _)) = inputs.get(owner.owner) else {
            continue;
        };
        let focused = focus.active == Some(entity);
        text.0 = if focused {
            focus.buffer.clone()
        } else {
            format_numeric_value(
                input.field,
                &params,
                &flow,
                &sweep,
                input,
            )
        };
    }
}

fn format_numeric_value(
    field: NumericField,
    params: &NacaParams,
    flow: &FlowSettings,
    sweep: &PolarSweepSettings,
    meta: &NumericInput,
) -> String {
    let v = match field {
        NumericField::NacaMDigit => params.m_digit,
        NumericField::NacaPDigit => params.p_digit,
        NumericField::NacaTDigits => params.t_digits,
        NumericField::AlphaDeg => flow.alpha_deg,
        NumericField::ReynoldsMillions => flow.reynolds / 1_000_000.0,
        NumericField::Mach => flow.mach,
        NumericField::PolarAlphaMinDeg => sweep.alpha_min_deg,
        NumericField::PolarAlphaMaxDeg => sweep.alpha_max_deg,
        NumericField::PolarAlphaStepDeg => sweep.alpha_step_deg,
        NumericField::PolarThreads => sweep.threads as f32,
    };

    if meta.integer {
        format!("{:.0}", v.round())
    } else {
        match meta.precision {
            0 => format!("{:.0}", v),
            1 => format!("{:.1}", v),
            2 => format!("{:.2}", v),
            3 => format!("{:.3}", v),
            _ => format!("{:.*}", meta.precision as usize, v),
        }
    }
}

fn set_numeric_value(
    field: NumericField,
    raw: f32,
    params: &mut NacaParams,
    flow: &mut FlowSettings,
    sweep: &mut PolarSweepSettings,
    meta: &NumericInput,
) {
    let mut v = raw.clamp(meta.min, meta.max);
    if meta.integer {
        v = v.round();
    }

    match field {
        NumericField::NacaMDigit => params.m_digit = v,
        NumericField::NacaPDigit => params.p_digit = v,
        NumericField::NacaTDigits => params.t_digits = v,
        NumericField::AlphaDeg => flow.alpha_deg = v,
        NumericField::ReynoldsMillions => {
            flow.reynolds = v * 1_000_000.0
        }
        NumericField::Mach => flow.mach = v,
        NumericField::PolarAlphaMinDeg => sweep.alpha_min_deg = v,
        NumericField::PolarAlphaMaxDeg => sweep.alpha_max_deg = v,
        NumericField::PolarAlphaStepDeg => sweep.alpha_step_deg = v,
        NumericField::PolarThreads => {
            sweep.threads = v.round().clamp(0.0, 255.0) as u8;
        }
    }
}

pub fn normalize_polar_sweep_settings(
    mut sweep: ResMut<PolarSweepSettings>,
) {
    if !sweep.is_changed() {
        return;
    }

    sweep.alpha_min_deg = sweep.alpha_min_deg.clamp(-30.0, 30.0);
    sweep.alpha_max_deg = sweep.alpha_max_deg.clamp(-30.0, 30.0);
    sweep.alpha_step_deg = sweep.alpha_step_deg.clamp(0.1, 5.0);

    if sweep.alpha_min_deg > sweep.alpha_max_deg {
        let tmp = sweep.alpha_min_deg;
        sweep.alpha_min_deg = sweep.alpha_max_deg;
        sweep.alpha_max_deg = tmp;
    }

    let available = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
        .min(32) as u8;
    sweep.threads = sweep.threads.min(available);
}

pub fn handle_input_mode_buttons(
    mut input_mode: ResMut<UiInputMode>,
    mut q: Query<
        (&Interaction, &InputModeButton),
        Changed<Interaction>,
    >,
) {
    for (interaction, button) in &mut q {
        if matches!(*interaction, Interaction::Pressed) {
            *input_mode = button.mode;
        }
    }
}

pub fn update_input_mode_button_styles(
    input_mode: Res<UiInputMode>,
    mut commands: Commands,
    mut q: Query<(
        Entity,
        &InputModeButton,
        Option<&ThemeBackgroundColor>,
        Option<&ThemeFontColor>,
    )>,
) {
    if !input_mode.is_changed() {
        return;
    }
    for (entity, button, bg, font) in &mut q {
        let selected = button.mode == *input_mode;
        let desired_bg = if selected {
            tokens::BUTTON_PRIMARY_BG
        } else {
            tokens::BUTTON_BG
        };
        let desired_font = if selected {
            tokens::BUTTON_PRIMARY_TEXT
        } else {
            tokens::BUTTON_TEXT
        };

        let bg_ok = bg.is_some_and(|t| t.0 == desired_bg);
        let font_ok = font.is_some_and(|t| t.0 == desired_font);
        if !bg_ok || !font_ok {
            commands.entity(entity).insert((
                ThemeBackgroundColor(desired_bg),
                ThemeFontColor(desired_font),
            ));
        }
    }
}

pub fn handle_coeff_mode_buttons(
    mut coeff_mode: ResMut<UiCoeffMode>,
    mut q: Query<
        (&Interaction, &CoeffModeButton),
        Changed<Interaction>,
    >,
) {
    for (interaction, button) in &mut q {
        if matches!(*interaction, Interaction::Pressed) {
            *coeff_mode = button.mode;
        }
    }
}

pub fn update_coeff_mode_button_styles(
    coeff_mode: Res<UiCoeffMode>,
    mut commands: Commands,
    mut q: Query<(
        Entity,
        &CoeffModeButton,
        Option<&ThemeBackgroundColor>,
        Option<&ThemeFontColor>,
    )>,
) {
    if !coeff_mode.is_changed() {
        return;
    }
    for (entity, button, bg, font) in &mut q {
        let selected = button.mode == *coeff_mode;
        let desired_bg = if selected {
            tokens::BUTTON_PRIMARY_BG
        } else {
            tokens::BUTTON_BG
        };
        let desired_font = if selected {
            tokens::BUTTON_PRIMARY_TEXT
        } else {
            tokens::BUTTON_TEXT
        };

        let bg_ok = bg.is_some_and(|t| t.0 == desired_bg);
        let font_ok = font.is_some_and(|t| t.0 == desired_font);
        if !bg_ok || !font_ok {
            commands.entity(entity).insert((
                ThemeBackgroundColor(desired_bg),
                ThemeFontColor(desired_font),
            ));
        }
    }
}

pub fn update_fallback_warning_badge(
    diag: Res<SolverDiagnostics>,
    mut q: Query<&mut Node, With<FallbackWarningBadge>>,
) {
    if !diag.is_changed() {
        return;
    }
    for mut node in &mut q {
        node.display = if diag.fallback_active {
            Display::Flex
        } else {
            Display::None
        };
    }
}

pub fn update_numeric_input_visibility(
    input_mode: Res<UiInputMode>,
    mut focus: ResMut<NumericInputFocus>,
    mut rows: Query<
        &mut Node,
        (With<NumericInputRow>, Without<InputSlider>),
    >,
    mut sliders: Query<
        &mut Node,
        (With<InputSlider>, Without<NumericInputRow>),
    >,
) {
    let show_numeric = *input_mode == UiInputMode::TypeOnly;
    if !show_numeric && focus.active.is_some() {
        focus.active = None;
        focus.buffer.clear();
    }
    for mut node in &mut rows {
        node.display = if show_numeric {
            Display::Flex
        } else {
            Display::None
        };
    }
    for mut node in &mut sliders {
        node.display = if show_numeric {
            Display::None
        } else {
            Display::Flex
        };
    }
}

pub fn update_table_text(
    params: Res<NacaParams>,
    flow: Res<FlowSettings>,
    coeff_mode: Res<UiCoeffMode>,
    mut query: Query<(&mut Text, &TableField)>,
    mut cache: Local<UiPanelSystemCache>,
) {
    if !params.is_changed()
        && !flow.is_changed()
        && !coeff_mode.is_changed()
    {
        return;
    }

    let cl = cl_thin(flow.alpha_deg);
    let key = PanelKey::from(&*params);
    if cache.key != Some(key) {
        cache.key = Some(key);
        cache.system = PanelLuSystem::new(&params);
    }

    let panel_sol = match *coeff_mode {
        UiCoeffMode::Approx => {
            crate::solvers::panel::compute_approx_solution(
                &params,
                flow.alpha_deg,
            )
        }
        UiCoeffMode::Panel => {
            let sol = cache
                .system
                .as_ref()
                .map(|sys| sys.panel_solution(&params, flow.alpha_deg))
                .unwrap_or_else(|| {
                    compute_panel_solution(&params, flow.alpha_deg)
                });
            if sol.x.is_empty() {
                crate::solvers::panel::compute_approx_solution(
                    &params,
                    flow.alpha_deg,
                )
            } else {
                sol
            }
        }
    };
    let est_cl = panel_sol.cl().unwrap_or(f32::NAN);
    let est_cm = panel_sol.cm_c4().unwrap_or(f32::NAN);
    let beta = (1.0 - flow.mach * flow.mach).clamp(0.05, 1.0).sqrt();
    let est_cl_corr = est_cl / beta;
    let bl_inputs = BoundaryLayerInputs::new(
        flow.reynolds,
        flow.mach,
        flow.viscous,
        flow.free_transition,
        config::FORCED_TRIP_X,
    );
    let boundary_layer =
        estimate_boundary_layer(&panel_sol, &bl_inputs);
    let est_cdp_text = boundary_layer
        .as_ref()
        .map(|res| format!("{:.4}", res.cd_profile))
        .unwrap_or_else(|| "--".into());
    let flow_state_text = if let Some(res) = boundary_layer.as_ref() {
        style::describe_flow_state(res)
    } else if !flow.viscous {
        "viscosity off".into()
    } else {
        "--".into()
    };

    for (mut text, field) in &mut query {
        text.0 = match field {
            TableField::NacaCode => params.code(),
            TableField::AlphaDeg => format!("{:.2}", flow.alpha_deg),
            TableField::Mach => format!("{:.2}", flow.mach),
            TableField::Reynolds => {
                format!("{:.2}", flow.reynolds / 1_000_000.0)
            }
            TableField::ViscosityMode => {
                if flow.viscous {
                    "Enabled".into()
                } else {
                    "Off (inviscid)".into()
                }
            }
            TableField::TransitionMode => {
                if flow.free_transition {
                    "Auto".into()
                } else {
                    "Forced trip".into()
                }
            }
            TableField::ClThin => format!("{:.3}", cl),
            TableField::RefCl => format!("{:.4}", est_cl_corr),
            TableField::RefCm => format!("{:.4}", est_cm),
            TableField::RefCdp => est_cdp_text.clone(),
            TableField::FlowState => flow_state_text.clone(),
        };
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct PanelKey {
    m: u8,
    p: u8,
    t: u16,
    num_points: usize,
}

impl From<&NacaParams> for PanelKey {
    fn from(params: &NacaParams) -> Self {
        Self {
            m: params.m_digit.round().clamp(0.0, 9.0) as u8,
            p: params.p_digit.round().clamp(0.0, 9.0) as u8,
            t: params.t_digits.round().clamp(0.0, 99.0) as u16,
            num_points: params.num_points,
        }
    }
}

#[derive(Default)]
pub struct UiPanelSystemCache {
    key: Option<PanelKey>,
    system: Option<PanelLuSystem>,
}

pub fn handle_view_buttons(
    mut mode: ResMut<VisualMode>,
    mut commands: Commands,
    q: Query<(
        Entity,
        &Interaction,
        &ViewButton,
        Option<&ThemeBackgroundColor>,
        Option<&ThemeFontColor>,
    )>,
) {
    let mut new_mode = *mode;
    let mut items = Vec::new();
    for (entity, interaction, button, bg, font) in &q {
        if let Interaction::Pressed = *interaction {
            new_mode = button.mode;
        }
        items.push((entity, button.mode, bg, font));
    }
    *mode = new_mode;

    for (entity, button_mode, bg, font) in items {
        let selected = *mode == button_mode;
        let desired_bg = if selected {
            tokens::BUTTON_PRIMARY_BG
        } else {
            tokens::BUTTON_BG
        };
        let desired_font = if selected {
            tokens::BUTTON_PRIMARY_TEXT
        } else {
            tokens::BUTTON_TEXT
        };

        let bg_ok = bg.is_some_and(|t| t.0 == desired_bg);
        let font_ok = font.is_some_and(|t| t.0 == desired_font);
        if !bg_ok || !font_ok {
            commands.entity(entity).insert((
                ThemeBackgroundColor(desired_bg),
                ThemeFontColor(desired_font),
            ));
        }
    }
}

pub fn handle_section_toggle_buttons(
    mut sections: ResMut<PanelSections>,
    mut commands: Commands,
    mut q: Query<
        (
            Entity,
            &Interaction,
            &SectionToggle,
            &Children,
            Option<&ThemeBackgroundColor>,
        ),
        (Changed<Interaction>, With<SectionToggle>),
    >,
    mut texts: Query<&mut Text>,
    mut contents: Query<(&SectionContent, &mut Node)>,
) {
    let mut any_changed = false;
    for (entity, interaction, toggle, children, bg) in &mut q {
        let mut open = sections.is_open(toggle.section);
        if let Interaction::Pressed = *interaction {
            open = sections.toggle(toggle.section);
            any_changed = true;
        }
        let desired_bg = if open {
            tokens::BUTTON_BG_HOVER
        } else {
            tokens::BUTTON_BG
        };
        if !bg.is_some_and(|t| t.0 == desired_bg) {
            commands
                .entity(entity)
                .insert(ThemeBackgroundColor(desired_bg));
        }

        if let Some(&child) = children.first() {
            if let Ok(mut text) = texts.get_mut(child) {
                text.0 =
                    style::section_header_label(toggle.section, open);
            }
        }
    }

    if any_changed {
        for (content, mut node) in &mut contents {
            let open = sections.is_open(content.section);
            node.display =
                if open { Display::Flex } else { Display::None };
        }
    }
}

pub fn handle_flow_toggle_buttons(
    mut flow: ResMut<FlowSettings>,
    mut commands: Commands,
    mut q: Query<
        (
            Entity,
            &Interaction,
            &FlowToggleKind,
            &Children,
            Option<&ThemeBackgroundColor>,
            Option<&ThemeFontColor>,
        ),
        (Changed<Interaction>, With<FlowToggleKind>),
    >,
    mut texts: Query<&mut Text>,
) {
    for (entity, interaction, kind, children, bg, font) in &mut q {
        if let Interaction::Pressed = *interaction {
            match kind {
                FlowToggleKind::Viscosity => {
                    flow.viscous = !flow.viscous;
                }
                FlowToggleKind::Transition => {
                    flow.free_transition = !flow.free_transition;
                }
            }
        }

        let active = match kind {
            FlowToggleKind::Viscosity => flow.viscous,
            FlowToggleKind::Transition => flow.free_transition,
        };
        let desired_bg = if active {
            tokens::BUTTON_PRIMARY_BG
        } else {
            tokens::BUTTON_BG
        };
        let desired_font = if active {
            tokens::BUTTON_PRIMARY_TEXT
        } else {
            tokens::BUTTON_TEXT
        };
        let bg_ok = bg.is_some_and(|t| t.0 == desired_bg);
        let font_ok = font.is_some_and(|t| t.0 == desired_font);
        if !bg_ok || !font_ok {
            commands.entity(entity).insert((
                ThemeBackgroundColor(desired_bg),
                ThemeFontColor(desired_font),
            ));
        }

        if let Some(&child) = children.first() {
            if let Ok(mut text) = texts.get_mut(child) {
                text.0 =
                    style::flow_toggle_label(*kind, active).to_string();
            }
        }
    }
}
