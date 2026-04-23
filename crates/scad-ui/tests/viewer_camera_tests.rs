use scad_scene::{OrbitalCamera, ProjectionMode};

#[test]
fn sync_camera_to_viewport_updates_projection_mode() {
    let mut camera = OrbitalCamera::new(1.0);

    scad_ui::viewer_camera::sync_camera_to_viewport(
        &mut camera,
        ProjectionMode::Orthographic,
        egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(640.0, 320.0)),
    );

    assert_eq!(camera.matrices().projection.w_axis.w, 1.0);
}

#[test]
fn sync_camera_to_viewport_updates_aspect_ratio() {
    let mut camera = OrbitalCamera::new(1.0);

    scad_ui::viewer_camera::sync_camera_to_viewport(
        &mut camera,
        ProjectionMode::Perspective,
        egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(900.0, 300.0)),
    );

    let projection = camera.matrices().projection;
    let ratio = projection.y_axis.y / projection.x_axis.x;
    assert!(
        (ratio - 3.0).abs() < 0.01,
        "相机纵横比应与视口一致，当前为 {ratio}"
    );
}
