use super::*;

pub(super) fn handle_cross_section_event(
    tab: &mut ViewerTab,
    event: &WindowEvent,
    viewport_rect: egui::Rect,
) -> bool {
    match event {
        WindowEvent::ModifiersChanged(modifiers) => {
            tab.ctrl_pressed = modifiers.state().control_key();
            false
        }
        WindowEvent::KeyboardInput { event, .. } => {
            if event.state != ElementState::Pressed || !tab.viewer.viewer_state().clip_plane_enabled
            {
                return false;
            }
            match event.physical_key {
                PhysicalKey::Code(KeyCode::KeyW) => {
                    tab.clip_edit_mode = EditMode::Translate;
                    true
                }
                PhysicalKey::Code(KeyCode::KeyE) => {
                    tab.clip_edit_mode = EditMode::Rotate;
                    true
                }
                _ => false,
            }
        }
        WindowEvent::MouseInput {
            state: ElementState::Pressed,
            button: winit::event::MouseButton::Left,
            ..
        } => begin_clip_drag(
            tab,
            Vec2::new(viewport_rect.width(), viewport_rect.height()),
        ),
        WindowEvent::MouseInput {
            state: ElementState::Released,
            button: winit::event::MouseButton::Left,
            ..
        } => {
            tab.clip_drag_active = false;
            false
        }
        WindowEvent::CursorMoved { position, .. } => {
            let cursor = viewport_local_cursor(*position, viewport_rect);
            if update_clip_drag(tab, cursor) {
                return true;
            }
            tab.cursor_position = Some(cursor);
            false
        }
        _ => false,
    }
}

pub(super) fn handle_camera_event(
    tab: &mut ViewerTab,
    event: &WindowEvent,
    viewport_rect: egui::Rect,
) -> bool {
    match event {
        WindowEvent::MouseInput { state, button, .. } => tab
            .camera_interaction
            .handle_mouse_input_event(*state, *button),
        WindowEvent::CursorMoved { position, .. } => tab.camera_interaction.handle_cursor_position(
            &mut tab.camera,
            viewport_local_cursor(*position, viewport_rect),
        ),
        WindowEvent::MouseWheel { delta, .. } => tab
            .camera_interaction
            .handle_wheel_delta(&mut tab.camera, delta),
        _ => false,
    }
}

fn begin_clip_drag(tab: &mut ViewerTab, viewport_size: Vec2) -> bool {
    if !tab.viewer.viewer_state().clip_plane_enabled {
        tab.clip_plane.selected = false;
        return false;
    }
    let Some(cursor) = tab.cursor_position else {
        return false;
    };
    let inverse = tab
        .camera
        .matrices_for_bounds(tab.current_bounds)
        .view_proj
        .inverse();
    let Some(ray) = scad_scene::cross_section::screen_ray(cursor, viewport_size, inverse) else {
        return false;
    };
    let Some(distance) = tab.clip_plane.ray_intersection(ray.origin, ray.direction) else {
        tab.clip_plane.selected = false;
        return false;
    };
    let hit_point = ray.origin + ray.direction * distance;
    if !tab.clip_plane.contains_point(hit_point) {
        tab.clip_plane.selected = false;
        return false;
    }
    tab.clip_plane.selected = true;
    tab.clip_drag_active = true;
    true
}

fn update_clip_drag(tab: &mut ViewerTab, cursor: Vec2) -> bool {
    if !tab.viewer.viewer_state().clip_plane_enabled || !tab.clip_drag_active {
        return false;
    }
    let previous = tab.cursor_position.unwrap_or(cursor);
    let delta = cursor - previous;
    let distance_scale = tab
        .camera
        .matrices_for_bounds(tab.current_bounds)
        .eye
        .distance(tab.clip_plane.center())
        * 0.0025;
    match tab.clip_edit_mode {
        EditMode::Translate => {
            let amount = (delta.x - delta.y) * distance_scale;
            tab.clip_plane
                .translate_along_normal(amount, tab.ctrl_pressed);
        }
        EditMode::Rotate => {
            let inverse_view = tab
                .camera
                .matrices_for_bounds(tab.current_bounds)
                .view
                .inverse();
            let right = inverse_view.x_axis.xyz().normalize_or_zero();
            let up = inverse_view.y_axis.xyz().normalize_or_zero();
            let axis = if delta.x.abs() >= delta.y.abs() {
                up
            } else {
                right
            };
            tab.clip_plane
                .rotate((delta.x - delta.y) * 0.01, axis, tab.ctrl_pressed);
        }
    }
    tab.cursor_position = Some(cursor);
    true
}

pub(super) fn apply_camera_action(
    camera: &mut OrbitalCamera,
    action: CameraAction,
    bounds: Option<Bounds>,
) {
    match action {
        CameraAction::SetTargetX(v) => camera.set_target_x(v),
        CameraAction::SetTargetY(v) => camera.set_target_y(v),
        CameraAction::SetTargetZ(v) => camera.set_target_z(v),
        CameraAction::SetDistance(v) => camera.set_distance(v),
        CameraAction::SetAzimuth(v) => camera.set_azimuth_degrees(v),
        CameraAction::SetElevation(v) => camera.set_elevation_degrees(v),
        CameraAction::ResetView => camera.reset_view(bounds),
        CameraAction::ViewTop => camera.view_top(),
        CameraAction::ViewBottom => camera.view_bottom(),
        CameraAction::ViewFront => camera.view_front(),
        CameraAction::ViewBack => camera.view_back(),
        CameraAction::ViewLeft => camera.view_left(),
        CameraAction::ViewRight => camera.view_right(),
    }
}

fn viewport_local_cursor(
    position: winit::dpi::PhysicalPosition<f64>,
    viewport_rect: egui::Rect,
) -> Vec2 {
    Vec2::new(
        position.x as f32 - viewport_rect.min.x,
        position.y as f32 - viewport_rect.min.y,
    )
}

pub(super) fn physical_viewport_rect(
    viewport_rect: egui::Rect,
    pixels_per_point: f32,
) -> egui::Rect {
    egui::Rect::from_min_max(
        egui::pos2(
            viewport_rect.min.x * pixels_per_point,
            viewport_rect.min.y * pixels_per_point,
        ),
        egui::pos2(
            viewport_rect.max.x * pixels_per_point,
            viewport_rect.max.y * pixels_per_point,
        ),
    )
}
