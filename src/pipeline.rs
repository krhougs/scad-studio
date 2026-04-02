use std::mem;

use egui_wgpu::wgpu;

use crate::{
    app::{ColorMode, RenderMode},
    mesh::Vertex,
};

pub struct ScenePipelines {
    pub solid: wgpu::RenderPipeline,
    pub wireframe: Option<wgpu::RenderPipeline>,
    pub xray: wgpu::RenderPipeline,
    pub section_stencil: wgpu::RenderPipeline,
}

const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth24PlusStencil8;

pub fn create_scene_pipelines(
    device: &wgpu::Device,
    config: &wgpu::SurfaceConfiguration,
    bind_group_layout: &wgpu::BindGroupLayout,
) -> ScenePipelines {
    let solid = create_pipeline(
        device,
        config,
        bind_group_layout,
        PipelineKind::Solid,
        wgpu::PolygonMode::Fill,
    );
    let wireframe = if supports_wireframe(device.features()) {
        Some(create_pipeline(
            device,
            config,
            bind_group_layout,
            PipelineKind::Solid,
            wgpu::PolygonMode::Line,
        ))
    } else {
        None
    };
    let xray = create_pipeline(
        device,
        config,
        bind_group_layout,
        PipelineKind::XRay,
        wgpu::PolygonMode::Fill,
    );
    let section_stencil = create_stencil_pipeline(device, config, bind_group_layout);
    ScenePipelines {
        solid,
        wireframe,
        xray,
        section_stencil,
    }
}

pub fn supports_wireframe(features: wgpu::Features) -> bool {
    features.contains(wgpu::Features::POLYGON_MODE_LINE)
}

pub fn requested_device_features(adapter_features: wgpu::Features) -> wgpu::Features {
    let mut requested = wgpu::Features::empty();
    if supports_wireframe(adapter_features) {
        requested |= wgpu::Features::POLYGON_MODE_LINE;
    }
    requested
}

pub fn resolve_render_mode(requested: RenderMode, features: wgpu::Features) -> RenderMode {
    if requested == RenderMode::Wireframe && !supports_wireframe(features) {
        RenderMode::Solid
    } else {
        requested
    }
}

#[allow(dead_code)]
pub fn polygon_mode_for(mode: RenderMode) -> wgpu::PolygonMode {
    match mode {
        RenderMode::Wireframe => wgpu::PolygonMode::Line,
        RenderMode::Solid | RenderMode::XRay => wgpu::PolygonMode::Fill,
    }
}

pub fn blend_state_for(mode: RenderMode) -> Option<wgpu::BlendState> {
    match mode {
        RenderMode::XRay => Some(wgpu::BlendState::ALPHA_BLENDING),
        RenderMode::Solid | RenderMode::Wireframe => Some(wgpu::BlendState::REPLACE),
    }
}

pub fn pipeline_color_mode(mode: ColorMode) -> u32 {
    match mode {
        ColorMode::Mono => 0,
        ColorMode::Color => 1,
    }
}

pub fn pipeline_alpha_for(mode: RenderMode) -> f32 {
    match mode {
        RenderMode::XRay => 0.38,
        RenderMode::Solid | RenderMode::Wireframe => 1.0,
    }
}

pub fn pipeline_fog_density(enabled: bool) -> f32 {
    if enabled { 0.01 } else { 0.0 }
}

pub fn clip_plane_enabled_flag(enabled: bool) -> u32 {
    u32::from(enabled)
}

pub fn depth_stencil_format() -> wgpu::TextureFormat {
    DEPTH_FORMAT
}

pub fn section_fill_stencil_compare() -> wgpu::CompareFunction {
    wgpu::CompareFunction::NotEqual
}

pub fn stencil_depth_compare() -> wgpu::CompareFunction {
    wgpu::CompareFunction::Always
}

pub fn vertex_buffer_layout() -> wgpu::VertexBufferLayout<'static> {
    wgpu::VertexBufferLayout {
        array_stride: mem::size_of::<Vertex>() as u64,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &[
            wgpu::VertexAttribute {
                offset: 0,
                shader_location: 0,
                format: wgpu::VertexFormat::Float32x3,
            },
            wgpu::VertexAttribute {
                offset: mem::size_of::<[f32; 3]>() as u64,
                shader_location: 1,
                format: wgpu::VertexFormat::Float32x3,
            },
            wgpu::VertexAttribute {
                offset: mem::size_of::<[f32; 6]>() as u64,
                shader_location: 2,
                format: wgpu::VertexFormat::Float32x4,
            },
        ],
    }
}

#[derive(Clone, Copy)]
enum PipelineKind {
    Solid,
    XRay,
}

fn create_stencil_pipeline(
    device: &wgpu::Device,
    config: &wgpu::SurfaceConfiguration,
    bind_group_layout: &wgpu::BindGroupLayout,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("section_stencil_shader"),
        source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
    });
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("section_stencil_pipeline_layout"),
        bind_group_layouts: &[bind_group_layout],
        push_constant_ranges: &[],
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("section_stencil_pipeline"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: &[vertex_buffer_layout()],
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: config.format,
                blend: Some(wgpu::BlendState::REPLACE),
                write_mask: wgpu::ColorWrites::empty(),
            })],
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: None,
            polygon_mode: wgpu::PolygonMode::Fill,
            ..Default::default()
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: DEPTH_FORMAT,
            depth_write_enabled: false,
            depth_compare: stencil_depth_compare(),
            stencil: wgpu::StencilState {
                front: wgpu::StencilFaceState {
                    compare: wgpu::CompareFunction::Always,
                    fail_op: wgpu::StencilOperation::Keep,
                    depth_fail_op: wgpu::StencilOperation::Keep,
                    pass_op: wgpu::StencilOperation::DecrementClamp,
                },
                back: wgpu::StencilFaceState {
                    compare: wgpu::CompareFunction::Always,
                    fail_op: wgpu::StencilOperation::Keep,
                    depth_fail_op: wgpu::StencilOperation::Keep,
                    pass_op: wgpu::StencilOperation::IncrementClamp,
                },
                read_mask: u32::MAX,
                write_mask: u32::MAX,
            },
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
        cache: None,
    })
}

fn create_pipeline(
    device: &wgpu::Device,
    config: &wgpu::SurfaceConfiguration,
    bind_group_layout: &wgpu::BindGroupLayout,
    kind: PipelineKind,
    polygon_mode: wgpu::PolygonMode,
) -> wgpu::RenderPipeline {
    let shader_source = match kind {
        PipelineKind::Solid => include_str!("shader.wgsl"),
        PipelineKind::XRay => include_str!("shader_xray.wgsl"),
    };
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some(match kind {
            PipelineKind::Solid => "scene_shader",
            PipelineKind::XRay => "scene_xray_shader",
        }),
        source: wgpu::ShaderSource::Wgsl(shader_source.into()),
    });
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some(match kind {
            PipelineKind::Solid => "scene_pipeline_layout",
            PipelineKind::XRay => "scene_xray_pipeline_layout",
        }),
        bind_group_layouts: &[bind_group_layout],
        push_constant_ranges: &[],
    });
    let render_mode = match kind {
        PipelineKind::Solid => {
            if polygon_mode == wgpu::PolygonMode::Line {
                RenderMode::Wireframe
            } else {
                RenderMode::Solid
            }
        }
        PipelineKind::XRay => RenderMode::XRay,
    };
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(match kind {
            PipelineKind::Solid => {
                if polygon_mode == wgpu::PolygonMode::Line {
                    "scene_wireframe_pipeline"
                } else {
                    "scene_pipeline"
                }
            }
            PipelineKind::XRay => "scene_xray_pipeline",
        }),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: &[vertex_buffer_layout()],
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: config.format,
                blend: blend_state_for(render_mode),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: if render_mode == RenderMode::XRay {
                None
            } else {
                Some(wgpu::Face::Back)
            },
            polygon_mode,
            ..Default::default()
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: DEPTH_FORMAT,
            depth_write_enabled: render_mode != RenderMode::XRay,
            depth_compare: wgpu::CompareFunction::LessEqual,
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
        cache: None,
    })
}
