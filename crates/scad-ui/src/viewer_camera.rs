use scad_scene::{OrbitalCamera, ProjectionMode};

pub fn sync_camera_to_viewport(
    camera: &mut OrbitalCamera,
    projection_mode: ProjectionMode,
    viewport_rect: egui::Rect,
) {
    camera.set_projection_mode(projection_mode);
    camera.set_aspect_ratio(viewport_aspect_ratio(viewport_rect));
}

fn viewport_aspect_ratio(viewport_rect: egui::Rect) -> f32 {
    (viewport_rect.width() / viewport_rect.height().max(1.0)).max(0.1)
}
