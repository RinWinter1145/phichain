#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

// Phigros 9 BlockRender's DisabledBlock, ReadyBlock, and ActiveBlock materials.
// The ComposeRT and EffectRT stages are generated at their official resolutions
// by noise.rs and supplied as mask_texture and effect_texture.
struct NoisePostProcessSettings {
    time: f32,
    strength: f32,
    pixel_scale: f32,
    padding: f32,
    viewport_origin: vec2<f32>,
    viewport_size: vec2<f32>,
};

// Shared gain for the domain silhouette and the scene sampled through it.
// Keeping one value here preserves their motion lock while making the entire
// field tear more strongly.
const DOMAIN_DISTORTION_GAIN: f32 = 1.30;

@group(0) @binding(0) var source_texture: texture_2d<f32>;
@group(0) @binding(1) var source_sampler: sampler;
@group(0) @binding(2) var mask_texture: texture_2d<f32>;
@group(0) @binding(3) var mask_sampler: sampler;
@group(0) @binding(4) var effect_texture: texture_2d<f32>;
@group(0) @binding(5) var effect_sampler: sampler;
@group(0) @binding(6) var displacement_texture: texture_2d<f32>;
@group(0) @binding(7) var displacement_sampler: sampler;
@group(0) @binding(8) var spark_texture: texture_2d<f32>;
@group(0) @binding(9) var spark_sampler: sampler;
@group(0) @binding(10) var<uniform> settings: NoisePostProcessSettings;

fn pixel_center(uv: vec2<f32>, dimensions: vec2<f32>, scale: f32) -> vec2<f32> {
    let size = max(scale, 1.0);
    return (floor(uv * dimensions / size) * size + vec2(size * 0.5)) / dimensions;
}

// Unity's _Time.x is seconds / 20. The official shaders sample BlockNoise1
// along the normalized direction and its perpendicular.
fn displacement_pair(
    uv: vec2<f32>,
    dimensions: vec2<f32>,
    tiling: vec2<f32>,
    speed: f32,
    quantize_scale: f32,
) -> vec2<f32> {
    let direction = normalize(vec2(1.0, 1.0));
    let perpendicular = vec2(-direction.y, direction.x);
    let motion = settings.time * 0.05 * speed;
    var first_uv = uv * tiling + direction * motion;
    var second_uv = uv * tiling + perpendicular * motion;
    if quantize_scale > 0.0 {
        first_uv = pixel_center(first_uv, dimensions, quantize_scale);
        second_uv = pixel_center(second_uv, dimensions, quantize_scale);
    }
    let first = textureSample(displacement_texture, displacement_sampler, first_uv).r;
    let second = textureSample(displacement_texture, displacement_sampler, second_uv).r;
    let average = (first + second) * 0.5;
    return vec2(average - 0.5, average);
}

fn rgb_to_hsv(color: vec3<f32>) -> vec3<f32> {
    let maximum = max(color.r, max(color.g, color.b));
    let minimum = min(color.r, min(color.g, color.b));
    let chroma = maximum - minimum;
    var hue = 0.0;
    if chroma > 1e-10 {
        if maximum == color.r {
            hue = fract((color.g - color.b) / (6.0 * chroma));
        } else if maximum == color.g {
            hue = (color.b - color.r) / (6.0 * chroma) + 1.0 / 3.0;
        } else {
            hue = (color.r - color.g) / (6.0 * chroma) + 2.0 / 3.0;
        }
    }
    return vec3(abs(hue), chroma / (maximum + 1e-10), maximum);
}

fn hsv_to_rgb(hsv: vec3<f32>) -> vec3<f32> {
    let p = clamp(
        abs(fract(hsv.xxx + vec3(1.0, 2.0 / 3.0, 1.0 / 3.0)) * 6.0 - 3.0) - 1.0,
        vec3(0.0),
        vec3(1.0),
    );
    return hsv.z * mix(vec3(1.0), p, hsv.y);
}

fn overlay(base: vec3<f32>, blend: vec3<f32>) -> vec3<f32> {
    let multiply = 2.0 * base * blend;
    let screen = 1.0 - 2.0 * (1.0 - base) * (1.0 - blend);
    return clamp(select(multiply, screen, base >= vec3(0.5)), vec3(0.0), vec3(1.0));
}

fn linear_to_srgb_channel(value: f32) -> f32 {
    if value <= 0.0031308 {
        return value * 12.92;
    }
    return 1.055 * pow(max(value, 0.0), 1.0 / 2.4) - 0.055;
}

fn srgb_to_linear_channel(value: f32) -> f32 {
    if value <= 0.04045 {
        return value / 12.92;
    }
    return pow((value + 0.055) / 1.055, 2.4);
}

fn linear_to_srgb(color: vec3<f32>) -> vec3<f32> {
    return vec3(
        linear_to_srgb_channel(color.r),
        linear_to_srgb_channel(color.g),
        linear_to_srgb_channel(color.b),
    );
}

fn srgb_to_linear(color: vec3<f32>) -> vec3<f32> {
    return vec3(
        srgb_to_linear_channel(color.r),
        srgb_to_linear_channel(color.g),
        srgb_to_linear_channel(color.b),
    );
}

// Unlit/ActiveBlock (Shader 38), with _TouchPosCount=0. Touch feedback is a
// separate BlockRender input and does not alter the chart-authored domain.
fn active_effect(
    uv: vec2<f32>,
    source_uv_origin: vec2<f32>,
    source_uv_size: vec2<f32>,
    viewport_dimensions: vec2<f32>,
    compose: f32,
    edge: f32,
    glow: f32,
    pair: vec2<f32>,
) -> vec4<f32> {
    let local_scene_uv = pixel_center(uv, viewport_dimensions, settings.pixel_scale)
        + vec2(pair.x * settings.strength * DOMAIN_DISTORTION_GAIN);
    let scene_uv = source_uv_origin + local_scene_uv * source_uv_size;
    // Phigros ships with PlayerSettings.m_ActiveColorSpace = 0 (Gamma).
    // Bevy's sRGB render target is decoded on sample, so convert back to the
    // gamma-domain values in which the official shader performs all math.
    let scene = linear_to_srgb(textureSample(source_texture, source_sampler, scene_uv).rgb);

    let spark_uv = uv * vec2(3.0, 1.2) + vec2(pair.x * 2.39);
    let spark_sample = textureSample(spark_texture, spark_sampler, spark_uv).r;
    let spark_tint = vec3(1.0, 0.2849056721, 0.2849056721);
    let spark = pair.y * spark_sample * spark_tint * 5.69;

    // The decompiled shader converts SceneColor RGB -> HSV, offsets H/S/V by
    // the three spark channels, converts back, then applies Overlay blending.
    let shifted_scene = hsv_to_rgb(rgb_to_hsv(scene) + spark * 0.2);
    let sparked_scene = overlay(shifted_scene, spark);
    let fill_base = vec3(0.7132074833, 0.2354929596, 0.2354929596)
        - vec3(pair.y * 0.411);
    let fill = mix(fill_base, sparked_scene, 0.667);

    // Keep the body translucent in Bevy's post-tonemap pass. Directly copying
    // Shader 38's HDR additive glow here clips to solid red because the Unity
    // effect is composed at a different stage of the frame.
    let body_alpha = compose * 0.667;
    let edge_alpha = edge * 0.8;
    let occupied = clamp(compose + edge, 0.0, 1.0);
    let outer_glow = glow * (1.0 - occupied) * 0.20;
    // A low-opacity, displacement-driven veil supplies the mist component
    // without burying the warped scene beneath a flat red layer.
    let mist = compose * (0.05 + pair.y * 0.11);
    let rgb = fill * compose
        + vec3(1.0, 0.12, 0.12) * edge_alpha
        + vec3(0.62, 0.055, 0.055) * mist
        + vec3(1.0, 0.10, 0.10) * outer_glow;
    let alpha = clamp(body_alpha + mist * 0.25 + edge_alpha + outer_glow, 0.0, 1.0);
    return vec4(rgb, alpha);
}

fn active_pixel_edge(uv: vec2<f32>, viewport_dimensions: vec2<f32>, center: f32) -> f32 {
    let pixel = 1.0 / viewport_dimensions;
    var expanded = center;
    expanded = max(expanded, textureSample(mask_texture, mask_sampler, uv + vec2(pixel.x, 0.0)).r);
    expanded = max(expanded, textureSample(mask_texture, mask_sampler, uv - vec2(pixel.x, 0.0)).r);
    expanded = max(expanded, textureSample(mask_texture, mask_sampler, uv + vec2(0.0, pixel.y)).r);
    expanded = max(expanded, textureSample(mask_texture, mask_sampler, uv - vec2(0.0, pixel.y)).r);
    expanded = max(expanded, textureSample(mask_texture, mask_sampler, uv + pixel).r);
    expanded = max(expanded, textureSample(mask_texture, mask_sampler, uv - pixel).r);
    expanded = max(expanded, textureSample(mask_texture, mask_sampler, uv + vec2(pixel.x, -pixel.y)).r);
    expanded = max(expanded, textureSample(mask_texture, mask_sampler, uv + vec2(-pixel.x, pixel.y)).r);
    return clamp(expanded - center, 0.0, 1.0);
}

// Unlit/DisabledBlock (Shader 39). Its ShaderLab state is Blend One One.
fn disabled_effect(uv: vec2<f32>, dimensions: vec2<f32>, coverage: f32) -> vec3<f32> {
    if coverage <= 0.0001 {
        return vec3(0.0);
    }
    let pair = displacement_pair(uv, dimensions, vec2(0.5, 0.2), 0.3, 0.0);
    let spark_uv = uv * vec2(3.0, 1.2) + vec2(pair.x * 2.29);
    let spark_sample = textureSample(spark_texture, spark_sampler, spark_uv).r;
    let fill = vec3(0.4970000088, 0.1376689821, 0.1376689821) * 0.4;
    let spark_tint = vec3(0.3113207817, 0.0778301954, 0.0778301954);
    return coverage * (fill + pair.y * spark_sample * spark_tint * 3.5);
}

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let source_uv = in.uv;
    let source = textureSample(source_texture, source_sampler, source_uv);
    let source_dimensions = vec2<f32>(textureDimensions(source_texture));
    var viewport_origin = settings.viewport_origin;
    var viewport_dimensions = settings.viewport_size;
    if viewport_dimensions.x <= 0.0 || viewport_dimensions.y <= 0.0 {
        viewport_origin = vec2(0.0);
        viewport_dimensions = source_dimensions;
    }
    let pixel_position = source_uv * source_dimensions;
    let viewport_max = viewport_origin + viewport_dimensions;
    if pixel_position.x < viewport_origin.x || pixel_position.y < viewport_origin.y
        || pixel_position.x >= viewport_max.x || pixel_position.y >= viewport_max.y {
        return source;
    }

    // ComposeRT/EffectRT cover the camera viewport, whereas Bevy's post-process
    // source covers the complete editor window.
    let uv = (pixel_position - viewport_origin) / viewport_dimensions;
    let source_uv_origin = viewport_origin / source_dimensions;
    let source_uv_size = viewport_dimensions / source_dimensions;
    // One displacement field drives the active mask, its pixel outline, and
    // the sampled scene. This prevents the border from sliding independently
    // over the turbulence inside the field.
    let domain_pair = displacement_pair(
        uv,
        viewport_dimensions,
        vec2(0.8, 0.3),
        3.0,
        settings.pixel_scale,
    );
    let domain_uv = uv + vec2(domain_pair.x * 0.10 * DOMAIN_DISTORTION_GAIN);
    let phases = textureSample(mask_texture, mask_sampler, uv).rgb;
    let active_coverage = textureSample(mask_texture, mask_sampler, domain_uv).r;
    let glow = textureSample(effect_texture, effect_sampler, domain_uv).g;
    // EdgeMask's visual output is a one-display-pixel dilation. Keeping the
    // quarter-resolution EffectRT edge directly made it four pixels thick in
    // the editor preview; derive the final hard ring from ComposeRT instead.
    let edge = active_pixel_edge(domain_uv, viewport_dimensions, active_coverage);
    let ready = phases.g;
    let disabled = phases.b;

    // DisabledBlock and ReadyBlock are additive layers in the official canvas.
    var color = linear_to_srgb(source.rgb)
        + disabled_effect(uv, viewport_dimensions, disabled);
    if ready > 0.0001 {
        let shine = (sin(settings.time * 37.9) * 0.5 + 1.0) * 0.12;
        color += vec3(shine * ready);
    }

    // ActiveBlock uses Blend One OneMinusSrcAlpha. The shader output is
    // premultiplied by ComposeRT/EffectRT, so reproduce that exact equation.
    if active_coverage + edge + glow > 0.0001 {
        let effect = active_effect(
            uv,
            source_uv_origin,
            source_uv_size,
            viewport_dimensions,
            active_coverage,
            edge,
            glow,
            domain_pair,
        );
        color = effect.rgb + color * (1.0 - clamp(effect.a, 0.0, 1.0));
    }
    // Convert the official gamma-space result back to linear before Bevy's
    // sRGB target encodes it for display.
    return vec4(srgb_to_linear(clamp(color, vec3(0.0), vec3(1.0))), source.a);
}
