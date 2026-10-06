use crate::noise::NoiseComposite;
use crate::ChartTime;
use bevy::core_pipeline::{
    core_2d::graph::{Core2d, Node2d},
    FullscreenShader,
};
use bevy::ecs::query::QueryItem;
use bevy::prelude::*;
use bevy::render::{
    extract_component::{
        ComponentUniforms, DynamicUniformIndex, ExtractComponent, ExtractComponentPlugin,
        UniformComponentPlugin,
    },
    render_asset::RenderAssets,
    render_graph::{
        NodeRunError, RenderGraphContext, RenderGraphExt, RenderLabel, ViewNode, ViewNodeRunner,
    },
    render_resource::{
        binding_types::{sampler, texture_2d, uniform_buffer},
        *,
    },
    renderer::{RenderContext, RenderDevice},
    texture::GpuImage,
    view::ViewTarget,
    RenderApp, RenderStartup,
};

const SHADER_PATH: &str = "shaders/noise_distortion.wgsl";

/// Marks the camera whose rendered game image should receive the official-like domain distortion.
#[derive(Component)]
pub struct NoisePostProcessCamera;

#[derive(Component, Clone, ExtractComponent)]
struct NoisePostProcessTextures {
    mask: Handle<Image>,
    displacement: Handle<Image>,
}

#[derive(Component, Clone, Copy, Default, ExtractComponent, ShaderType)]
struct NoisePostProcessSettings {
    time: f32,
    strength: f32,
    pixel_scale: f32,
    _padding: f32,
}

pub(crate) struct NoisePostProcessPlugin;

impl Plugin for NoisePostProcessPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            ExtractComponentPlugin::<NoisePostProcessTextures>::default(),
            ExtractComponentPlugin::<NoisePostProcessSettings>::default(),
            UniformComponentPlugin::<NoisePostProcessSettings>::default(),
        ))
        .add_systems(PostStartup, attach_noise_post_process)
        .add_systems(Update, update_noise_post_process_time);

        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };
        render_app
            .add_systems(RenderStartup, init_pipeline)
            .add_render_graph_node::<ViewNodeRunner<NoisePostProcessNode>>(
                Core2d,
                NoisePostProcessLabel,
            )
            .add_render_graph_edges(
                Core2d,
                (
                    Node2d::Tonemapping,
                    NoisePostProcessLabel,
                    Node2d::EndMainPassPostProcessing,
                ),
            );
    }
}

fn attach_noise_post_process(
    mut commands: Commands,
    cameras: Query<
        Entity,
        (
            With<NoisePostProcessCamera>,
            Without<NoisePostProcessSettings>,
        ),
    >,
    composite: Res<NoiseComposite>,
) {
    for entity in &cameras {
        commands.entity(entity).insert((
            NoisePostProcessTextures {
                mask: composite.mask.clone(),
                displacement: composite.displacement.clone(),
            },
            NoisePostProcessSettings {
                strength: 0.015,
                pixel_scale: 6.0,
                ..default()
            },
        ));
    }
}

fn update_noise_post_process_time(
    time: Res<ChartTime>,
    mut settings: Query<&mut NoisePostProcessSettings>,
) {
    for mut settings in &mut settings {
        settings.time = time.0;
    }
}

#[derive(Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
struct NoisePostProcessLabel;

#[derive(Default)]
struct NoisePostProcessNode;

impl ViewNode for NoisePostProcessNode {
    type ViewQuery = (
        &'static ViewTarget,
        &'static NoisePostProcessSettings,
        &'static DynamicUniformIndex<NoisePostProcessSettings>,
        &'static NoisePostProcessTextures,
    );

    fn run(
        &self,
        _graph: &mut RenderGraphContext,
        render_context: &mut RenderContext,
        (view_target, _settings, settings_index, textures): QueryItem<Self::ViewQuery>,
        world: &World,
    ) -> Result<(), NodeRunError> {
        let pipeline = world.resource::<NoisePostProcessPipeline>();
        let pipeline_cache = world.resource::<PipelineCache>();
        let Some(render_pipeline) = pipeline_cache.get_render_pipeline(pipeline.pipeline_id) else {
            return Ok(());
        };
        let settings_uniforms = world.resource::<ComponentUniforms<NoisePostProcessSettings>>();
        let Some(settings_binding) = settings_uniforms.uniforms().binding() else {
            return Ok(());
        };
        let gpu_images = world.resource::<RenderAssets<GpuImage>>();
        let (Some(mask), Some(displacement)) = (
            gpu_images.get(&textures.mask),
            gpu_images.get(&textures.displacement),
        ) else {
            return Ok(());
        };

        let post_process = view_target.post_process_write();
        let bind_group = render_context.render_device().create_bind_group(
            "noise_post_process_bind_group",
            &pipeline_cache.get_bind_group_layout(&pipeline.layout),
            &BindGroupEntries::sequential((
                post_process.source,
                &pipeline.sampler,
                &mask.texture_view,
                &mask.sampler,
                &displacement.texture_view,
                &displacement.sampler,
                settings_binding.clone(),
            )),
        );
        let mut render_pass = render_context.begin_tracked_render_pass(RenderPassDescriptor {
            label: Some("noise_post_process_pass"),
            color_attachments: &[Some(RenderPassColorAttachment {
                view: post_process.destination,
                depth_slice: None,
                resolve_target: None,
                ops: Operations::default(),
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        render_pass.set_render_pipeline(render_pipeline);
        render_pass.set_bind_group(0, &bind_group, &[settings_index.index()]);
        render_pass.draw(0..3, 0..1);
        Ok(())
    }
}

#[derive(Resource)]
struct NoisePostProcessPipeline {
    layout: BindGroupLayoutDescriptor,
    sampler: Sampler,
    pipeline_id: CachedRenderPipelineId,
}

fn init_pipeline(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    asset_server: Res<AssetServer>,
    fullscreen_shader: Res<FullscreenShader>,
    pipeline_cache: Res<PipelineCache>,
) {
    let layout = BindGroupLayoutDescriptor::new(
        "noise_post_process_bind_group_layout",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::FRAGMENT,
            (
                texture_2d(TextureSampleType::Float { filterable: true }),
                sampler(SamplerBindingType::Filtering),
                texture_2d(TextureSampleType::Float { filterable: true }),
                sampler(SamplerBindingType::Filtering),
                texture_2d(TextureSampleType::Float { filterable: true }),
                sampler(SamplerBindingType::Filtering),
                uniform_buffer::<NoisePostProcessSettings>(true),
            ),
        ),
    );
    let sampler = render_device.create_sampler(&SamplerDescriptor::default());
    let pipeline_id = pipeline_cache.queue_render_pipeline(RenderPipelineDescriptor {
        label: Some("noise_post_process_pipeline".into()),
        layout: vec![layout.clone()],
        vertex: fullscreen_shader.to_vertex_state(),
        fragment: Some(FragmentState {
            shader: asset_server.load(SHADER_PATH),
            targets: vec![Some(ColorTargetState {
                format: TextureFormat::bevy_default(),
                blend: None,
                write_mask: ColorWrites::ALL,
            })],
            ..default()
        }),
        ..default()
    });
    commands.insert_resource(NoisePostProcessPipeline {
        layout,
        sampler,
        pipeline_id,
    });
}
