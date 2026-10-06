#import bevy_sprite::mesh2d_vertex_output::VertexOutput

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var mask_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var mask_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var noise_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(3) var noise_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(4) var spark_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(5) var spark_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(6) var displacement_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(7) var displacement_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(8) var<uniform> params: vec4<f32>;

fn mask_at(uv: vec2<f32>) -> vec3<f32> {
    return textureSample(mask_texture, mask_sampler, clamp(uv, vec2(0.0), vec2(1.0))).rgb;
}

@fragment
fn fragment(mesh: VertexOutput) -> @location(0) vec4<f32> {
    let uv = mesh.uv;
    let mask = mask_at(uv);
    let active_phase = step(0.5, mask.r);
    let ready_phase = step(0.5, mask.g) * (1.0 - active_phase);
    let disabled_phase = step(0.5, mask.b) * (1.0 - active_phase) * (1.0 - ready_phase);
    let coverage = max(active_phase, max(ready_phase, disabled_phase));
    if coverage < 0.5 {
        discard;
    }

    let dimensions = vec2<f32>(textureDimensions(mask_texture));
    let texel = 1.0 / dimensions;
    let neighbor = min(
        min(max(max(mask_at(uv + vec2(texel.x, 0.0)).r, mask_at(uv + vec2(texel.x, 0.0)).g), mask_at(uv + vec2(texel.x, 0.0)).b),
            max(max(mask_at(uv - vec2(texel.x, 0.0)).r, mask_at(uv - vec2(texel.x, 0.0)).g), mask_at(uv - vec2(texel.x, 0.0)).b)),
        min(max(max(mask_at(uv + vec2(0.0, texel.y)).r, mask_at(uv + vec2(0.0, texel.y)).g), mask_at(uv + vec2(0.0, texel.y)).b),
            max(max(mask_at(uv - vec2(0.0, texel.y)).r, mask_at(uv - vec2(0.0, texel.y)).g), mask_at(uv - vec2(0.0, texel.y)).b))
    );
    let edge = coverage * (1.0 - step(0.5, neighbor));

    let time = params.x;
    let displacement_uv = fract(uv * vec2(0.8, 0.3) + vec2(time * 0.15, time * 0.07));
    let displacement = textureSample(displacement_texture, displacement_sampler, displacement_uv).rg * 2.0 - 1.0;
    let displaced_uv = fract(uv * vec2(1.5, 1.46) + vec2(time * 0.03, time * 0.018) + displacement * 0.015);
    let noise = textureSample(noise_texture, noise_sampler, displaced_uv).rgb;
    let spark_uv = fract(uv * vec2(3.0, 1.2) + vec2(time * 0.12, 0.0) + displacement * 0.02);
    let spark = textureSample(spark_texture, spark_sampler, spark_uv).r;

    let active_fill = vec3(0.7132075, 0.2354930, 0.2354930) * (0.72 + noise.r * 0.28);
    let disabled_fill = vec3(0.497, 0.137669, 0.137669) + spark * vec3(0.10, 0.018, 0.018);
    let shine = smoothstep(0.44, 0.5, 1.0 - abs(fract((uv.x + uv.y) * 1.4 + time * 0.379) - 0.5) * 2.0);
    let ready_fill = mix(disabled_fill, vec3(1.0), shine * 0.55);

    var color = active_fill * active_phase + ready_fill * ready_phase + disabled_fill * disabled_phase;
    var alpha = 0.667 * active_phase + 0.62 * ready_phase + 0.40 * disabled_phase;
    color = mix(color, vec3(1.0, 0.3301886, 0.3301886), edge * 0.8);
    alpha = max(alpha, edge * 0.8);
    return vec4(color, alpha);
}
