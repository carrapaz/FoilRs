#![cfg(feature = "ui")]
use bevy::{
    feathers::constants::fonts,
    prelude::*,
    sprite::Text2d,
    text::{Justify, TextColor, TextFont, TextLayout},
};

/// Stores spawned entities used as labels for the Cp plot, so we can cleanly
/// despawn and refresh them each frame.
#[derive(Resource, Default)]
pub struct CpPlotLabels {
    pub entities: Vec<Entity>,
}

/// Stores spawned entities used as labels for the polar plots.
#[derive(Resource, Default)]
pub struct PolarPlotLabels {
    pub entities: Vec<Entity>,
}

/// Remove any existing Cp plot labels.
pub fn clear_cp_labels(
    commands: &mut Commands,
    labels: &mut CpPlotLabels,
) {
    for e in labels.entities.drain(..) {
        commands.entity(e).despawn();
    }
}

/// Remove any existing polar plot labels.
pub fn clear_polar_labels(
    commands: &mut Commands,
    labels: &mut PolarPlotLabels,
) {
    for e in labels.entities.drain(..) {
        commands.entity(e).despawn();
    }
}

/// Refresh axis labels for the Cp plot. This despawns previous labels and
/// respawns new ones at the current tick positions.
pub fn refresh_cp_labels(
    commands: &mut Commands,
    asset_server: &AssetServer,
    labels: &mut CpPlotLabels,
    x_ticks: &[f32],
    cp_ticks: &[f32],
    base_y: f32,
) {
    clear_cp_labels(commands, labels);

    let font = asset_server.load(fonts::MONO);
    let color = Color::srgb(0.7, 0.7, 0.7);
    let font_size = 12.0;

    // X ticks (along chord fraction)
    for &x in x_ticks {
        let world_x = (x - 0.5) * 450.0; // match CHORD_PX
        let pos = Vec3::new(world_x, base_y - 18.0, 1.0);
        let text = format!("{:.2}", x);
        let ent = commands
            .spawn((
                Text2d::new(text),
                TextFont {
                    font: font.clone(),
                    font_size,
                    ..default()
                },
                TextLayout::new_with_justify(Justify::Center),
                TextColor(color),
                Transform::from_translation(pos),
            ))
            .id();
        labels.entities.push(ent);
    }

    // Cp ticks
    for &cp in cp_ticks {
        let world_y = base_y - cp * 120.0; // match scale_y
        let pos = Vec3::new(-0.5 * 450.0 - 32.0, world_y, 1.0);
        let text = format!("{:.1}", cp);
        let ent = commands
            .spawn((
                Text2d::new(text),
                TextFont {
                    font: font.clone(),
                    font_size,
                    ..default()
                },
                TextLayout::new_with_justify(Justify::Right),
                TextColor(color),
                Transform::from_translation(pos),
            ))
            .id();
        labels.entities.push(ent);
    }
}

/// Refresh axis labels for the polar plots (α, CL, CDp).
pub fn refresh_polar_labels(
    commands: &mut Commands,
    asset_server: &AssetServer,
    labels: &mut PolarPlotLabels,
    alpha_ticks: &[f32],
    cl_ticks: &[f32],
    cd_ticks: &[f32],
    cl_base_y: f32,
    cl_scale_y: f32,
    cd_base_y: f32,
    cd_scale_y: f32,
) {
    clear_polar_labels(commands, labels);

    const CHORD_PX: f32 = 450.0;
    const ALPHA_MIN_DEG: f32 = -10.0;
    const ALPHA_MAX_DEG: f32 = 15.0;

    let font = asset_server.load(fonts::MONO);
    let color = Color::srgb(0.7, 0.7, 0.7);
    let font_size = 12.0;

    let x_left = -0.5 * CHORD_PX;
    let x_right = 0.5 * CHORD_PX;

    let alpha_to_world_x = |alpha_deg: f32| -> f32 {
        let t = ((alpha_deg - ALPHA_MIN_DEG)
            / (ALPHA_MAX_DEG - ALPHA_MIN_DEG))
            .clamp(0.0, 1.0);
        x_left + t * (x_right - x_left)
    };

    // α tick labels (below the CD axis)
    for &a in alpha_ticks {
        let world_x = alpha_to_world_x(a);
        let pos = Vec3::new(world_x, cd_base_y - 18.0, 1.0);
        let text = format!("{:.0}", a);
        let ent = commands
            .spawn((
                Text2d::new(text),
                TextFont {
                    font: font.clone(),
                    font_size,
                    ..default()
                },
                TextLayout::new_with_justify(Justify::Center),
                TextColor(color),
                Transform::from_translation(pos),
            ))
            .id();
        labels.entities.push(ent);
    }

    // CL tick labels
    for &cl in cl_ticks {
        let world_y = cl_base_y + cl * cl_scale_y;
        let pos = Vec3::new(x_left - 36.0, world_y, 1.0);
        let text = format!("{:.1}", cl);
        let ent = commands
            .spawn((
                Text2d::new(text),
                TextFont {
                    font: font.clone(),
                    font_size,
                    ..default()
                },
                TextLayout::new_with_justify(Justify::Right),
                TextColor(color),
                Transform::from_translation(pos),
            ))
            .id();
        labels.entities.push(ent);
    }

    // CD tick labels
    for &cd in cd_ticks {
        let world_y = cd_base_y + cd * cd_scale_y;
        let pos = Vec3::new(x_left - 36.0, world_y, 1.0);
        let text = format!("{:.3}", cd);
        let ent = commands
            .spawn((
                Text2d::new(text),
                TextFont {
                    font: font.clone(),
                    font_size,
                    ..default()
                },
                TextLayout::new_with_justify(Justify::Right),
                TextColor(color),
                Transform::from_translation(pos),
            ))
            .id();
        labels.entities.push(ent);
    }

    // Plot labels (titles)
    for (label, y) in
        [("CL", cl_base_y + 110.0), ("CDp", cd_base_y + 90.0)]
    {
        let ent = commands
            .spawn((
                Text2d::new(label),
                TextFont {
                    font: font.clone(),
                    font_size,
                    ..default()
                },
                TextLayout::new_with_justify(Justify::Left),
                TextColor(color.with_alpha(0.9)),
                Transform::from_translation(Vec3::new(x_left, y, 1.0)),
            ))
            .id();
        labels.entities.push(ent);
    }
}
