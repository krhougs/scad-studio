//! 图片缩放时保持焦点下像素位置不变的平移修正（视口中心 + 相对平移模型）。

use egui::{Pos2, Vec2};

pub fn pan_after_zoom_to_focal(
    viewport_center: Pos2,
    pan: Vec2,
    tex_size: Vec2,
    old_scale: f32,
    new_scale: f32,
    focal: Pos2,
) -> Option<Vec2> {
    let old_d = tex_size * old_scale;
    if old_d.x <= 1e-6 || old_d.y <= 1e-6 {
        return None;
    }
    let new_d = tex_size * new_scale;
    let center_img = viewport_center.to_vec2() + pan;
    let half_old = old_d * 0.5;
    let top_old = center_img - half_old;
    let u = (focal.to_vec2() - top_old) / old_d;
    let half_new = new_d * 0.5;
    let top_new = focal.to_vec2() - u * new_d;
    let center_new = top_new + half_new;
    Some(center_new - viewport_center.to_vec2())
}
