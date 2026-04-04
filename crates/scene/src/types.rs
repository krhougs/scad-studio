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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderSettings {
    pub render_mode: RenderMode,
    pub color_mode: ColorMode,
    pub projection_mode: ProjectionMode,
    pub wireframe_supported: bool,
    pub show_grid: bool,
    pub show_build_plate: bool,
    pub show_axis_gizmo: bool,
    pub shadows_enabled: bool,
    pub fog_enabled: bool,
    pub clip_plane_enabled: bool,
}

impl Default for RenderSettings {
    fn default() -> Self {
        Self {
            render_mode: RenderMode::Solid,
            color_mode: ColorMode::Color,
            projection_mode: ProjectionMode::Perspective,
            wireframe_supported: false,
            show_grid: true,
            show_build_plate: true,
            show_axis_gizmo: true,
            shadows_enabled: true,
            fog_enabled: false,
            clip_plane_enabled: false,
        }
    }
}
