use bytemuck::{Pod, Zeroable};
use wgpu;
use wgpu::util::DeviceExt;

use crate::{cross_section::ClipPlane, pipeline};

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct SectionVertex {
    position: [f32; 3],
    color: [f32; 4],
}

pub struct SectionResources {
    preview_pipeline: wgpu::RenderPipeline,
    fill_pipeline: wgpu::RenderPipeline,
    preview_vertex_buffer: wgpu::Buffer,
    fill_vertex_buffer: wgpu::Buffer,
    vertex_count: u32,
}

impl SectionResources {
    pub fn new(
        device: &wgpu::Device,
        config: &wgpu::SurfaceConfiguration,
        bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        Self {
            preview_pipeline: create_pipeline(
                device,
                config,
                bind_group_layout,
                SectionPipelineKind::Preview,
            ),
            fill_pipeline: create_pipeline(
                device,
                config,
                bind_group_layout,
                SectionPipelineKind::Fill,
            ),
            preview_vertex_buffer: create_vertex_buffer(device, "section_preview_vertex_buffer"),
            fill_vertex_buffer: create_vertex_buffer(device, "section_fill_vertex_buffer"),
            vertex_count: 6,
        }
    }

    pub fn update_buffers(&self, queue: &wgpu::Queue, plane: &ClipPlane) {
        let preview = build_vertices(
            plane,
            if plane.selected {
                [0.35, 0.72, 1.0, 0.34]
            } else {
                [0.24, 0.52, 0.95, 0.22]
            },
        );
        let fill = build_vertices(plane, [1.0, 0.64, 0.22, 0.95]);
        queue.write_buffer(
            &self.preview_vertex_buffer,
            0,
            bytemuck::cast_slice(&preview),
        );
        queue.write_buffer(&self.fill_vertex_buffer, 0, bytemuck::cast_slice(&fill));
    }

    pub fn draw_preview<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        bind_group: &'a wgpu::BindGroup,
    ) {
        render_pass.set_pipeline(&self.preview_pipeline);
        render_pass.set_bind_group(0, bind_group, &[]);
        render_pass.set_vertex_buffer(0, self.preview_vertex_buffer.slice(..));
        render_pass.draw(0..self.vertex_count, 0..1);
    }

    pub fn draw_fill<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        bind_group: &'a wgpu::BindGroup,
    ) {
        render_pass.set_pipeline(&self.fill_pipeline);
        render_pass.set_bind_group(0, bind_group, &[]);
        render_pass.set_vertex_buffer(0, self.fill_vertex_buffer.slice(..));
        render_pass.draw(0..self.vertex_count, 0..1);
    }
}

#[derive(Clone, Copy)]
enum SectionPipelineKind {
    Preview,
    Fill,
}

fn create_vertex_buffer(device: &wgpu::Device, label: &str) -> wgpu::Buffer {
    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some(label),
        contents: bytemuck::cast_slice(&[SectionVertex::zeroed(); 6]),
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
    })
}

fn create_pipeline(
    device: &wgpu::Device,
    config: &wgpu::SurfaceConfiguration,
    bind_group_layout: &wgpu::BindGroupLayout,
    kind: SectionPipelineKind,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("section_shader"),
        source: wgpu::ShaderSource::Wgsl(include_str!("shaders/shader_section.wgsl").into()),
    });
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("section_pipeline_layout"),
        bind_group_layouts: &[bind_group_layout],
        push_constant_ranges: &[],
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(match kind {
            SectionPipelineKind::Preview => "section_preview_pipeline",
            SectionPipelineKind::Fill => "section_fill_pipeline",
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
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: None,
            ..Default::default()
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: pipeline::depth_stencil_format(),
            depth_write_enabled: false,
            depth_compare: pipeline::stencil_depth_compare(),
            stencil: match kind {
                SectionPipelineKind::Preview => wgpu::StencilState::default(),
                SectionPipelineKind::Fill => wgpu::StencilState {
                    front: wgpu::StencilFaceState {
                        compare: pipeline::section_fill_stencil_compare(),
                        fail_op: wgpu::StencilOperation::Keep,
                        depth_fail_op: wgpu::StencilOperation::Keep,
                        pass_op: wgpu::StencilOperation::Zero,
                    },
                    back: wgpu::StencilFaceState {
                        compare: pipeline::section_fill_stencil_compare(),
                        fail_op: wgpu::StencilOperation::Keep,
                        depth_fail_op: wgpu::StencilOperation::Keep,
                        pass_op: wgpu::StencilOperation::Zero,
                    },
                    read_mask: u32::MAX,
                    write_mask: u32::MAX,
                },
            },
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
        cache: None,
    })
}

fn build_vertices(plane: &ClipPlane, color: [f32; 4]) -> [SectionVertex; 6] {
    let corners = plane.corners();
    [
        vertex(corners[0], color),
        vertex(corners[1], color),
        vertex(corners[2], color),
        vertex(corners[0], color),
        vertex(corners[2], color),
        vertex(corners[3], color),
    ]
}

fn vertex(position: glam::Vec3, color: [f32; 4]) -> SectionVertex {
    SectionVertex {
        position: position.to_array(),
        color,
    }
}

fn vertex_buffer_layout() -> wgpu::VertexBufferLayout<'static> {
    wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<SectionVertex>() as u64,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &[
            wgpu::VertexAttribute {
                offset: 0,
                shader_location: 0,
                format: wgpu::VertexFormat::Float32x3,
            },
            wgpu::VertexAttribute {
                offset: std::mem::size_of::<[f32; 3]>() as u64,
                shader_location: 1,
                format: wgpu::VertexFormat::Float32x4,
            },
        ],
    }
}
