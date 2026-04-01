#![allow(dead_code)]

#[path = "../src/gizmo.rs"]
mod gizmo;

use glam::{Mat4, Vec2, Vec3};
use gizmo::project_axes;

#[test]
fn projected_axes_keep_center_as_starting_point() {
    let center = Vec2::new(32.0, 48.0);
    let view = Mat4::look_at_rh(Vec3::new(3.0, 3.0, 3.0), Vec3::ZERO, Vec3::Y);

    let axes = project_axes(view, center, 20.0);

    assert!(axes.iter().all(|axis| axis.start == center));
}

#[test]
fn projected_axes_produce_distinct_endpoints_for_isometric_view() {
    let center = Vec2::new(32.0, 48.0);
    let view = Mat4::look_at_rh(Vec3::new(3.0, 3.0, 3.0), Vec3::ZERO, Vec3::Y);

    let axes = project_axes(view, center, 20.0);

    assert_ne!(axes[0].end, axes[1].end);
    assert_ne!(axes[1].end, axes[2].end);
    assert_ne!(axes[0].end, axes[2].end);
}
