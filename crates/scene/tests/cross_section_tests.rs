use scene::cross_section::{ClipPlane, EditMode};
use glam::{Quat, Vec3};

#[test]
fn translating_plane_moves_distance_along_normal() {
    let mut plane = ClipPlane::default();

    plane.translate_along_normal(5.0, false);

    assert_eq!(plane.distance, 5.0);
}

#[test]
fn translating_plane_with_snap_uses_one_millimeter_steps() {
    let mut plane = ClipPlane::default();

    plane.translate_along_normal(1.4, true);

    assert_eq!(plane.distance, 1.0);
}

#[test]
fn rotating_plane_with_snap_uses_five_degree_steps() {
    let mut plane = ClipPlane::default();

    plane.rotate(7.0_f32.to_radians(), Vec3::X, true);

    let expected = Quat::from_axis_angle(Vec3::X, 5.0_f32.to_radians()) * Vec3::Y;
    assert!(plane.normal.distance(expected.normalize()) < 0.001);
}

#[test]
fn ray_intersection_returns_distance_to_plane() {
    let plane = ClipPlane::default();
    let ray_origin = Vec3::new(0.0, 10.0, 0.0);
    let ray_direction = Vec3::NEG_Y;

    let hit = plane.ray_intersection(ray_origin, ray_direction);

    assert_eq!(hit, Some(10.0));
}

#[test]
fn edit_mode_switches_between_translate_and_rotate() {
    let mut mode = EditMode::Translate;

    mode = mode.toggle();
    assert_eq!(mode, EditMode::Rotate);

    mode = mode.toggle();
    assert_eq!(mode, EditMode::Translate);
}

#[test]
fn plane_corners_stay_on_plane_and_match_visible_extent() {
    let plane = ClipPlane::default();

    let corners = plane.corners();

    assert_eq!(corners.len(), 4);
    for corner in corners {
        assert!(plane.signed_distance(corner).abs() < 0.001);
        assert!(plane.contains_point(corner));
    }
    assert!(!plane.contains_point(Vec3::new(plane.visible_extent + 1.0, 0.0, 0.0)));
}

#[test]
fn screen_ray_from_center_points_toward_negative_z() {
    let ray = scene::cross_section::screen_ray(
        glam::Vec2::new(50.0, 50.0),
        glam::Vec2::new(100.0, 100.0),
        glam::Mat4::IDENTITY,
    )
    .expect("center ray should exist");

    assert!(ray.direction.z > 0.9);
}
