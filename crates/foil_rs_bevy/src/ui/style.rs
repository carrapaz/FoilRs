use crate::solvers::BoundaryLayerResult;

use super::types::{FlowToggleKind, PanelSection};

pub(super) fn section_header_label(
    section: PanelSection,
    open: bool,
) -> String {
    let state = if open { "[-]" } else { "[+]" };
    let title = match section {
        PanelSection::Geometry => "NACA geometry",
        PanelSection::Flow => "Freestream & viscosity",
        PanelSection::Polars => "Polar sweep",
    };
    format!("{state} {title}")
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
