use bevy::prelude::*;

use crate::solvers::BoundaryLayerResult;
use crate::state::VisualMode;

use super::types::{FlowToggleKind, PanelSection};

pub(super) fn panel_base_color(mode: VisualMode) -> Color {
    match mode {
        VisualMode::Field => Color::srgb(0.13, 0.15, 0.20),
        VisualMode::Cp => Color::srgb(0.16, 0.12, 0.12),
        VisualMode::Polars => Color::srgb(0.12, 0.13, 0.17),
        VisualMode::Panels => Color::srgb(0.12, 0.15, 0.12),
    }
}

pub(super) fn top_bar_color(mode: VisualMode) -> Color {
    // Slightly darker than the main panel, to read as a header.
    let c = panel_base_color(mode).to_srgba();
    Color::srgba(c.red * 0.85, c.green * 0.85, c.blue * 0.85, 0.98)
}

pub(super) fn section_header_color(open: bool) -> Color {
    if open {
        Color::srgb(0.26, 0.30, 0.36)
    } else {
        Color::srgb(0.18, 0.20, 0.24)
    }
}

pub(super) fn section_header_label(
    section: PanelSection,
    open: bool,
) -> String {
    let state = if open { "[-]" } else { "[+]" };
    let title = match section {
        PanelSection::Geometry => "NACA geometry",
        PanelSection::Flow => "Freestream & viscosity",
    };
    format!("{state} {title}")
}

pub(super) fn toggle_color(active: bool) -> Color {
    if active {
        Color::srgb(0.28, 0.32, 0.38)
    } else {
        Color::srgb(0.18, 0.18, 0.22)
    }
}

pub(super) fn input_bg(focused: bool) -> Color {
    if focused {
        Color::srgb(0.16, 0.18, 0.26)
    } else {
        Color::srgb(0.12, 0.12, 0.16)
    }
}

pub(super) fn input_border(focused: bool) -> Color {
    if focused {
        Color::srgb(0.42, 0.42, 0.52)
    } else {
        Color::srgb(0.26, 0.26, 0.32)
    }
}

pub(super) fn input_mode_button_color(selected: bool) -> Color {
    if selected {
        Color::srgb(0.30, 0.28, 0.40)
    } else {
        Color::srgb(0.20, 0.20, 0.28)
    }
}

pub(super) fn view_button_color(
    current: VisualMode,
    button_mode: VisualMode,
) -> Color {
    if current == button_mode {
        Color::srgb(0.35, 0.30, 0.45)
    } else {
        Color::srgb(0.22, 0.22, 0.30)
    }
}

pub(super) fn view_button_label(mode: VisualMode) -> &'static str {
    match mode {
        VisualMode::Field => "Field",
        VisualMode::Cp => "Cp(x)",
        VisualMode::Polars => "Polars",
        VisualMode::Panels => "Panels",
    }
}

pub(super) fn flow_toggle_label(
    kind: FlowToggleKind,
    active: bool,
) -> &'static str {
    match kind {
        FlowToggleKind::Viscosity => {
            if active {
                "Viscosity: Enabled"
            } else {
                "Viscosity: Off (inviscid)"
            }
        }
        FlowToggleKind::Transition => {
            if active {
                "Transition: Auto"
            } else {
                "Transition: Forced trip"
            }
        }
    }
}

pub(super) fn describe_flow_state(res: &BoundaryLayerResult) -> String {
    if res.probable_stall {
        if let Some(x) = res.separation_upper {
            return format!("stall (upper @ {:.0}%)", x * 100.0);
        }
        if let Some(x) = res.separation_lower {
            return format!("stall (lower @ {:.0}%)", x * 100.0);
        }
        return "stall (separation)".into();
    }

    let mut parts = Vec::new();
    if let Some(x) = res.transition_upper {
        parts.push(format!("UP tr @ {:.0}%", x * 100.0));
    }
    if let Some(x) = res.transition_lower {
        parts.push(format!("LO tr @ {:.0}%", x * 100.0));
    }

    if parts.is_empty() {
        "attached".into()
    } else {
        parts.join(" | ")
    }
}
