use bevy::prelude::*;

#[derive(Resource, Clone, Copy, PartialEq, Eq)]
pub enum VisualMode {
    /// Velocity field + streamlines.
    Field,
    /// Cp(x) distribution.
    Cp,
    /// Polar curves (CL/CD/CM vs α).
    Polars,
    /// Panel discretization visualization.
    Panels,
}

impl VisualMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Field => "Field",
            Self::Cp => "Cp(x)",
            Self::Polars => "Polars",
            Self::Panels => "Panels",
        }
    }
}

/// Which value a text cell in the table represents.
#[derive(Component, Clone, Copy)]
pub enum TableField {
    NacaCode,
    AlphaDeg,
    Reynolds,
    Mach,
    ClThin,
    RefCl,
    RefCm,
    RefCdp,
    FlowState,
    ViscosityMode,
    TransitionMode,
}

#[derive(Resource, Clone, Copy, PartialEq, Eq)]
pub enum UiInputMode {
    SliderOnly,
    TypeOnly,
}

impl Default for UiInputMode {
    fn default() -> Self {
        Self::SliderOnly
    }
}

#[derive(Resource, Clone, Copy, PartialEq, Eq)]
pub enum UiColorThemeMode {
    Colorful,
    XFoilMono,
}

impl Default for UiColorThemeMode {
    fn default() -> Self {
        Self::Colorful
    }
}

impl UiColorThemeMode {
    pub fn toggle(self) -> Self {
        match self {
            Self::Colorful => Self::XFoilMono,
            Self::XFoilMono => Self::Colorful,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Colorful => "Theme: Color",
            Self::XFoilMono => "Theme: B/W",
        }
    }
}

#[derive(Resource, Clone)]
pub struct PanelSections {
    pub geometry_open: bool,
    pub flow_open: bool,
}

impl Default for PanelSections {
    fn default() -> Self {
        Self {
            geometry_open: true,
            flow_open: true,
        }
    }
}

impl PanelSections {
    pub fn is_open(&self, section: PanelSection) -> bool {
        match section {
            PanelSection::Geometry => self.geometry_open,
            PanelSection::Flow => self.flow_open,
        }
    }

    pub fn toggle(&mut self, section: PanelSection) -> bool {
        match section {
            PanelSection::Geometry => {
                self.geometry_open = !self.geometry_open;
                self.geometry_open
            }
            PanelSection::Flow => {
                self.flow_open = !self.flow_open;
                self.flow_open
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PanelSection {
    Geometry,
    Flow,
}

#[derive(Component)]
pub struct SectionToggle {
    pub section: PanelSection,
}

#[derive(Component)]
pub struct SectionContent {
    pub section: PanelSection,
}

#[derive(Component, Clone, Copy)]
pub enum FlowToggleKind {
    Viscosity,
    Transition,
}

#[derive(Component)]
pub struct ViewButton {
    pub mode: VisualMode,
}

#[derive(Component)]
pub struct InputModeButton {
    pub mode: UiInputMode,
}

#[derive(Component)]
pub struct ExportPolarsButton;

#[derive(Component)]
pub struct ExportStatusText;

#[derive(Component)]
pub struct ThemeToggleButton;

#[derive(Resource, Default, Clone)]
pub struct ExportStatus {
    pub message: String,
}

#[derive(Component)]
pub struct LeftPanelMainControls;

#[derive(Component)]
pub struct LeftPanelPanelControls;

#[derive(Component)]
pub struct PanelCountText;

#[derive(Component)]
pub struct NumericInputRow;

#[derive(Component)]
pub struct InputSlider;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum NumericField {
    NacaMDigit,
    NacaPDigit,
    NacaTDigits,
    AlphaDeg,
    ReynoldsMillions,
    Mach,
}

#[derive(Component)]
pub struct NumericInput {
    pub field: NumericField,
    pub min: f32,
    pub max: f32,
    pub precision: u8,
    pub integer: bool,
}

#[derive(Component)]
pub struct NumericInputText {
    pub owner: Entity,
}

#[derive(Resource, Default)]
pub struct NumericInputFocus {
    pub active: Option<Entity>,
    pub buffer: String,
}

#[derive(Component)]
pub struct NacaHeading;

#[derive(Component)]
pub struct ModePanel;

#[derive(Component)]
pub struct TopBar;

#[derive(Component)]
pub struct UiRoot;
