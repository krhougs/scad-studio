use std::{fmt, mem, sync::Arc};

use bytemuck::{Pod, Zeroable};
use egui_wgpu::wgpu;
use egui_wgpu::wgpu::util::DeviceExt;
use glam::{Mat4, Vec3, Vec4};
use winit::{dpi::PhysicalSize, window::Window};

use crate::{
    camera::OrbitalCamera,
    mesh::{MeshData, Vertex},
};

const CLEAR_COLOR: wgpu::Color = wgpu::Color {
    r: 0.07,
    g: 0.09,
    b: 0.12,
    a: 1.0,
};
const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

pub struct Renderer {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    depth_buffer: DepthBuffer,
    scene_pipeline: wgpu::RenderPipeline,
    scene_bind_group: wgpu::BindGroup,
    scene_uniform_buffer: wgpu::Buffer,
    egui_renderer: egui_wgpu::Renderer,
    mesh: Option<GpuMesh>,
}

pub struct EguiPaintData {
    pub clipped_primitives: Vec<egui::ClippedPrimitive>,
    pub textures_delta: egui::TexturesDelta,
    pub pixels_per_point: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct SceneUniform {
    view_proj: [[f32; 4]; 4],
    model: [[f32; 4]; 4],
    eye_position: [f32; 4],
    light_direction: [f32; 4],
}

#[derive(Debug)]
struct DepthBuffer {
    _texture: wgpu::Texture,
    view: wgpu::TextureView,
}

#[derive(Debug)]
struct GpuMesh {
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    index_count: u32,
}

#[derive(Debug)]
pub struct RendererError(String);

impl Renderer {
    pub async fn new(window: Arc<Window>) -> Result<Self, RendererError> {
        let size = window.inner_size();
        let instance = wgpu::Instance::default();
        let surface = instance
            .create_surface(window)
            .map_err(|error| RendererError(format!("创建渲染 surface 失败: {error}")))?;
        let adapter = request_adapter(&instance, &surface).await?;
        let (device, queue) = request_device(&adapter).await?;
        let config = build_surface_config(&surface, &adapter, size)?;
        surface.configure(&device, &config);
        let depth_buffer = DepthBuffer::new(&device, config.width, config.height);
        let scene_uniform_buffer = create_scene_uniform_buffer(&device);
        let scene_bind_group_layout = create_scene_bind_group_layout(&device);
        let scene_bind_group =
            create_scene_bind_group(&device, &scene_bind_group_layout, &scene_uniform_buffer);
        let scene_pipeline =
            create_scene_pipeline(&device, &config, &scene_bind_group_layout);
        let egui_renderer = egui_wgpu::Renderer::new(
            &device,
            config.format,
            egui_wgpu::RendererOptions::default(),
        );
        Ok(Self {
            surface,
            device,
            queue,
            config,
            depth_buffer,
            scene_pipeline,
            scene_bind_group,
            scene_uniform_buffer,
            egui_renderer,
            mesh: None,
        })
    }

    pub fn max_texture_side(&self) -> usize {
        self.device.limits().max_texture_dimension_2d as usize
    }

    pub fn aspect_ratio(&self) -> f32 {
        self.config.width.max(1) as f32 / self.config.height.max(1) as f32
    }

    pub fn resize(&mut self, size: PhysicalSize<u32>) {
        if size.width == 0 || size.height == 0 {
            return;
        }
        self.config.width = size.width;
        self.config.height = size.height;
        self.surface.configure(&self.device, &self.config);
        self.depth_buffer = DepthBuffer::new(&self.device, size.width, size.height);
    }

    pub fn set_mesh(&mut self, mesh: MeshData) {
        let vertex_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("mesh_vertex_buffer"),
                contents: bytemuck::cast_slice(&mesh.vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });
        let index_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("mesh_index_buffer"),
                contents: bytemuck::cast_slice(&mesh.indices),
                usage: wgpu::BufferUsages::INDEX,
            });
        self.mesh = Some(GpuMesh {
            vertex_buffer,
            index_buffer,
            index_count: mesh.indices.len() as u32,
        });
    }

    pub fn clear_mesh(&mut self) {
        self.mesh = None;
    }

    pub fn render(
        &mut self,
        camera: &OrbitalCamera,
        egui_paint: EguiPaintData,
    ) -> Result<(), RendererError> {
        let frame = self.acquire_frame()?;
        self.update_scene_uniforms(camera);
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("main_encoder"),
        });
        self.draw_scene_pass(&mut encoder, &view);
        let extra_buffers = self.upload_egui_resources(&mut encoder, &egui_paint);
        self.draw_egui_pass(&mut encoder, &view, &egui_paint);
        self.queue.submit(extra_buffers.into_iter().chain([encoder.finish()]));
        frame.present();
        self.release_egui_textures(&egui_paint.textures_delta);
        Ok(())
    }

    fn acquire_frame(&mut self) -> Result<wgpu::SurfaceTexture, RendererError> {
        match self.surface.get_current_texture() {
            Ok(frame) => Ok(frame),
            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                self.surface.configure(&self.device, &self.config);
                self.surface
                    .get_current_texture()
                    .map_err(|error| RendererError(format!("重新获取 surface 纹理失败: {error}")))
            }
            Err(error) => Err(RendererError(format!("获取 surface 纹理失败: {error}"))),
        }
    }

    fn update_scene_uniforms(&mut self, camera: &OrbitalCamera) {
        let matrices = camera.matrices();
        let scene_uniform = SceneUniform {
            view_proj: matrices.view_proj.to_cols_array_2d(),
            model: Mat4::IDENTITY.to_cols_array_2d(),
            eye_position: Vec4::from((matrices.eye, 1.0)).to_array(),
            light_direction: Vec4::from((Vec3::new(-0.5, -0.8, -0.2).normalize(), 0.0)).to_array(),
        };
        self.queue.write_buffer(
            &self.scene_uniform_buffer,
            0,
            bytemuck::bytes_of(&scene_uniform),
        );
    }

    fn draw_scene_pass(&self, encoder: &mut wgpu::CommandEncoder, view: &wgpu::TextureView) {
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("scene_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(CLEAR_COLOR),
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &self.depth_buffer.view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        let Some(mesh) = &self.mesh else {
            return;
        };
        render_pass.set_pipeline(&self.scene_pipeline);
        render_pass.set_bind_group(0, &self.scene_bind_group, &[]);
        render_pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
        render_pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        render_pass.draw_indexed(0..mesh.index_count, 0, 0..1);
    }

    fn upload_egui_resources(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        egui_paint: &EguiPaintData,
    ) -> Vec<wgpu::CommandBuffer> {
        for (texture_id, delta) in &egui_paint.textures_delta.set {
            self.egui_renderer
                .update_texture(&self.device, &self.queue, *texture_id, delta);
        }
        let screen_descriptor = self.screen_descriptor(egui_paint.pixels_per_point);
        self.egui_renderer.update_buffers(
            &self.device,
            &self.queue,
            encoder,
            &egui_paint.clipped_primitives,
            &screen_descriptor,
        )
    }

    fn draw_egui_pass(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        egui_paint: &EguiPaintData,
    ) {
        let render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("egui_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        let screen_descriptor = self.screen_descriptor(egui_paint.pixels_per_point);
        self.egui_renderer.render(
            &mut render_pass.forget_lifetime(),
            &egui_paint.clipped_primitives,
            &screen_descriptor,
        );
    }

    fn release_egui_textures(&mut self, textures_delta: &egui::TexturesDelta) {
        for texture_id in &textures_delta.free {
            self.egui_renderer.free_texture(texture_id);
        }
    }

    fn screen_descriptor(&self, pixels_per_point: f32) -> egui_wgpu::ScreenDescriptor {
        egui_wgpu::ScreenDescriptor {
            size_in_pixels: [self.config.width, self.config.height],
            pixels_per_point,
        }
    }
}

impl DepthBuffer {
    fn new(device: &wgpu::Device, width: u32, height: u32) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("depth_texture"),
            size: wgpu::Extent3d {
                width: width.max(1),
                height: height.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        Self {
            _texture: texture,
            view,
        }
    }
}

fn create_scene_uniform_buffer(device: &wgpu::Device) -> wgpu::Buffer {
    let initial = SceneUniform {
        view_proj: Mat4::IDENTITY.to_cols_array_2d(),
        model: Mat4::IDENTITY.to_cols_array_2d(),
        eye_position: [0.0, 0.0, 3.0, 1.0],
        light_direction: [0.0, -1.0, 0.0, 0.0],
    };
    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("scene_uniform_buffer"),
        contents: bytemuck::bytes_of(&initial),
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    })
}

fn create_scene_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("scene_bind_group_layout"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        }],
    })
}

fn create_scene_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    buffer: &wgpu::Buffer,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("scene_bind_group"),
        layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: buffer.as_entire_binding(),
        }],
    })
}

fn create_scene_pipeline(
    device: &wgpu::Device,
    config: &wgpu::SurfaceConfiguration,
    bind_group_layout: &wgpu::BindGroupLayout,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("scene_shader"),
        source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
    });
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("scene_pipeline_layout"),
        bind_group_layouts: &[bind_group_layout],
        push_constant_ranges: &[],
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("scene_pipeline"),
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
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: Some(wgpu::Face::Back),
            ..Default::default()
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: DEPTH_FORMAT,
            depth_write_enabled: true,
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
        ],
    }
}

async fn request_adapter(
    instance: &wgpu::Instance,
    surface: &wgpu::Surface<'_>,
) -> Result<wgpu::Adapter, RendererError> {
    instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(surface),
            force_fallback_adapter: false,
        })
        .await
        .map_err(|error| RendererError(format!("请求 GPU adapter 失败: {error}")))
}

async fn request_device(
    adapter: &wgpu::Adapter,
) -> Result<(wgpu::Device, wgpu::Queue), RendererError> {
    adapter
        .request_device(&wgpu::DeviceDescriptor {
            label: Some("wgpu_device"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            experimental_features: wgpu::ExperimentalFeatures::disabled(),
            memory_hints: wgpu::MemoryHints::Performance,
            trace: wgpu::Trace::Off,
        })
        .await
        .map_err(|error| RendererError(format!("请求 GPU device 失败: {error}")))
}

fn build_surface_config(
    surface: &wgpu::Surface<'_>,
    adapter: &wgpu::Adapter,
    size: PhysicalSize<u32>,
) -> Result<wgpu::SurfaceConfiguration, RendererError> {
    let mut config = surface
        .get_default_config(adapter, size.width.max(1), size.height.max(1))
        .ok_or_else(|| RendererError("当前 adapter 不支持 surface".into()))?;
    let capabilities = surface.get_capabilities(adapter);
    if let Some(format) = capabilities.formats.iter().find(|format| format.is_srgb()) {
        config.format = *format;
    }
    Ok(config)
}

impl std::error::Error for RendererError {}

impl fmt::Display for RendererError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
