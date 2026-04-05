use egui_wgpu::wgpu;
use scad_scene::scene_bindings;

#[test]
fn scene_bind_group_layout_keeps_shadow_texture_bindings_for_lit_passes() {
    let entries = scene_bindings::scene_bind_group_layout_entries();

    assert_eq!(entries.len(), 3);
    assert_eq!(entries[0].binding, 0);
    assert_eq!(entries[1].binding, 1);
    assert_eq!(entries[2].binding, 2);
    assert!(matches!(
        entries[1].ty,
        wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Depth,
            ..
        }
    ));
    assert!(matches!(
        entries[2].ty,
        wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison)
    ));
}

#[test]
fn shadow_bind_group_layout_uses_only_uniform_binding() {
    let entries = scene_bindings::shadow_bind_group_layout_entries();

    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].binding, 0);
    assert_eq!(
        entries[0].visibility,
        wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT
    );
    assert!(matches!(
        entries[0].ty,
        wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            ..
        }
    ));
}
