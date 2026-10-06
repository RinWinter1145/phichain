use crate::noise_postprocess::NoisePostProcessPlugin;
use crate::{ChartTime, GameSet, GameViewport};
use bevy::asset::RenderAssetUsages;
use bevy::picking::Pickable;
use bevy::prelude::*;
use bevy::reflect::TypePath;
use bevy::render::render_resource::{AsBindGroup, Extent3d, TextureDimension, TextureFormat};
use bevy::shader::ShaderRef;
use bevy::sprite_render::{AlphaMode2d, Material2d, Material2dPlugin};
use phichain_chart::bpm_list::BpmList;
use phichain_chart::constants::{CANVAS_HEIGHT, CANVAS_WIDTH};
use phichain_chart::noise::NoiseArea;

const NOISE_LAYER: f32 = 25.0;
const MASK_WIDTH: u32 = 450;
const MASK_HEIGHT: u32 = 300;

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
struct NoiseCompositeMaterial {
    #[texture(0)]
    #[sampler(1)]
    mask: Handle<Image>,
    #[texture(2)]
    #[sampler(3)]
    noise: Handle<Image>,
    #[texture(4)]
    #[sampler(5)]
    spark: Handle<Image>,
    #[texture(6)]
    #[sampler(7)]
    displacement: Handle<Image>,
    #[uniform(8)]
    params: Vec4,
}

impl Material2d for NoiseCompositeMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/noise_composite.wgsl".into()
    }

    fn alpha_mode(&self) -> AlphaMode2d {
        AlphaMode2d::Blend
    }
}

#[derive(Resource)]
pub(crate) struct NoiseComposite {
    pub(crate) mask: Handle<Image>,
    pub(crate) displacement: Handle<Image>,
    material: Handle<NoiseCompositeMaterial>,
    entity: Entity,
}

#[derive(Component, Default, Debug, Clone, Copy, Ord, PartialOrd, Eq, PartialEq)]
pub struct NoiseAreaOrder(pub usize);

pub struct NoiseAreaPlugin;

impl Plugin for NoiseAreaPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(Material2dPlugin::<NoiseCompositeMaterial>::default())
            .add_plugins(NoisePostProcessPlugin)
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
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<NoiseCompositeMaterial>>,
    asset_server: Res<AssetServer>,
) {
    let mask = images.add(Image::new_fill(
        Extent3d {
            width: MASK_WIDTH,
            height: MASK_HEIGHT,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &[0, 0, 0, 0],
        TextureFormat::Rgba8Unorm,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    ));
    let displacement = asset_server.load("noise/BlockNoise1.png");
    let material = materials.add(NoiseCompositeMaterial {
        mask: mask.clone(),
        noise: asset_server.load("noise/FD_Noise_00000.png"),
        spark: asset_server.load("noise/PointNoise.png"),
        displacement: displacement.clone(),
        params: Vec4::ZERO,
    });
    let entity = commands
        .spawn((
            Mesh2d(meshes.add(Rectangle::default())),
            MeshMaterial2d(material.clone()),
            Transform::from_xyz(0.0, 0.0, NOISE_LAYER),
            Pickable::IGNORE,
        ))
        .id();
    commands.insert_resource(NoiseComposite {
        mask,
        displacement,
        material,
        entity,
    });
}

fn add_noise_area_preview(
    add: On<Add, NoiseArea>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
) {
    commands.entity(add.entity).insert((
        Sprite {
            image: asset_server.load("noise/BlockNoise1.png"),
            color: Color::NONE,
            custom_size: Some(Vec2::ONE),
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, NOISE_LAYER),
    ));
}

fn update_noise_area_previews(
    bpm_list: Option<Res<BpmList>>,
    chart_time: Res<ChartTime>,
    viewport: Res<GameViewport>,
    mut previews: Query<(Entity, &NoiseArea, &mut Sprite, &mut Transform)>,
) {
    let Some(bpm_list) = bpm_list else {
        return;
    };

    for (_entity, area, mut sprite, mut transform) in &mut previews {
        if !area.is_visible(chart_time.0, &bpm_list) {
            sprite.color = Color::NONE;
            continue;
        }

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
    areas: Query<&NoiseArea>,
    bpm_list: Option<Res<BpmList>>,
    chart_time: Res<ChartTime>,
    viewport: Res<GameViewport>,
    composite: Res<NoiseComposite>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<NoiseCompositeMaterial>>,
    mut transforms: Query<&mut Transform>,
) {
    let Some(bpm_list) = bpm_list else {
        return;
    };
    let pixel_count = (MASK_WIDTH * MASK_HEIGHT) as usize;
    let mut normal = vec![0_u8; pixel_count];
    let mut subtract = vec![0_u8; pixel_count];

    for area in &areas {
        if !area.is_visible(chart_time.0, &bpm_list) {
            continue;
        }
        let phase = if area.is_active(chart_time.0, &bpm_list) {
            1_u8
        } else {
            let enable = bpm_list.time_at(area.enable_beat);
            if chart_time.0 < enable && enable - chart_time.0 <= 0.5 {
                2_u8
            } else {
                4_u8
            }
        };
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
        let x0 = (((min.x / CANVAS_WIDTH + 0.5) * MASK_WIDTH as f32).floor() as i32)
            .clamp(0, MASK_WIDTH as i32 - 1);
        let x1 = (((max.x / CANVAS_WIDTH + 0.5) * MASK_WIDTH as f32).ceil() as i32)
            .clamp(0, MASK_WIDTH as i32 - 1);
        let y0 = (((0.5 - max.y / CANVAS_HEIGHT) * MASK_HEIGHT as f32).floor() as i32)
            .clamp(0, MASK_HEIGHT as i32 - 1);
        let y1 = (((0.5 - min.y / CANVAS_HEIGHT) * MASK_HEIGHT as f32).ceil() as i32)
            .clamp(0, MASK_HEIGHT as i32 - 1);
        for y in y0..=y1 {
            for x in x0..=x1 {
                let point = Vec2::new(
                    (x as f32 + 0.5) / MASK_WIDTH as f32 * CANVAS_WIDTH - CANVAS_WIDTH / 2.0,
                    CANVAS_HEIGHT / 2.0 - (y as f32 + 0.5) / MASK_HEIGHT as f32 * CANVAS_HEIGHT,
                );
                if rect.contains(point) {
                    let index = y as usize * MASK_WIDTH as usize + x as usize;
                    if area.is_subtract {
                        subtract[index] ^= phase;
                    } else {
                        normal[index] |= phase;
                    }
                }
            }
        }
    }

    if let Some(image) = images.get_mut(&composite.mask) {
        let mut data = vec![0_u8; pixel_count * 4];
        for index in 0..pixel_count {
            let phases = normal[index] ^ subtract[index];
            data[index * 4] = if phases & 1 != 0 { 255 } else { 0 };
            data[index * 4 + 1] = if phases & 2 != 0 { 255 } else { 0 };
            data[index * 4 + 2] = if phases & 4 != 0 { 255 } else { 0 };
            data[index * 4 + 3] = 255;
        }
        image.data = Some(data);
    }
    if let Some(material) = materials.get_mut(&composite.material) {
        material.params.x = chart_time.0;
    }
    if let Ok(mut transform) = transforms.get_mut(composite.entity) {
        transform.scale = Vec3::new(viewport.0.width(), viewport.0.height(), 1.0);
    }
}
