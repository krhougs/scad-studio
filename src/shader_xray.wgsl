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

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) world_normal: vec3<f32>,
};

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    let world_position = scene.model * vec4<f32>(input.position, 1.0);
    let world_normal = normalize((scene.model * vec4<f32>(input.normal, 0.0)).xyz);
    var output: VertexOutput;
    output.clip_position = scene.view_proj * world_position;
    output.world_position = world_position.xyz;
    output.world_normal = world_normal;
    return output;
}

fn base_color(normal: vec3<f32>) -> vec3<f32> {
    if scene.render_config.x == 1u {
        return 0.35 + abs(normal) * vec3<f32>(0.5, 0.42, 0.38);
    }
    return vec3<f32>(0.74, 0.78, 0.84);
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
    let light = normalize(-scene.lights[1].direction_spot.xyz);
    let view_dir = normalize(scene.eye_position.xyz - input.world_position);
    let half_vector = normalize(light + view_dir);
    let fresnel = pow(1.0 - max(dot(normal, view_dir), 0.0), 2.6);

    let diffuse = max(dot(normal, light), 0.0);
    let specular = pow(max(dot(normal, half_vector), 0.0), 32.0) * 0.2;
    let shaded = base_color(normal) * (0.16 + diffuse * 0.55)
        + vec3<f32>(specular)
        + vec3<f32>(fresnel * 0.45);
    return vec4<f32>(apply_fog(shaded, input.world_position), scene.render_params.x);
}
