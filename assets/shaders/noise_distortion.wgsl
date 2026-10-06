#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

struct NoisePostProcessSettings {
    time: f32,
    strength: f32,
    pixel_scale: f32,
    padding: f32,
};

@group(0) @binding(0) var source_texture: texture_2d<f32>;
@group(0) @binding(1) var source_sampler: sampler;
@group(0) @binding(2) var mask_texture: texture_2d<f32>;
@group(0) @binding(3) var mask_sampler: sampler;
@group(0) @binding(4) var displacement_texture: texture_2d<f32>;
@group(0) @binding(5) var displacement_sampler: sampler;
@group(0) @binding(6) var<uniform> settings: NoisePostProcessSettings;

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let uv = in.uv;
    let source = textureSample(source_texture, source_sampler, uv);
    let phases = textureSample(mask_texture, mask_sampler, uv).rgb;
    let coverage = step(0.5, max(phases.r, max(phases.g, phases.b)));
    if coverage < 0.5 {
        return source;
    }

    let displacement_uv = fract(uv * vec2(0.8, 0.3) + vec2(settings.time * 0.15, settings.time * 0.07));
    let displacement = textureSample(displacement_texture, displacement_sampler, displacement_uv).rg * 2.0 - 1.0;
    let dimensions = vec2<f32>(textureDimensions(source_texture));
    let warped_uv = clamp(uv + displacement * settings.strength, vec2(0.0), vec2(1.0));
    let pixelated_uv = (floor(warped_uv * dimensions / settings.pixel_scale) + 0.5) * settings.pixel_scale / dimensions;
    let distorted = textureSample(source_texture, source_sampler, pixelated_uv);
    return mix(source, distorted, coverage * 0.82);
}
