pub mod camera;
pub mod cross_section;
pub mod gizmo;
pub mod grid;
pub mod lighting;
pub mod mesh;
pub mod pipeline;
pub mod renderer;
pub mod scene_bindings;
pub mod section;
pub mod shadow;
pub mod system_fonts;
pub mod three_mf;

pub use camera::{CameraInteraction, CameraMatrices, OrbitalCamera};
pub use cross_section::{ClipPlane, EditMode};
pub use mesh::{Bounds, MeshData, MeshTriangle, Vertex};
pub use renderer::{EguiPaintData, Renderer, RendererError, transparent_index_buffer_usage};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderMode {
    Solid,
    Wireframe,
    XRay,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorMode {
    Color,
    Mono,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectionMode {
    Perspective,
    Orthographic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderSettings {
    pub render_mode: RenderMode,
    pub color_mode: ColorMode,
    pub show_grid: bool,
    pub show_build_plate: bool,
    pub shadows_enabled: bool,
    pub fog_enabled: bool,
}

impl Default for RenderSettings {
    fn default() -> Self {
        Self {
            render_mode: RenderMode::Solid,
            color_mode: ColorMode::Color,
            show_grid: true,
            show_build_plate: false,
            shadows_enabled: false,
            fog_enabled: false,
        }
    }
}
