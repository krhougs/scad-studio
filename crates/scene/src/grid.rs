use bytemuck::{Pod, Zeroable};
use wgpu;
use wgpu::util::DeviceExt;

pub const BUILD_PLATE_SIZE: f32 = 256.0;

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
pub struct GridVertex {
    pub position: [f32; 3],
    pub color: [f32; 4],
}

pub fn generate_grid_vertices(lines_per_side: i32, spacing: f32) -> Vec<GridVertex> {
    let mut vertices = Vec::new();
    let extent = lines_per_side as f32 * spacing;
    for index in -lines_per_side..=lines_per_side {
        let offset = index as f32 * spacing;
        vertices.push(vertex([-extent, 0.0, offset], [0.35, 0.39, 0.46, 0.5]));
        vertices.push(vertex([extent, 0.0, offset], [0.35, 0.39, 0.46, 0.5]));
        vertices.push(vertex([offset, 0.0, -extent], [0.35, 0.39, 0.46, 0.5]));
        vertices.push(vertex([offset, 0.0, extent], [0.35, 0.39, 0.46, 0.5]));
    }
    vertices
}

pub fn generate_build_plate_vertices(size: f32) -> Vec<GridVertex> {
    let half = size * 0.5;
    vec![
        vertex([-half, 0.0, -half], [0.82, 0.88, 0.95, 0.95]),
        vertex([half, 0.0, -half], [0.82, 0.88, 0.95, 0.95]),
        vertex([half, 0.0, -half], [0.82, 0.88, 0.95, 0.95]),
        vertex([half, 0.0, half], [0.82, 0.88, 0.95, 0.95]),
        vertex([half, 0.0, half], [0.82, 0.88, 0.95, 0.95]),
        vertex([-half, 0.0, half], [0.82, 0.88, 0.95, 0.95]),
        vertex([-half, 0.0, half], [0.82, 0.88, 0.95, 0.95]),
        vertex([-half, 0.0, -half], [0.82, 0.88, 0.95, 0.95]),
    ]
}

fn vertex(position: [f32; 3], color: [f32; 4]) -> GridVertex {
    GridVertex { position, color }
}

pub struct GridScene {
    pipeline: wgpu::RenderPipeline,
    grid_vertex_buffer: wgpu::Buffer,
    grid_vertex_count: u32,
    plate_vertex_buffer: wgpu::Buffer,
    plate_vertex_count: u32,
}

impl GridScene {
    pub fn new(
        device: &wgpu::Device,
        config: &wgpu::SurfaceConfiguration,
        bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let grid_vertices = generate_grid_vertices(20, 16.0);
        let plate_vertices = generate_build_plate_vertices(BUILD_PLATE_SIZE);
        let grid_vertex_buffer = create_vertex_buffer(device, "grid_vertex_buffer", &grid_vertices);
        let plate_vertex_buffer =
            create_vertex_buffer(device, "plate_vertex_buffer", &plate_vertices);
        let pipeline = create_pipeline(device, config, bind_group_layout);
        Self {
            pipeline,
            grid_vertex_buffer,
            grid_vertex_count: grid_vertices.len() as u32,
            plate_vertex_buffer,
            plate_vertex_count: plate_vertices.len() as u32,
        }
    }

    pub fn draw<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        bind_group: &'a wgpu::BindGroup,
        show_grid: bool,
        show_build_plate: bool,
    ) {
        if !show_grid && !show_build_plate {
            return;
        }
        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, bind_group, &[]);
        if show_grid {
            render_pass.set_vertex_buffer(0, self.grid_vertex_buffer.slice(..));
            render_pass.draw(0..self.grid_vertex_count, 0..1);
        }
        if show_build_plate {
            render_pass.set_vertex_buffer(0, self.plate_vertex_buffer.slice(..));
            render_pass.draw(0..self.plate_vertex_count, 0..1);
        }
    }
}

fn create_vertex_buffer(
    device: &wgpu::Device,
    label: &str,
    vertices: &[GridVertex],
) -> wgpu::Buffer {
    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some(label),
        contents: bytemuck::cast_slice(vertices),
        usage: wgpu::BufferUsages::VERTEX,
    })
}

fn create_pipeline(
    device: &wgpu::Device,
    config: &wgpu::SurfaceConfiguration,
    bind_group_layout: &wgpu::BindGroupLayout,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("grid_shader"),
        source: wgpu::ShaderSource::Wgsl(include_str!("shaders/shader_grid.wgsl").into()),
    });
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("grid_pipeline_layout"),
        bind_group_layouts: &[bind_group_layout],
        push_constant_ranges: &[],
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("grid_pipeline"),
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
            topology: wgpu::PrimitiveTopology::LineList,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: None,
            ..Default::default()
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: wgpu::TextureFormat::Depth24PlusStencil8,
            depth_write_enabled: false,
            depth_compare: wgpu::CompareFunction::LessEqual,
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
        cache: None,
    })
}

fn vertex_buffer_layout() -> wgpu::VertexBufferLayout<'static> {
    wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<GridVertex>() as u64,
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
