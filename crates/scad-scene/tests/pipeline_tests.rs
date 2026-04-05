use egui_wgpu::wgpu;
use scad_scene::pipeline::{
    blend_state_for, clip_plane_enabled_flag, depth_stencil_format, pipeline_alpha_for,
    pipeline_color_mode, pipeline_fog_density, pipeline_specular_strength, polygon_mode_for,
    requested_device_features, resolve_render_mode, section_fill_stencil_compare,
    solid_transparent_blend_state, solid_transparent_depth_write_enabled, stencil_depth_compare,
    vertex_buffer_layout,
};
use scad_scene::{ColorMode, RenderMode, Vertex, transparent_index_buffer_usage};

#[test]
fn wireframe_mode_falls_back_to_solid_when_gpu_feature_is_missing() {
    let resolved = resolve_render_mode(RenderMode::Wireframe, wgpu::Features::empty());

    assert_eq!(resolved, RenderMode::Solid);
}

#[test]
fn wireframe_mode_is_kept_when_gpu_feature_is_available() {
    let resolved = resolve_render_mode(RenderMode::Wireframe, wgpu::Features::POLYGON_MODE_LINE);

    assert_eq!(resolved, RenderMode::Wireframe);
}

#[test]
fn polygon_mode_for_wireframe_uses_line_mode() {
    let mode = polygon_mode_for(RenderMode::Wireframe);

    assert_eq!(mode, wgpu::PolygonMode::Line);
}

#[test]
fn xray_pipeline_uses_alpha_blending_and_translucent_alpha() {
    let blend = blend_state_for(RenderMode::XRay);

    assert_eq!(blend, Some(wgpu::BlendState::ALPHA_BLENDING));
    assert!(pipeline_alpha_for(RenderMode::XRay) < 1.0);
}

#[test]
fn color_mode_is_forwarded_to_uniform_flag() {
    assert_eq!(pipeline_color_mode(ColorMode::Mono), 0);
    assert_eq!(pipeline_color_mode(ColorMode::Color), 1);
}

#[test]
fn requested_device_features_enable_line_mode_when_adapter_supports_it() {
    let requested = requested_device_features(
        wgpu::Features::POLYGON_MODE_LINE
            | wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES,
    );

    assert!(requested.contains(wgpu::Features::POLYGON_MODE_LINE));
}

#[test]
fn requested_device_features_stay_empty_when_adapter_lacks_line_mode() {
    let requested =
        requested_device_features(wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES);

    assert_eq!(requested, wgpu::Features::empty());
}

#[test]
fn fog_density_is_zero_when_disabled_and_default_when_enabled() {
    assert_eq!(pipeline_fog_density(false), 0.0);
    assert_eq!(pipeline_fog_density(true), 0.01);
}

#[test]
fn color_mode_uses_weaker_specular_strength_than_mono_mode() {
    assert!(
        pipeline_specular_strength(ColorMode::Color) < pipeline_specular_strength(ColorMode::Mono)
    );
}

#[test]
fn color_mode_disables_specular_strength_for_color_surfaces() {
    assert_eq!(pipeline_specular_strength(ColorMode::Color), 0.0);
}

#[test]
fn solid_transparent_pipeline_uses_alpha_blend_without_depth_write() {
    assert_eq!(
        solid_transparent_blend_state(),
        Some(wgpu::BlendState::ALPHA_BLENDING)
    );
    assert!(!solid_transparent_depth_write_enabled());
}

#[test]
fn transparent_index_buffer_usage_supports_copy_dst_updates() {
    let usage = transparent_index_buffer_usage();

    assert!(usage.contains(wgpu::BufferUsages::INDEX));
    assert!(usage.contains(wgpu::BufferUsages::COPY_DST));
}

#[test]
fn clip_plane_flag_and_depth_format_match_section_pipeline_requirements() {
    assert_eq!(clip_plane_enabled_flag(false), 0);
    assert_eq!(clip_plane_enabled_flag(true), 1);
    assert_eq!(
        depth_stencil_format(),
        wgpu::TextureFormat::Depth24PlusStencil8
    );
}

#[test]
fn section_passes_use_expected_stencil_and_depth_compare_modes() {
    assert_eq!(
        section_fill_stencil_compare(),
        wgpu::CompareFunction::NotEqual
    );
    assert_eq!(stencil_depth_compare(), wgpu::CompareFunction::Always);
}

#[test]
fn vertex_buffer_layout_exposes_model_color_attribute() {
    let layout = vertex_buffer_layout();

    assert_eq!(
        layout.array_stride,
        std::mem::size_of::<Vertex>() as u64
    );
    assert_eq!(layout.attributes.len(), 3);
    assert_eq!(layout.attributes[2].shader_location, 2);
    assert_eq!(layout.attributes[2].format, wgpu::VertexFormat::Float32x4);
}
