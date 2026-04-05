use glam::{Vec2, Vec3, Vec4};
use scad_scene::{Bounds, OrbitalCamera, ProjectionMode};

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

#[test]
fn switching_to_orthographic_keeps_eye_position_but_changes_projection_matrix() {
    let mut camera = OrbitalCamera::new(1.0);

    let perspective = camera.matrices();
    camera.set_projection_mode(ProjectionMode::Orthographic);
    let orthographic = camera.matrices();

    assert_eq!(perspective.eye, orthographic.eye);
    assert_ne!(perspective.projection, orthographic.projection);
    assert_eq!(orthographic.projection.w_axis.w, 1.0);
}

#[test]
fn matrices_for_bounds_tighten_depth_range_for_scene_bounds() {
    let bounds = Bounds {
        min: Vec3::new(-100.0, -20.0, -80.0),
        max: Vec3::new(100.0, 120.0, 80.0),
    };
    let mut camera = OrbitalCamera::new(1.0);
    camera.fit_bounds(bounds);

    let (near, far) = camera.clipping_planes(Some(bounds));

    assert!(near > 0.01);
    assert!(far < 10_000.0);
    assert!(far > near);
}

#[test]
fn orthographic_fit_bounds_still_respects_aspect_ratio() {
    let bounds = Bounds {
        min: Vec3::new(-10.0, -1.0, -1.0),
        max: Vec3::new(10.0, 1.0, 1.0),
    };
    let mut wide = OrbitalCamera::new(2.0);
    let mut narrow = OrbitalCamera::new(0.5);
    wide.set_projection_mode(ProjectionMode::Orthographic);
    narrow.set_projection_mode(ProjectionMode::Orthographic);

    wide.fit_bounds(bounds);
    narrow.fit_bounds(bounds);

    let wide_distance = wide.matrices().eye.distance(bounds.center());
    let narrow_distance = narrow.matrices().eye.distance(bounds.center());

    assert!(narrow_distance > wide_distance);
}

#[test]
fn orthographic_fit_bounds_keeps_all_corners_inside_clip_space() {
    let bounds = Bounds {
        min: Vec3::new(-8.0, -3.0, -5.0),
        max: Vec3::new(9.0, 7.0, 6.0),
    };
    let mut camera = OrbitalCamera::new(1.0);
    camera.set_projection_mode(ProjectionMode::Orthographic);
    camera.fit_bounds(bounds);
    let matrices = camera.matrices();

    for corner in bounds_corners(bounds) {
        let clip = matrices.view_proj * Vec4::new(corner.x, corner.y, corner.z, 1.0);
        assert!(
            clip.x.abs() <= 1.0,
            "x clip out of range for {corner:?}: {}",
            clip.x
        );
        assert!(
            clip.y.abs() <= 1.0,
            "y clip out of range for {corner:?}: {}",
            clip.y
        );
    }
}

#[test]
fn orbit_allows_crossing_over_the_top_of_the_model() {
    let mut camera = OrbitalCamera::new(1.0);

    camera.orbit(Vec2::new(0.0, -400.0));

    assert!(
        camera.matrices().eye.y < 0.0,
        "camera should move below the target after a full vertical orbit"
    );
}

fn bounds_corners(bounds: Bounds) -> [Vec3; 8] {
    let min = bounds.min;
    let max = bounds.max;
    [
        Vec3::new(min.x, min.y, min.z),
        Vec3::new(min.x, min.y, max.z),
        Vec3::new(min.x, max.y, min.z),
        Vec3::new(min.x, max.y, max.z),
        Vec3::new(max.x, min.y, min.z),
        Vec3::new(max.x, min.y, max.z),
        Vec3::new(max.x, max.y, min.z),
        Vec3::new(max.x, max.y, max.z),
    ]
}
