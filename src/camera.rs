use std::f32::consts::{PI, TAU};

use glam::{Mat4, Vec2, Vec3, Vec4Swizzles};
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};

use crate::app::ProjectionMode;
use crate::mesh::Bounds;

const MIN_DISTANCE: f32 = 0.05;
const MAX_DISTANCE: f32 = 5_000.0;
const MIN_CLIP_NEAR: f32 = 0.05;
const CLIP_PADDING_FACTOR: f32 = 0.2;
const MIN_CLIP_PADDING: f32 = 1.0;
const ROTATE_SPEED: f32 = 0.01;
const PAN_SPEED: f32 = 0.002;
const ZOOM_SPEED: f32 = 0.12;

#[derive(Debug, Clone, Copy)]
pub struct CameraMatrices {
    pub view_proj: Mat4,
    pub view: Mat4,
    #[allow(dead_code)]
    pub projection: Mat4,
    pub eye: Vec3,
}

#[derive(Debug, Clone, Copy)]
pub struct OrbitalCamera {
    target: Vec3,
    distance: f32,
    azimuth: f32,
    elevation: f32,
    aspect_ratio: f32,
    fov_y_radians: f32,
    projection_mode: ProjectionMode,
}

#[derive(Debug, Default)]
pub struct CameraInteraction {
    last_cursor: Option<Vec2>,
    drag_mode: Option<DragMode>,
}

#[derive(Debug, Clone, Copy)]
enum DragMode {
    Orbit,
    Pan,
}

impl OrbitalCamera {
    pub fn new(aspect_ratio: f32) -> Self {
        Self {
            target: Vec3::ZERO,
            distance: 3.0,
            azimuth: 0.7,
            elevation: 0.45,
            aspect_ratio: aspect_ratio.max(0.1),
            fov_y_radians: 45.0_f32.to_radians(),
            projection_mode: ProjectionMode::Perspective,
        }
    }

    pub fn set_aspect_ratio(&mut self, aspect_ratio: f32) {
        self.aspect_ratio = aspect_ratio.max(0.1);
    }

    pub fn set_projection_mode(&mut self, projection_mode: ProjectionMode) {
        self.projection_mode = projection_mode;
    }

    pub fn eye(&self) -> Vec3 {
        self.compute_eye_position()
    }

    pub fn target(&self) -> Vec3 {
        self.target
    }

    pub fn distance(&self) -> f32 {
        self.distance
    }

    pub fn azimuth_degrees(&self) -> f32 {
        self.azimuth.to_degrees()
    }

    pub fn elevation_degrees(&self) -> f32 {
        self.elevation.to_degrees()
    }

    pub fn set_target_x(&mut self, x: f32) {
        self.target.x = x;
    }

    pub fn set_target_y(&mut self, y: f32) {
        self.target.y = y;
    }

    pub fn set_target_z(&mut self, z: f32) {
        self.target.z = z;
    }

    pub fn set_distance(&mut self, distance: f32) {
        self.distance = distance.clamp(MIN_DISTANCE, MAX_DISTANCE);
    }

    pub fn set_azimuth_degrees(&mut self, degrees: f32) {
        self.azimuth = degrees.to_radians();
    }

    pub fn set_elevation_degrees(&mut self, degrees: f32) {
        self.elevation = degrees.clamp(-89.9, 89.9).to_radians();
    }

    pub fn reset_view(&mut self, bounds: Option<Bounds>) {
        if let Some(bounds) = bounds {
            self.fit_bounds(bounds);
        } else {
            self.target = Vec3::ZERO;
            self.distance = 3.0;
        }
        self.azimuth = 0.7;
        self.elevation = 0.45;
    }

    pub fn view_top(&mut self) {
        self.elevation = std::f32::consts::FRAC_PI_2 * 0.95;
    }

    pub fn view_bottom(&mut self) {
        self.elevation = -std::f32::consts::FRAC_PI_2 * 0.95;
    }

    pub fn view_front(&mut self) {
        self.azimuth = 0.0;
        self.elevation = 0.0;
    }

    pub fn view_back(&mut self) {
        self.azimuth = std::f32::consts::PI;
        self.elevation = 0.0;
    }

    pub fn view_left(&mut self) {
        self.azimuth = std::f32::consts::FRAC_PI_2;
        self.elevation = 0.0;
    }

    pub fn view_right(&mut self) {
        self.azimuth = -std::f32::consts::FRAC_PI_2;
        self.elevation = 0.0;
    }

    pub fn matrices(&self) -> CameraMatrices {
        self.matrices_for_bounds(None)
    }

    pub fn matrices_for_bounds(&self, bounds: Option<Bounds>) -> CameraMatrices {
        let eye = self.compute_eye_position();
        let view = Mat4::look_at_rh(eye, self.target, self.orbit_up());
        let (near, far) = self.clipping_planes(bounds);
        let projection = match self.projection_mode {
            ProjectionMode::Perspective => {
                Mat4::perspective_rh(self.fov_y_radians, self.aspect_ratio.max(0.1), near, far)
            }
            ProjectionMode::Orthographic => self.orthographic_projection(near, far),
        };
        CameraMatrices {
            view_proj: projection * view,
            view,
            projection,
            eye,
        }
    }

    pub fn clipping_planes(&self, bounds: Option<Bounds>) -> (f32, f32) {
        let Some(bounds) = bounds else {
            return (MIN_CLIP_NEAR, 10_000.0);
        };
        let eye = self.compute_eye_position();
        let view = Mat4::look_at_rh(eye, self.target, self.orbit_up());
        let mut min_depth = f32::INFINITY;
        let mut max_depth: f32 = 0.0;
        for corner in bounds_corners(bounds) {
            let view_space = view * corner.extend(1.0);
            let depth = -view_space.z;
            min_depth = min_depth.min(depth);
            max_depth = max_depth.max(depth);
        }
        let padding = bounds
            .radius()
            .mul_add(CLIP_PADDING_FACTOR, MIN_CLIP_PADDING);
        let near = (min_depth - padding).max(MIN_CLIP_NEAR);
        let far = (max_depth + padding).max(near + 1.0);
        (near, far)
    }

    pub fn fit_bounds(&mut self, bounds: Bounds) {
        self.target = bounds.center();
        let radius = bounds.radius().max(0.25);
        let fit_distance = match self.projection_mode {
            ProjectionMode::Perspective => {
                let vertical_half_fov = self.fov_y_radians * 0.5;
                let horizontal_half_fov =
                    (vertical_half_fov.tan() * self.aspect_ratio.max(0.1)).atan();
                let limiting_half_fov = vertical_half_fov.min(horizontal_half_fov);
                radius / limiting_half_fov.tan()
            }
            ProjectionMode::Orthographic => {
                let eye = self.compute_eye_position();
                let view = Mat4::look_at_rh(eye, self.target, self.orbit_up());
                let mut min = Vec2::splat(f32::INFINITY);
                let mut max = Vec2::splat(f32::NEG_INFINITY);
                for corner in bounds_corners(bounds) {
                    let view_space = view * corner.extend(1.0);
                    min = min.min(view_space.xy());
                    max = max.max(view_space.xy());
                }
                let half_extent = (max - min) * 0.5;
                let half_height = half_extent
                    .y
                    .max(half_extent.x / self.aspect_ratio.max(0.1))
                    .max(0.25);
                half_height / (self.fov_y_radians * 0.5).tan()
            }
        };
        self.distance = (fit_distance * 1.35).clamp(MIN_DISTANCE, MAX_DISTANCE);
    }

    pub fn orbit(&mut self, delta: Vec2) {
        self.azimuth = wrap_angle(self.azimuth - delta.x * ROTATE_SPEED);
        self.elevation = wrap_angle(self.elevation - delta.y * ROTATE_SPEED);
    }

    pub fn pan(&mut self, delta: Vec2) {
        let eye = self.compute_eye_position();
        let forward = (self.target - eye).normalize_or_zero();
        let right = forward.cross(self.orbit_up()).normalize_or_zero();
        let up = right.cross(forward).normalize_or_zero();
        let scale = self.distance.max(0.25) * PAN_SPEED;
        self.target += (-delta.x * scale) * right + (delta.y * scale) * up;
    }

    pub fn zoom(&mut self, delta: f32) {
        let factor = (1.0 - delta * ZOOM_SPEED).clamp(0.2, 5.0);
        self.distance = (self.distance * factor).clamp(MIN_DISTANCE, MAX_DISTANCE);
    }

    fn compute_eye_position(&self) -> Vec3 {
        let x = self.distance * self.elevation.cos() * self.azimuth.cos();
        let y = self.distance * self.elevation.sin();
        let z = self.distance * self.elevation.cos() * self.azimuth.sin();
        self.target + Vec3::new(x, y, z)
    }

    fn orbit_up(&self) -> Vec3 {
        Vec3::new(
            -self.elevation.sin() * self.azimuth.cos(),
            self.elevation.cos(),
            -self.elevation.sin() * self.azimuth.sin(),
        )
        .normalize_or_zero()
    }

    fn orthographic_projection(&self, near: f32, far: f32) -> Mat4 {
        let half_height = (self.distance * (self.fov_y_radians * 0.5).tan()).max(0.01);
        let half_width = (half_height * self.aspect_ratio.max(0.1)).max(0.01);
        Mat4::orthographic_rh(
            -half_width,
            half_width,
            -half_height,
            half_height,
            near,
            far,
        )
    }
}

fn wrap_angle(angle: f32) -> f32 {
    let mut wrapped = angle % TAU;
    if wrapped <= -PI {
        wrapped += TAU;
    } else if wrapped > PI {
        wrapped -= TAU;
    }
    wrapped
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

impl CameraInteraction {
    pub fn handle_event(&mut self, camera: &mut OrbitalCamera, event: &WindowEvent) -> bool {
        match event {
            WindowEvent::MouseInput { state, button, .. } => {
                self.handle_mouse_input(*state, *button)
            }
            WindowEvent::CursorMoved { position, .. } => self.handle_cursor(camera, *position),
            WindowEvent::MouseWheel { delta, .. } => self.handle_wheel(camera, delta),
            _ => false,
        }
    }

    fn handle_mouse_input(&mut self, state: ElementState, button: MouseButton) -> bool {
        if state == ElementState::Released {
            self.drag_mode = None;
            self.last_cursor = None;
            return false;
        }
        self.drag_mode = match button {
            MouseButton::Left => Some(DragMode::Orbit),
            MouseButton::Middle | MouseButton::Right => Some(DragMode::Pan),
            _ => None,
        };
        self.drag_mode.is_some()
    }

    fn handle_cursor(
        &mut self,
        camera: &mut OrbitalCamera,
        position: winit::dpi::PhysicalPosition<f64>,
    ) -> bool {
        let current = Vec2::new(position.x as f32, position.y as f32);
        let Some(last) = self.last_cursor.replace(current) else {
            return false;
        };
        let Some(mode) = self.drag_mode else {
            return false;
        };
        let delta = current - last;
        match mode {
            DragMode::Orbit => camera.orbit(delta),
            DragMode::Pan => camera.pan(delta),
        }
        true
    }

    fn handle_wheel(&mut self, camera: &mut OrbitalCamera, delta: &MouseScrollDelta) -> bool {
        let amount = match delta {
            MouseScrollDelta::LineDelta(_, y) => *y,
            MouseScrollDelta::PixelDelta(position) => position.y as f32 / 120.0,
        };
        camera.zoom(amount);
        true
    }
}
