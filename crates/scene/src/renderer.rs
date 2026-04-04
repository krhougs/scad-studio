use std::fmt;

use wgpu;
use wgpu::util::DeviceExt;
use glam::{Mat4, Vec3, Vec4};

use crate::{
    camera::OrbitalCamera,
    cross_section::ClipPlane,
    grid::GridScene,
    lighting::{self},
    mesh::MeshData,
    pipeline::{self, ScenePipelines},
    scene_bindings::{
        DepthBuffer, SceneUniform, create_scene_bind_group, create_scene_bind_group_layout,
        create_scene_uniform_buffer, create_shadow_bind_group, create_shadow_bind_group_layout,
    },
    section::SectionResources,
    shadow::{self, ShadowResources},
    types::{RenderMode, RenderSettings},
};

const CLEAR_COLOR: wgpu::Color = wgpu::Color {
    r: 0.07,
    g: 0.09,
    b: 0.12,
    a: 1.0,
};
const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth24PlusStencil8;

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
    mesh: Option<GpuMesh>,
}

#[derive(Debug)]
struct GpuMesh {
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    index_count: u32,
    bounds: crate::mesh::Bounds,
}

#[derive(Debug)]
pub struct RendererError(String);

impl Renderer {
    pub async fn new(
        surface: wgpu::Surface<'static>,
        device: wgpu::Device,
        queue: wgpu::Queue,
        width: u32,
        height: u32,
        surface_format: wgpu::TextureFormat,
    ) -> Result<Self, RendererError> {
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: width.max(1),
            height: height.max(1),
            present_mode: wgpu::PresentMode::AutoVsync,
            desired_maximum_frame_latency: 2,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: vec![],
        };
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
            mesh: None,
        })
    }

    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }

    pub fn queue(&self) -> &wgpu::Queue {
        &self.queue
    }

    pub fn surface_format(&self) -> wgpu::TextureFormat {
        self.config.format
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

    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);
        self.depth_buffer = DepthBuffer::new(&self.device, width, height, DEPTH_FORMAT);
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
            bounds: mesh.bounds,
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
    ) -> Result<(), RendererError> {
        let frame = self.acquire_frame()?;
        self.update_scene_uniforms(camera, settings, clip_plane);
        if let Some(clip_plane) = clip_plane {
            self.section_resources.update_buffers(&self.queue, clip_plane);
        }
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("main_encoder"),
            });
        self.draw_shadow_pass(&mut encoder, settings);
        self.draw_scene_pass(&mut encoder, &view, settings, clip_plane);
        self.queue.submit(std::iter::once(encoder.finish()));
        frame.present();
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
        let matrices = camera.matrices();
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
                .map(|plane| [plane.normal.x, plane.normal.y, plane.normal.z, plane.distance])
                .unwrap_or([0.0, 1.0, 0.0, 0.0]),
            render_params: [
                pipeline::pipeline_alpha_for(render_mode),
                if settings.shadows_enabled { 1.0 } else { 0.0 },
                pipeline::pipeline_fog_density(settings.fog_enabled),
                0.0,
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
        render_pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        render_pass.draw_indexed(0..mesh.index_count, 0, 0..1);
    }

    fn draw_scene_pass(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        settings: &RenderSettings,
        clip_plane: Option<&ClipPlane>,
    ) {
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
                stencil_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(0),
                    store: wgpu::StoreOp::Store,
                }),
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
        });
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
            render_pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            render_pass.draw_indexed(0..mesh.index_count, 0, 0..1);
        }
        render_pass.set_pipeline(self.pipeline_for(settings));
        render_pass.set_bind_group(0, &self.scene_bind_group, &[]);
        render_pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
        render_pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        render_pass.draw_indexed(0..mesh.index_count, 0, 0..1);
        if clip_plane.is_some() {
            self.section_resources
                .draw_fill(&mut render_pass, &self.scene_bind_group);
            self.section_resources
                .draw_preview(&mut render_pass, &self.scene_bind_group);
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

    fn scene_bounds(&self) -> crate::mesh::Bounds {
        self.mesh.as_ref().map(|mesh| mesh.bounds).unwrap_or(crate::mesh::Bounds {
            min: Vec3::new(-128.0, -1.0, -128.0),
            max: Vec3::new(128.0, 128.0, 128.0),
        })
    }
}

impl std::error::Error for RendererError {}

impl fmt::Display for RendererError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
