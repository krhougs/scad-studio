use glam::{Mat4, Vec2, Vec3, Vec4};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GizmoAxis {
    pub start: Vec2,
    pub end: Vec2,
    pub color: [u8; 3],
}

pub fn project_axes(view: Mat4, center: Vec2, axis_length: f32) -> [GizmoAxis; 3] {
    [
        axis(view, center, axis_length, Vec3::X, [255, 96, 96]),
        axis(view, center, axis_length, Vec3::Y, [96, 220, 128]),
        axis(view, center, axis_length, Vec3::Z, [96, 152, 255]),
    ]
}

fn axis(view: Mat4, center: Vec2, axis_length: f32, direction: Vec3, color: [u8; 3]) -> GizmoAxis {
    let projected = view * Vec4::new(direction.x, direction.y, direction.z, 0.0);
    let screen = center + Vec2::new(projected.x, -projected.y) * axis_length;
    GizmoAxis {
        start: center,
        end: screen,
        color,
    }
}

pub fn paint_overlay(ctx: &egui::Context, show_axis_gizmo: bool, view: Mat4) {
    if !show_axis_gizmo {
        return;
    }
    let screen = ctx.input(|input| input.content_rect());
    let center = Vec2::new(screen.left() + 44.0, screen.bottom() - 44.0);
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        egui::Id::new("axis_gizmo"),
    ));
    for axis in project_axes(view, center, 18.0) {
        painter.line_segment(
            [
                egui::pos2(axis.start.x, axis.start.y),
                egui::pos2(axis.end.x, axis.end.y),
            ],
            egui::Stroke::new(
                2.0,
                egui::Color32::from_rgb(axis.color[0], axis.color[1], axis.color[2]),
            ),
        );
    }
}
