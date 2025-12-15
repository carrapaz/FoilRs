pub(super) const BASE_UI_WIDTH: f32 = 1920.0;
pub(super) const BASE_UI_HEIGHT: f32 = 1080.0;
pub(super) const MIN_UI_SCALE: f32 = 0.75;
pub(super) const MAX_UI_SCALE: f32 = 1.3;

pub(super) const FORCED_TRIP_X: f32 = 0.05;

pub(super) const BUTTON_RADIUS: f32 = 8.0;

pub(super) fn target_ui_scale(width: f32, height: f32) -> f32 {
    let width_ratio = width / BASE_UI_WIDTH;
    let height_ratio = height / BASE_UI_HEIGHT;
    width_ratio
        .min(height_ratio)
        .clamp(MIN_UI_SCALE, MAX_UI_SCALE)
}
