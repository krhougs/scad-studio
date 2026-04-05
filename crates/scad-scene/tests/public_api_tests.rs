use scad_scene::{OrbitalCamera, ProjectionMode, RenderMode, RenderSettings};

#[test]
fn render_settings_defaults_match_viewer_defaults() {
    let settings = RenderSettings::default();

    assert_eq!(settings.render_mode, RenderMode::Solid);
    assert_eq!(settings.color_mode, scad_scene::ColorMode::Color);
    assert!(settings.show_grid);
    assert!(!settings.show_build_plate);
}

#[test]
fn camera_supports_projection_switching_from_scene_crate() {
    let mut camera = OrbitalCamera::new(1.0);
    camera.set_projection_mode(ProjectionMode::Orthographic);

    assert_eq!(camera.matrices().projection.w_axis.w, 1.0);
}
