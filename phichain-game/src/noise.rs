use crate::noise_postprocess::NoisePostProcessPlugin;
use crate::{ChartTime, GameSet, GameViewport};
use bevy::asset::RenderAssetUsages;
use bevy::image::{ImageAddressMode, ImageLoaderSettings, ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use phichain_chart::bpm_list::BpmList;
use phichain_chart::constants::{CANVAS_HEIGHT, CANVAS_WIDTH};
use phichain_chart::noise::NoiseArea;

const NOISE_LAYER: f32 = 25.0;
const INITIAL_MASK_SIZE: u32 = 1;
const DISABLED_SHOW_DURATION: f32 = 0.5;

#[derive(Resource)]
pub(crate) struct NoiseComposite {
    pub(crate) mask: Handle<Image>,
    pub(crate) effect: Handle<Image>,
    pub(crate) displacement: Handle<Image>,
    pub(crate) spark: Handle<Image>,
}

#[derive(Component, Default, Debug, Clone, Copy, Ord, PartialOrd, Eq, PartialEq)]
pub struct NoiseAreaOrder(pub usize);

/// Runtime-only state for PreviewBlockControl.DisabledBlockShow.
///
/// The official coroutine advances with frame delta time, not chart time. This
/// matters when a charter seeks into a visible block while playback is paused:
/// it still fades in over half a second.
#[derive(Component, Default)]
struct NoisePreviewState {
    was_visible: bool,
    show_progress: f32,
}

pub struct NoiseAreaPlugin;

impl Plugin for NoiseAreaPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(NoisePostProcessPlugin)
            .add_systems(Startup, setup_noise_composite)
            .add_systems(
                Update,
                (update_noise_area_previews, update_noise_composite)
                    .chain()
                    .in_set(GameSet),
            )
            .add_observer(add_noise_area_preview);
    }
}

fn setup_noise_composite(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    asset_server: Res<AssetServer>,
) {
    let mut mask_image = Image::new_fill(
        Extent3d {
            width: INITIAL_MASK_SIZE,
            height: INITIAL_MASK_SIZE,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &[0, 0, 0, 0],
        TextureFormat::Rgba8Unorm,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    mask_image.sampler = ImageSampler::nearest();
    let mask = images.add(mask_image);

    let mut effect_image = Image::new_fill(
        Extent3d {
            width: INITIAL_MASK_SIZE * 2,
            height: INITIAL_MASK_SIZE * 2,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &[0, 0, 0, 0],
        TextureFormat::Rgba8Unorm,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    // BlockRender creates EffectRT with FilterMode.Bilinear. ActiveBlock snaps
    // edge reads to texel centres but deliberately interpolates the glow channel.
    effect_image.sampler = ImageSampler::linear();
    let effect = images.add(effect_image);

    let displacement = asset_server.load_with_settings(
        "noise/BlockNoise1.png",
        |settings: &mut ImageLoaderSettings| {
            // The official project uses Gamma color space. Noise values must
            // therefore be sampled as stored bytes, without sRGB decoding.
            settings.is_srgb = false;
            settings.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
                address_mode_u: ImageAddressMode::MirrorRepeat,
                address_mode_v: ImageAddressMode::MirrorRepeat,
                ..ImageSamplerDescriptor::nearest()
            });
        },
    );
    let spark = asset_server.load_with_settings(
        "noise/PointNoise.png",
        |settings: &mut ImageLoaderSettings| {
            settings.is_srgb = false;
            settings.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
                address_mode_u: ImageAddressMode::Repeat,
                address_mode_v: ImageAddressMode::Repeat,
                ..ImageSamplerDescriptor::nearest()
            });
        },
    );
    commands.insert_resource(NoiseComposite {
        mask,
        effect,
        displacement,
        spark,
    });
}

fn add_noise_area_preview(
    add: On<Add, NoiseArea>,
    mut commands: Commands,
    composite: Res<NoiseComposite>,
) {
    commands.entity(add.entity).insert((
        Sprite {
            image: composite.displacement.clone(),
            color: Color::NONE,
            custom_size: Some(Vec2::ONE),
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, NOISE_LAYER),
        NoisePreviewState::default(),
    ));
}

fn update_noise_area_previews(
    bpm_list: Option<Res<BpmList>>,
    chart_time: Res<ChartTime>,
    frame_time: Res<Time>,
    viewport: Res<GameViewport>,
    mut previews: Query<(
        Entity,
        &NoiseArea,
        &mut NoisePreviewState,
        &mut Sprite,
        &mut Transform,
    )>,
) {
    let Some(bpm_list) = bpm_list else {
        return;
    };

    for (_entity, area, mut state, mut sprite, mut transform) in &mut previews {
        let visible = area.is_visible(chart_time.0, &bpm_list);
        if !visible {
            state.was_visible = false;
            state.show_progress = 0.0;
            sprite.color = Color::NONE;
            continue;
        }

        if !state.was_visible {
            state.was_visible = true;
            state.show_progress = 0.0;
        }
        state.show_progress =
            (state.show_progress + frame_time.delta_secs() / DISABLED_SHOW_DURATION).min(1.0);

        let active = area.is_active(chart_time.0, &bpm_list);
        let enable_time = bpm_list.time_at(area.enable_beat);
        let ready = !active && chart_time.0 < enable_time && enable_time - chart_time.0 <= 0.5;
        // Domain sprites remain as invisible pick targets. The compositor below is the sole
        // visual output, so overlaps and subtract parity match the game instead of alpha-blending.
        let _ = (active, ready);
        sprite.color = Color::NONE;

        let rect = area.rect_at(chart_time.0, &bpm_list);
        let scale = Vec2::new(
            viewport.0.width() / CANVAS_WIDTH,
            viewport.0.height() / CANVAS_HEIGHT,
        );
        sprite.custom_size = Some(rect.size * scale);
        transform.translation = (rect.center * scale).extend(NOISE_LAYER);
        transform.rotation = Quat::from_rotation_z(rect.rotation_degrees.to_radians());
    }
}

fn update_noise_composite(
    areas: Query<(&NoiseArea, &NoisePreviewState)>,
    bpm_list: Option<Res<BpmList>>,
    chart_time: Res<ChartTime>,
    viewport: Res<GameViewport>,
    composite: Res<NoiseComposite>,
    mut images: ResMut<Assets<Image>>,
) {
    let Some(bpm_list) = bpm_list else {
        return;
    };
    // Use a quarter-resolution geometry mask so the displaced pixel boundary
    // keeps the official stepped character without breaking into oversized
    // eight-pixel chunks in a compact editor preview.
    let mask_width = ((viewport.0.width().max(4.0) as u32) / 4).max(1);
    let mask_height = ((viewport.0.height().max(4.0) as u32) / 4).max(1);
    let pixel_count = (mask_width * mask_height) as usize;
    let mut normal = vec![[0_u8; 3]; pixel_count];
    let mut subtract = vec![[0_u8; 3]; pixel_count];

    for (area, state) in &areas {
        if !area.is_visible(chart_time.0, &bpm_list) {
            continue;
        }
        let phase = if area.is_active(chart_time.0, &bpm_list) {
            0
        } else {
            let enable = bpm_list.time_at(area.enable_beat);
            if chart_time.0 < enable && enable - chart_time.0 <= 0.5 {
                1
            } else {
                2
            }
        };
        let coverage = (state.show_progress * 255.0).round() as u8;
        let rect = area.rect_at(chart_time.0, &bpm_list);
        let angle = Vec2::from_angle(rect.rotation_degrees.to_radians());
        let half = rect.size / 2.0;
        let corners = [
            Vec2::new(-half.x, -half.y),
            Vec2::new(-half.x, half.y),
            Vec2::new(half.x, -half.y),
            Vec2::new(half.x, half.y),
        ]
        .map(|corner| rect.center + corner.rotate(angle));
        let min = corners
            .iter()
            .fold(Vec2::splat(f32::INFINITY), |a, b| a.min(*b));
        let max = corners
            .iter()
            .fold(Vec2::splat(f32::NEG_INFINITY), |a, b| a.max(*b));
        let x0 = (((min.x / CANVAS_WIDTH + 0.5) * mask_width as f32).floor() as i32)
            .clamp(0, mask_width as i32 - 1);
        let x1 = (((max.x / CANVAS_WIDTH + 0.5) * mask_width as f32).ceil() as i32)
            .clamp(0, mask_width as i32 - 1);
        let y0 = (((0.5 - max.y / CANVAS_HEIGHT) * mask_height as f32).floor() as i32)
            .clamp(0, mask_height as i32 - 1);
        let y1 = (((0.5 - min.y / CANVAS_HEIGHT) * mask_height as f32).ceil() as i32)
            .clamp(0, mask_height as i32 - 1);
        for y in y0..=y1 {
            for x in x0..=x1 {
                let point = Vec2::new(
                    (x as f32 + 0.5) / mask_width as f32 * CANVAS_WIDTH - CANVAS_WIDTH / 2.0,
                    CANVAS_HEIGHT / 2.0 - (y as f32 + 0.5) / mask_height as f32 * CANVAS_HEIGHT,
                );
                if rect.contains(point) {
                    let index = y as usize * mask_width as usize + x as usize;
                    if area.is_subtract {
                        subtract[index][phase] = xor_coverage(subtract[index][phase], coverage);
                    } else {
                        normal[index][phase] = union_coverage(normal[index][phase], coverage);
                    }
                }
            }
        }
    }

    let mut mask_data = vec![0_u8; pixel_count * 4];
    for index in 0..pixel_count {
        mask_data[index * 4] = normal[index][0].abs_diff(subtract[index][0]);
        mask_data[index * 4 + 1] = normal[index][1].abs_diff(subtract[index][1]);
        mask_data[index * 4 + 2] = normal[index][2].abs_diff(subtract[index][2]);
        mask_data[index * 4 + 3] = 255;
    }

    // The active channel is displaced in the post-process shader so the body,
    // border, glow, and warped scene all use the same field and frame time.
    let effect_data = build_effect_texture(&mask_data, mask_width, mask_height);

    if let Some(image) = images.get_mut(&composite.mask) {
        let extent = Extent3d {
            width: mask_width,
            height: mask_height,
            depth_or_array_layers: 1,
        };
        if image.texture_descriptor.size != extent {
            image.resize(extent);
        }
        image.data = Some(mask_data);
    }
    if let Some(image) = images.get_mut(&composite.effect) {
        let extent = Extent3d {
            width: mask_width * 2,
            height: mask_height * 2,
            depth_or_array_layers: 1,
        };
        if image.texture_descriptor.size != extent {
            image.resize(extent);
        }
        image.data = Some(effect_data);
    }
}

fn union_coverage(current: u8, added: u8) -> u8 {
    let current = current as u32;
    let added = added as u32;
    (added + current * (255 - added) / 255).min(255) as u8
}

fn xor_coverage(current: u8, added: u8) -> u8 {
    let current = current as i32;
    let added = added as i32;
    (current + added - 2 * current * added / 255).clamp(0, 255) as u8
}

fn dilate_3x3(source: &[u8], width: u32, height: u32) -> Vec<u8> {
    let mut output = vec![0_u8; source.len()];
    for y in 0..height as i32 {
        for x in 0..width as i32 {
            let mut value = 0_u8;
            for dy in -1..=1 {
                for dx in -1..=1 {
                    let sx = (x + dx).clamp(0, width as i32 - 1) as u32;
                    let sy = (y + dy).clamp(0, height as i32 - 1) as u32;
                    value = value.max(source[(sy * width + sx) as usize]);
                }
            }
            output[(y as u32 * width + x as u32) as usize] = value;
        }
    }
    output
}

fn build_effect_texture(mask: &[u8], mask_width: u32, mask_height: u32) -> Vec<u8> {
    let width = mask_width * 2;
    let height = mask_height * 2;
    let mut compose = vec![0_u8; (width * height) as usize];
    for y in 0..height {
        for x in 0..width {
            compose[(y * width + x) as usize] = mask[(((y / 2) * mask_width + x / 2) * 4) as usize];
        }
    }

    // EdgeMask is one 3x3 dilation minus ComposeRT.
    let first_dilation = dilate_3x3(&compose, width, height);
    let edge: Vec<u8> = first_dilation
        .iter()
        .zip(&compose)
        .map(|(expanded, base)| expanded.saturating_sub(*base))
        .collect();

    // GlowMask iteratively dilates and accumulates weighted rings. With the
    // official 0.01 threshold, radius-six's 0.003974687 pass is skipped.
    const WEIGHTS: [f32; 5] = [
        0.458_567_98,
        0.282_861_23,
        0.156_589_22,
        0.073_059_08,
        0.024_947_807,
    ];
    let mut previous = compose.clone();
    let mut glow = vec![0.0_f32; compose.len()];
    for weight in WEIGHTS {
        let expanded = dilate_3x3(&previous, width, height);
        for index in 0..glow.len() {
            let ring = expanded[index].saturating_sub(previous[index]) as f32 / 255.0;
            let outside = 1.0 - compose[index] as f32 / 255.0;
            glow[index] = (glow[index] + ring * outside * weight).clamp(0.0, 1.0);
        }
        previous = expanded;
    }

    let mut output = vec![0_u8; (width * height * 4) as usize];
    for index in 0..compose.len() {
        output[index * 4] = edge[index];
        output[index * 4 + 1] = (glow[index] * 255.0).round() as u8;
        output[index * 4 + 3] = 255;
    }
    output
}

#[cfg(test)]
mod tests {
    use super::{union_coverage, xor_coverage};

    #[test]
    fn coverage_composition_matches_normal_union_and_subtract_parity() {
        assert_eq!(union_coverage(0, 255), 255);
        assert_eq!(union_coverage(255, 128), 255);
        assert_eq!(xor_coverage(0, 255), 255);
        assert_eq!(xor_coverage(255, 255), 0);
        assert_eq!(xor_coverage(0, 128), 128);
    }
}
