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
    @location(1) normal: vec3<f32>,
    @location(2) color: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) model_color: vec4<f32>,
    @location(3) light_clip_position: vec4<f32>,
};

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    let world_position = scene.model * vec4<f32>(input.position, 1.0);
    let world_normal = normalize((scene.model * vec4<f32>(input.normal, 0.0)).xyz);
    var output: VertexOutput;
    output.clip_position = scene.view_proj * world_position;
    output.world_position = world_position.xyz;
    output.world_normal = world_normal;
    output.model_color = input.color;
    output.light_clip_position = scene.light_view_proj * world_position;
    return output;
}

fn base_color(normal: vec3<f32>, model_color: vec4<f32>) -> vec3<f32> {
    if scene.render_config.x == 1u && model_color.a >= 0.0 {
        return model_color.rgb;
    }
    if scene.render_config.x == 1u {
        return 0.32 + abs(normal) * vec3<f32>(0.52, 0.4, 0.36);
    }
    return vec3<f32>(0.74, 0.78, 0.84);
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
    var sum = 0.0;
    let texel = 1.0 / 1024.0;
    for (var x = -1; x <= 1; x = x + 1) {
        for (var y = -1; y <= 1; y = y + 1) {
            let offset = vec2<f32>(f32(x), f32(y)) * texel;
            sum += textureSampleCompareLevel(
                shadow_map,
                shadow_sampler,
                uv + offset,
                depth_ref,
            );
        }
    }
    return sum / 9.0;
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

fn clip_discard(world_position: vec3<f32>) {
    if scene.render_config.y == 0u {
        return;
    }
    if dot(scene.clip_plane.xyz, world_position) - scene.clip_plane.w > 0.0 {
        discard;
    }
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    clip_discard(input.world_position);
    let normal = normalize(input.world_normal);
    let view_dir = normalize(scene.eye_position.xyz - input.world_position);
    let base = base_color(normal, input.model_color);
    var shaded = vec3<f32>(0.0);
    let shadow = sample_shadow(input.light_clip_position);
    for (var index = 0u; index < scene.light_meta.x; index = index + 1u) {
        let light = scene.lights[index];
        let kind = light.kind_flags.x;
        let intensity = light.color_intensity.w;
        let color = light.color_intensity.xyz;
        if kind == 0u {
            shaded += base * color * intensity;
            continue;
        }
        var light_dir = vec3<f32>(0.0);
        var attenuation = 1.0;
        if kind == 1u {
            light_dir = normalize(-light.direction_spot.xyz);
        } else if kind == 2u {
            let to_light = light.position_range.xyz - input.world_position;
            let distance_to_light = length(to_light);
            light_dir = normalize(to_light);
            attenuation = max(1.0 - distance_to_light / max(light.position_range.w, 0.001), 0.0);
            let spot_cos = dot(normalize(-light.direction_spot.xyz), light_dir);
            let cone = smoothstep(light.extra.x, light.direction_spot.w, spot_cos);
            attenuation *= cone;
        } else {
            let to_light = light.position_range.xyz - input.world_position;
            let distance_to_light = length(to_light);
            light_dir = normalize(to_light);
            attenuation = max(1.0 - distance_to_light / max(light.position_range.w, 0.001), 0.0);
        }
        let half_vector = normalize(light_dir + view_dir);
        let diffuse = max(dot(normal, light_dir), 0.0);
        let specular = pow(max(dot(normal, half_vector), 0.0), 48.0) * 0.35;
        let shadow_factor = select(1.0, shadow, light.kind_flags.y == 1u);
        shaded += base * color * (diffuse * intensity * attenuation * shadow_factor);
        shaded += color * (specular * intensity * attenuation * shadow_factor);
    }
    return vec4<f32>(apply_fog(shaded, input.world_position), scene.render_params.x);
}
