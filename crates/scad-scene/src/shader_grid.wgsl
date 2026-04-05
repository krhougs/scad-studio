struct LightRaw {
    kind_flags: vec4<u32>,
    color_intensity: vec4<f32>,
    position_range: vec4<f32>,
    direction_spot: vec4<f32>,
    extra: vec4<f32>,
};

struct SceneUniform {
    view_proj: mat4x4<f32>,
    model: mat4x4<f32>,
    light_view_proj: mat4x4<f32>,
    eye_position: vec4<f32>,
    clip_plane: vec4<f32>,
    render_params: vec4<f32>,
    light_meta: vec4<u32>,
    render_config: vec4<u32>,
    lights: array<LightRaw, 4>,
};

@group(0) @binding(0)
var<uniform> scene: SceneUniform;
@group(0) @binding(1)
var shadow_map: texture_depth_2d;
@group(0) @binding(2)
var shadow_sampler: sampler_comparison;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) color: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) color: vec4<f32>,
    @location(2) light_clip_position: vec4<f32>,
};

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    output.world_position = input.position;
    output.clip_position = scene.view_proj * vec4<f32>(input.position, 1.0);
    output.color = input.color;
    output.light_clip_position = scene.light_view_proj * vec4<f32>(input.position, 1.0);
    return output;
}

fn sample_shadow(light_clip_position: vec4<f32>) -> f32 {
    if scene.render_params.y < 0.5 {
        return 1.0;
    }
    let ndc = light_clip_position.xyz / light_clip_position.w;
    if ndc.x < -1.0 || ndc.x > 1.0 || ndc.y < -1.0 || ndc.y > 1.0 || ndc.z > 1.0 {
        return 1.0;
    }
    let uv = vec2<f32>(ndc.x * 0.5 + 0.5, -ndc.y * 0.5 + 0.5);
    let depth_ref = ndc.z - 0.0015;
    return textureSampleCompareLevel(shadow_map, shadow_sampler, uv, depth_ref);
}

fn apply_fog(color: vec3<f32>, world_position: vec3<f32>) -> vec3<f32> {
    if scene.render_params.z <= 0.0 {
        return color;
    }
    let distance_to_eye = distance(scene.eye_position.xyz, world_position);
    let fog_factor = exp(-distance_to_eye * scene.render_params.z);
    let fog_color = vec3<f32>(0.07, 0.09, 0.12);
    return mix(fog_color, color, fog_factor);
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let distance_to_eye = distance(input.world_position.xz, scene.eye_position.xz);
    let fade = clamp(1.0 - distance_to_eye / 600.0, 0.12, 1.0);
    let shadow = mix(0.55, 1.0, sample_shadow(input.light_clip_position));
    return vec4<f32>(
        apply_fog(input.color.rgb * shadow, input.world_position),
        input.color.a * fade,
    );
}
