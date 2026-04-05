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
    @location(1) color: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    output.clip_position = scene.view_proj * vec4<f32>(input.position, 1.0);
    output.color = input.color;
    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    return input.color;
}
