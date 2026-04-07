#![allow(dead_code)]

#[path = "../src/image_zoom_math.rs"]
mod image_zoom_math;

use egui::{Pos2, vec2};
use image_zoom_math::pan_after_zoom_to_focal;

#[test]
fn zoom_toward_center_keeps_pan_near_zero() {
    let center = Pos2::new(200.0, 150.0);
    let tex = vec2(100.0, 100.0);
    let p = pan_after_zoom_to_focal(center, vec2(0.0, 0.0), tex, 0.5, 1.0, center).unwrap();
    assert!(p.length() < 1e-3, "pan={p:?}");
}

#[test]
fn zoom_preserves_point_under_focal() {
    let center = Pos2::new(100.0, 100.0);
    let tex = vec2(80.0, 40.0);
    let old_s = 1.0;
    let new_s = 2.0;
    let focal = Pos2::new(120.0, 90.0);
    let pan0 = vec2(5.0, -10.0);
    let pan1 = pan_after_zoom_to_focal(center, pan0, tex, old_s, new_s, focal).unwrap();
    let old_d = tex * old_s;
    let new_d = tex * new_s;
    let c0 = center.to_vec2() + pan0;
    let top0 = c0 - old_d * 0.5;
    let u = (focal.to_vec2() - top0) / old_d;
    let c1 = center.to_vec2() + pan1;
    let top1 = c1 - new_d * 0.5;
    let p0 = top0 + u * old_d;
    let p1 = top1 + u * new_d;
    assert!((p0 - p1).length() < 0.5, "p0={p0:?} p1={p1:?}");
}
