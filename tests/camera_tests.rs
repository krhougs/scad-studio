#[path = "../src/camera.rs"]
mod camera;
#[path = "../src/mesh.rs"]
mod mesh;

use camera::OrbitalCamera;
use glam::Vec3;
use mesh::Bounds;

#[test]
fn fit_bounds_moves_camera_target_to_model_center() {
    let mut camera = OrbitalCamera::new(1.0);
    let bounds = Bounds {
        min: Vec3::new(-2.0, -1.0, -3.0),
        max: Vec3::new(4.0, 5.0, 1.0),
    };

    camera.fit_bounds(bounds);
    let matrices = camera.matrices();

    assert!(matrices.eye.distance(bounds.center()) > 0.1);
}

#[test]
fn zoom_keeps_camera_distance_positive() {
    let mut camera = OrbitalCamera::new(1.0);

    camera.zoom(100.0);
    let close_eye = camera.matrices().eye;
    camera.zoom(-200.0);
    let far_eye = camera.matrices().eye;

    assert!(close_eye.length() > 0.0);
    assert!(far_eye.length() > close_eye.length());
}

#[test]
fn fit_bounds_uses_aspect_ratio_for_narrow_viewports() {
    let bounds = Bounds {
        min: Vec3::new(-10.0, -1.0, -1.0),
        max: Vec3::new(10.0, 1.0, 1.0),
    };
    let mut wide = OrbitalCamera::new(2.0);
    let mut narrow = OrbitalCamera::new(0.5);

    wide.fit_bounds(bounds);
    narrow.fit_bounds(bounds);

    let wide_distance = wide.matrices().eye.distance(bounds.center());
    let narrow_distance = narrow.matrices().eye.distance(bounds.center());

    assert!(narrow_distance > wide_distance);
}
