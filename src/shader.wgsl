struct SceneUniform {
    view_proj: mat4x4<f32>,
    model: mat4x4<f32>,
    eye_position: vec4<f32>,
    light_direction: vec4<f32>,
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

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let normal = normalize(input.world_normal);
    let light = normalize(-scene.light_direction.xyz);
    let view_dir = normalize(scene.eye_position.xyz - input.world_position);
    let half_vector = normalize(light + view_dir);

    let ambient = 0.18;
    let diffuse = max(dot(normal, light), 0.0);
    let specular = pow(max(dot(normal, half_vector), 0.0), 48.0) * 0.35;

    let base = vec3<f32>(0.74, 0.78, 0.84);
    let shaded = base * (ambient + diffuse * 0.82) + vec3<f32>(specular);
    return vec4<f32>(shaded, 1.0);
}
