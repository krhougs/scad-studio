use std::{fmt, sync::Arc};

use egui_wgpu::wgpu;
use egui_wgpu::wgpu::util::DeviceExt;
use glam::{Mat4, Vec3, Vec4};
#[cfg(target_arch = "wasm32")]
use web_sys::HtmlCanvasElement;
use winit::{dpi::PhysicalSize, window::Window};

use crate::{
    RenderMode, RenderSettings,
    camera::OrbitalCamera,
    cross_section::ClipPlane,
    grid::GridScene,
    lighting::{self},
    mesh::{Bounds, MeshData},
    pipeline::{self, ScenePipelines},
    scene_bindings::{
        DepthBuffer, SceneUniform, create_scene_bind_group, create_scene_bind_group_layout,
        create_scene_uniform_buffer, create_shadow_bind_group, create_shadow_bind_group_layout,
    },
    section::SectionResources,
    shadow::{self, ShadowResources},
};

const APP_BG_COLOR: wgpu::Color = wgpu::Color {
    r: 0.0,
    g: 0.0,
    b: 0.0,
    a: 1.0,
};
const CLEAR_COLOR: wgpu::Color = wgpu::Color {
    r: 0.07,
    g: 0.09,
    b: 0.12,
    a: 1.0,
};
const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth24PlusStencil8;

pub fn transparent_index_buffer_usage() -> wgpu::BufferUsages {
    wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST
}

pub struct Renderer {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    depth_buffer: DepthBuffer,
    grid_scene: GridScene,
    shadow_resources: ShadowResources,
    scene_pipelines: ScenePipelines,
    section_resources: SectionResources,
    scene_bind_group: wgpu::BindGroup,
    shadow_bind_group: wgpu::BindGroup,
    scene_uniform_buffer: wgpu::Buffer,
    egui_renderer: egui_wgpu::Renderer,
    mesh: Option<GpuMesh>,
}

pub struct EguiPaintData {
    pub clipped_primitives: Vec<egui::ClippedPrimitive>,
    pub textures_delta: egui::TexturesDelta,
    pub pixels_per_point: f32,
}

impl EguiPaintData {
    pub fn empty() -> Self {
        Self {
            clipped_primitives: Vec::new(),
            textures_delta: egui::TexturesDelta::default(),
            pixels_per_point: 1.0,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct RenderViewport {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    scissor_x: u32,
    scissor_y: u32,
    scissor_width: u32,
    scissor_height: u32,
}

#[derive(Debug)]
struct GpuMesh {
    vertex_buffer: wgpu::Buffer,
    full_index_buffer: wgpu::Buffer,
    full_index_count: u32,
    opaque_index_buffer: Option<wgpu::Buffer>,
    opaque_index_count: u32,
    transparent_index_buffer: Option<wgpu::Buffer>,
    transparent_index_count: u32,
    mesh_data: MeshData,
    bounds: Bounds,
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
        Self::new_with_surface(&instance, surface, size).await
    }

    #[cfg(target_arch = "wasm32")]
    pub async fn new_for_canvas(canvas: HtmlCanvasElement) -> Result<Self, RendererError> {
        let size = PhysicalSize::new(canvas.width().max(1), canvas.height().max(1));
        let instance = wgpu::Instance::default();
        let surface = instance
            .create_surface(wgpu::SurfaceTarget::Canvas(canvas))
            .map_err(|error| RendererError(format!("创建 canvas surface 失败: {error}")))?;
        Self::new_with_surface(&instance, surface, size).await
    }

    async fn new_with_surface(
        instance: &wgpu::Instance,
        surface: wgpu::Surface<'static>,
        size: PhysicalSize<u32>,
    ) -> Result<Self, RendererError> {
        let adapter = request_adapter(&instance, &surface).await?;
        let (device, queue) = request_device(&adapter).await?;
        let config = build_surface_config(&surface, &adapter, size)?;
        surface.configure(&device, &config);
        let depth_buffer = DepthBuffer::new(&device, config.width, config.height, DEPTH_FORMAT);
        let scene_uniform_buffer = create_scene_uniform_buffer(&device);
        let scene_bind_group_layout = create_scene_bind_group_layout(&device);
        let shadow_bind_group_layout = create_shadow_bind_group_layout(&device);
        let shadow_resources = ShadowResources::new(&device, &shadow_bind_group_layout);
        let scene_bind_group = create_scene_bind_group(
            &device,
            &scene_bind_group_layout,
            &scene_uniform_buffer,
            &shadow_resources,
        );
        let shadow_bind_group =
            create_shadow_bind_group(&device, &shadow_bind_group_layout, &scene_uniform_buffer);
        let grid_scene = GridScene::new(&device, &config, &scene_bind_group_layout);
        let scene_pipelines =
            pipeline::create_scene_pipelines(&device, &config, &scene_bind_group_layout);
        let section_resources = SectionResources::new(&device, &config, &scene_bind_group_layout);
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
            grid_scene,
            shadow_resources,
            scene_pipelines,
            section_resources,
            scene_bind_group,
            shadow_bind_group,
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

    pub fn wireframe_supported(&self) -> bool {
        pipeline::supports_wireframe(self.device.features())
    }

    pub fn resize(&mut self, size: PhysicalSize<u32>) {
        if size.width == 0 || size.height == 0 {
            return;
        }
        self.config.width = size.width;
        self.config.height = size.height;
        self.surface.configure(&self.device, &self.config);
        self.depth_buffer = DepthBuffer::new(&self.device, size.width, size.height, DEPTH_FORMAT);
    }

    pub fn set_mesh(&mut self, mesh: MeshData) {
        let bounds = mesh.bounds;
        let (opaque_indices, transparent_indices) = mesh.triangle_index_partitions();
        let vertex_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("mesh_vertex_buffer"),
                contents: bytemuck::cast_slice(&mesh.vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });
        let full_index_buffer = self.create_index_buffer(
            "mesh_index_buffer",
            &mesh.indices,
            wgpu::BufferUsages::INDEX,
        );
        let opaque_index_buffer = self.create_optional_index_buffer(
            "mesh_opaque_index_buffer",
            &opaque_indices,
            wgpu::BufferUsages::INDEX,
        );
        let transparent_index_buffer = self.create_optional_index_buffer(
            "mesh_transparent_index_buffer",
            &transparent_indices,
            transparent_index_buffer_usage(),
        );
        self.mesh = Some(GpuMesh {
            vertex_buffer,
            full_index_buffer,
            full_index_count: mesh.indices.len() as u32,
            opaque_index_buffer,
            opaque_index_count: opaque_indices.len() as u32,
            transparent_index_buffer,
            transparent_index_count: transparent_indices.len() as u32,
            mesh_data: mesh,
            bounds,
        });
    }

    pub fn clear_mesh(&mut self) {
        self.mesh = None;
    }

    pub fn render(
        &mut self,
        camera: &OrbitalCamera,
        settings: &RenderSettings,
        clip_plane: Option<&ClipPlane>,
        viewport: Option<[f32; 4]>,
        egui_paint: EguiPaintData,
    ) -> Result<(), RendererError> {
        let frame = self.acquire_frame()?;
        self.update_scene_uniforms(camera, settings, clip_plane);
        if let Some(clip_plane) = clip_plane {
            self.section_resources
                .update_buffers(&self.queue, clip_plane);
        }
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("main_encoder"),
            });
        let scene_viewport = viewport.and_then(|rect| {
            RenderViewport::from_physical(rect, self.config.width, self.config.height)
        });
        let draw_scene = viewport.is_none() || scene_viewport.is_some();
        if draw_scene {
            self.draw_shadow_pass(&mut encoder, settings);
        }
        self.draw_scene_pass(
            &mut encoder,
            &view,
            camera,
            settings,
            clip_plane,
            scene_viewport,
            draw_scene,
        );
        let extra_buffers = self.upload_egui_resources(&mut encoder, &egui_paint);
        self.draw_egui_pass(&mut encoder, &view, &egui_paint);
        self.queue
            .submit(extra_buffers.into_iter().chain([encoder.finish()]));
        frame.present();
        self.release_egui_textures(&egui_paint.textures_delta);
        Ok(())
    }

    pub fn render_egui_only(&mut self, egui_paint: EguiPaintData) -> Result<(), RendererError> {
        let frame = self.acquire_frame()?;
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("egui_only_encoder"),
            });
        self.clear_surface_pass(&mut encoder, &view);
        let extra_buffers = self.upload_egui_resources(&mut encoder, &egui_paint);
        self.draw_egui_pass(&mut encoder, &view, &egui_paint);
        self.queue
            .submit(extra_buffers.into_iter().chain([encoder.finish()]));
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

    fn update_scene_uniforms(
        &mut self,
        camera: &OrbitalCamera,
        settings: &RenderSettings,
        clip_plane: Option<&ClipPlane>,
    ) {
        let matrices = camera.matrices_for_bounds(Some(self.scene_bounds()));
        let render_mode =
            pipeline::resolve_render_mode(settings.render_mode, self.device.features());
        let lighting_state = lighting::encode_lights(&lighting::default_lights());
        let shadow_light = lighting_state
            .primary_shadow_light()
            .unwrap_or_else(|| lighting::default_lights()[1]);
        let light_view_proj = shadow::build_light_view_proj(shadow_light, self.scene_bounds());
        let scene_uniform = SceneUniform {
            view_proj: matrices.view_proj.to_cols_array_2d(),
            model: Mat4::IDENTITY.to_cols_array_2d(),
            light_view_proj: light_view_proj.to_cols_array_2d(),
            eye_position: Vec4::from((matrices.eye, 1.0)).to_array(),
            clip_plane: clip_plane
                .map(|plane| {
                    [
                        plane.normal.x,
                        plane.normal.y,
                        plane.normal.z,
                        plane.distance,
                    ]
                })
                .unwrap_or([0.0, 1.0, 0.0, 0.0]),
            render_params: [
                pipeline::pipeline_alpha_for(render_mode),
                if settings.shadows_enabled { 1.0 } else { 0.0 },
                pipeline::pipeline_fog_density(settings.fog_enabled),
                pipeline::pipeline_specular_strength(settings.color_mode),
            ],
            light_meta: [
                lighting_state.light_count,
                lighting_state.shadow_light_index,
                0,
                0,
            ],
            render_config: [
                pipeline::pipeline_color_mode(settings.color_mode),
                pipeline::clip_plane_enabled_flag(clip_plane.is_some()),
                0,
                0,
            ],
            lights: lighting_state.lights,
        };
        self.queue.write_buffer(
            &self.scene_uniform_buffer,
            0,
            bytemuck::bytes_of(&scene_uniform),
        );
    }

    fn draw_shadow_pass(&self, encoder: &mut wgpu::CommandEncoder, settings: &RenderSettings) {
        if !settings.shadows_enabled {
            return;
        }
        let Some(mesh) = &self.mesh else {
            return;
        };
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("shadow_pass"),
            color_attachments: &[],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &self.shadow_resources.view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        render_pass.set_pipeline(&self.shadow_resources.pipeline);
        render_pass.set_bind_group(0, &self.shadow_bind_group, &[]);
        render_pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
        if let Some(index_buffer) = mesh.opaque_index_buffer.as_ref() {
            render_pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            render_pass.draw_indexed(0..mesh.opaque_index_count, 0, 0..1);
        }
    }

    fn draw_scene_pass(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        camera: &OrbitalCamera,
        settings: &RenderSettings,
        clip_plane: Option<&ClipPlane>,
        viewport: Option<RenderViewport>,
        draw_scene: bool,
    ) {
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("scene_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(if draw_scene {
                        CLEAR_COLOR
                    } else {
                        APP_BG_COLOR
                    }),
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
                stencil_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(0),
                    store: wgpu::StoreOp::Store,
                }),
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        if !draw_scene {
            return;
        }
        if let Some(viewport) = viewport {
            render_pass.set_viewport(
                viewport.x,
                viewport.y,
                viewport.width,
                viewport.height,
                0.0,
                1.0,
            );
            render_pass.set_scissor_rect(
                viewport.scissor_x,
                viewport.scissor_y,
                viewport.scissor_width,
                viewport.scissor_height,
            );
        }
        self.grid_scene.draw(
            &mut render_pass,
            &self.scene_bind_group,
            settings.show_grid,
            settings.show_build_plate,
        );
        let Some(mesh) = &self.mesh else {
            if clip_plane.is_some() {
                self.section_resources
                    .draw_preview(&mut render_pass, &self.scene_bind_group);
            }
            return;
        };
        if clip_plane.is_some() {
            render_pass.set_pipeline(&self.scene_pipelines.section_stencil);
            render_pass.set_bind_group(0, &self.scene_bind_group, &[]);
            render_pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
            render_pass
                .set_index_buffer(mesh.full_index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            render_pass.draw_indexed(0..mesh.full_index_count, 0, 0..1);
        }
        render_pass.set_bind_group(0, &self.scene_bind_group, &[]);
        render_pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
        match pipeline::resolve_render_mode(settings.render_mode, self.device.features()) {
            RenderMode::Solid => {
                if let Some(index_buffer) = mesh.opaque_index_buffer.as_ref() {
                    render_pass.set_pipeline(&self.scene_pipelines.solid);
                    render_pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                    render_pass.draw_indexed(0..mesh.opaque_index_count, 0, 0..1);
                }
                if let Some(index_buffer) = mesh.transparent_index_buffer.as_ref() {
                    let sorted_indices = mesh
                        .mesh_data
                        .sorted_transparent_triangle_indices(camera.eye().to_array());
                    self.queue
                        .write_buffer(index_buffer, 0, bytemuck::cast_slice(&sorted_indices));
                    render_pass.set_pipeline(&self.scene_pipelines.solid_transparent);
                    render_pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                    render_pass.draw_indexed(0..mesh.transparent_index_count, 0, 0..1);
                }
            }
            RenderMode::Wireframe | RenderMode::XRay => {
                render_pass.set_pipeline(self.pipeline_for(settings));
                render_pass
                    .set_index_buffer(mesh.full_index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                render_pass.draw_indexed(0..mesh.full_index_count, 0, 0..1);
            }
        }
        if clip_plane.is_some() {
            self.section_resources
                .draw_fill(&mut render_pass, &self.scene_bind_group);
            self.section_resources
                .draw_preview(&mut render_pass, &self.scene_bind_group);
        }
    }

    fn clear_surface_pass(&self, encoder: &mut wgpu::CommandEncoder, view: &wgpu::TextureView) {
        encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("app_bg_clear_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(APP_BG_COLOR),
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
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

    fn pipeline_for(&self, settings: &RenderSettings) -> &wgpu::RenderPipeline {
        match pipeline::resolve_render_mode(settings.render_mode, self.device.features()) {
            RenderMode::Solid => &self.scene_pipelines.solid,
            RenderMode::Wireframe => self
                .scene_pipelines
                .wireframe
                .as_ref()
                .unwrap_or(&self.scene_pipelines.solid),
            RenderMode::XRay => &self.scene_pipelines.xray,
        }
    }

    fn scene_bounds(&self) -> Bounds {
        self.mesh
            .as_ref()
            .map(|mesh| mesh.bounds)
            .unwrap_or(Bounds {
                min: Vec3::new(-128.0, -1.0, -128.0),
                max: Vec3::new(128.0, 128.0, 128.0),
            })
    }

    fn create_index_buffer(
        &self,
        label: &'static str,
        indices: &[u32],
        usage: wgpu::BufferUsages,
    ) -> wgpu::Buffer {
        self.device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(label),
                contents: bytemuck::cast_slice(indices),
                usage,
            })
    }

    fn create_optional_index_buffer(
        &self,
        label: &'static str,
        indices: &[u32],
        usage: wgpu::BufferUsages,
    ) -> Option<wgpu::Buffer> {
        if indices.is_empty() {
            None
        } else {
            Some(self.create_index_buffer(label, indices, usage))
        }
    }
}

impl RenderViewport {
    fn from_physical(rect: [f32; 4], surface_width: u32, surface_height: u32) -> Option<Self> {
        let max_x = surface_width.max(1) as f32;
        let max_y = surface_height.max(1) as f32;
        let left = rect[0].clamp(0.0, max_x).floor();
        let top = rect[1].clamp(0.0, max_y).floor();
        let right = (rect[0] + rect[2]).clamp(left, max_x).ceil();
        let bottom = (rect[1] + rect[3]).clamp(top, max_y).ceil();
        let width = (right - left).max(0.0);
        let height = (bottom - top).max(0.0);
        if width < 1.0 || height < 1.0 {
            return None;
        }
        Some(Self {
            x: left,
            y: top,
            width,
            height,
            scissor_x: left as u32,
            scissor_y: top as u32,
            scissor_width: width as u32,
            scissor_height: height as u32,
        })
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
    let required_features = pipeline::requested_device_features(adapter.features());
    adapter
        .request_device(&wgpu::DeviceDescriptor {
            label: Some("wgpu_device"),
            required_features,
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
