use bevy::feathers::{
    dark_theme::create_dark_theme, theme::ThemeProps, tokens,
};
use bevy::prelude::Color;

use super::types::UiColorThemeMode;

pub fn theme_props_for(mode: UiColorThemeMode) -> ThemeProps {
    match mode {
        UiColorThemeMode::Colorful => create_dark_theme(),
        UiColorThemeMode::XFoilMono => create_xfoil_mono_theme(),
    }
}

fn create_xfoil_mono_theme() -> ThemeProps {
    let mut theme = create_dark_theme();

    let bg0 = Color::srgb(0.0, 0.0, 0.0);
    let bg1 = Color::srgb(0.08, 0.08, 0.08);
    let bg2 = Color::srgb(0.14, 0.14, 0.14);
    let bg3 = Color::srgb(0.20, 0.20, 0.20);
    let fg = Color::srgb(1.0, 1.0, 1.0);
    let fg_dim = Color::srgb(0.78, 0.78, 0.78);

    theme.color.insert(tokens::WINDOW_BG, bg0);

    theme.color.insert(tokens::BUTTON_BG, bg1);
    theme.color.insert(tokens::BUTTON_BG_HOVER, bg2);
    theme.color.insert(tokens::BUTTON_BG_PRESSED, bg3);
    theme.color.insert(tokens::BUTTON_BG_DISABLED, bg1);
    theme.color.insert(tokens::BUTTON_TEXT, fg);
    theme.color.insert(tokens::BUTTON_TEXT_DISABLED, fg_dim);

    theme.color.insert(tokens::BUTTON_PRIMARY_BG, bg1);
    theme.color.insert(tokens::BUTTON_PRIMARY_BG_HOVER, bg2);
    theme.color.insert(tokens::BUTTON_PRIMARY_BG_PRESSED, bg3);
    theme.color.insert(tokens::BUTTON_PRIMARY_BG_DISABLED, bg1);
    theme.color.insert(tokens::BUTTON_PRIMARY_TEXT, fg);
    theme
        .color
        .insert(tokens::BUTTON_PRIMARY_TEXT_DISABLED, fg_dim);

    theme.color.insert(tokens::SLIDER_BG, bg1);
    theme.color.insert(tokens::SLIDER_BAR, fg);
    theme.color.insert(tokens::SLIDER_BAR_DISABLED, fg_dim);
    theme.color.insert(tokens::SLIDER_TEXT, fg);
    theme.color.insert(tokens::SLIDER_TEXT_DISABLED, fg_dim);

    theme.color.insert(tokens::CHECKBOX_BG, bg1);
    theme.color.insert(tokens::CHECKBOX_BG_CHECKED, fg);
    theme.color.insert(tokens::CHECKBOX_BORDER, fg_dim);
    theme.color.insert(tokens::CHECKBOX_BORDER_HOVER, fg);
    theme.color.insert(tokens::CHECKBOX_MARK, bg0);
    theme.color.insert(tokens::CHECKBOX_TEXT, fg);
    theme.color.insert(tokens::CHECKBOX_TEXT_DISABLED, fg_dim);

    theme.color.insert(tokens::RADIO_BORDER, fg_dim);
    theme.color.insert(tokens::RADIO_BORDER_HOVER, fg);
    theme.color.insert(tokens::RADIO_MARK, fg);
    theme.color.insert(tokens::RADIO_MARK_DISABLED, fg_dim);
    theme.color.insert(tokens::RADIO_TEXT, fg);
    theme.color.insert(tokens::RADIO_TEXT_DISABLED, fg_dim);

    theme.color.insert(tokens::SWITCH_BG, bg1);
    theme.color.insert(tokens::SWITCH_BG_CHECKED, fg);
    theme.color.insert(tokens::SWITCH_BORDER, fg_dim);
    theme.color.insert(tokens::SWITCH_BORDER_HOVER, fg);
    theme.color.insert(tokens::SWITCH_SLIDE, fg_dim);

    theme
}
