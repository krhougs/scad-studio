use glam::{Mat4, Vec2, Vec3};
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};

use crate::mesh::Bounds;

const MIN_DISTANCE: f32 = 0.05;
const MAX_DISTANCE: f32 = 5_000.0;
const ROTATE_SPEED: f32 = 0.01;
const PAN_SPEED: f32 = 0.002;
const ZOOM_SPEED: f32 = 0.12;
const MIN_PITCH: f32 = -1.54;
const MAX_PITCH: f32 = 1.54;

#[derive(Debug, Clone, Copy)]
pub struct CameraMatrices {
    pub view_proj: Mat4,
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
        }
    }

    pub fn set_aspect_ratio(&mut self, aspect_ratio: f32) {
        self.aspect_ratio = aspect_ratio.max(0.1);
    }

    pub fn matrices(&self) -> CameraMatrices {
        let eye = self.eye_position();
        let view = Mat4::look_at_rh(eye, self.target, Vec3::Y);
        let projection =
            Mat4::perspective_rh(self.fov_y_radians, self.aspect_ratio.max(0.1), 0.01, 10_000.0);
        CameraMatrices {
            view_proj: projection * view,
            eye,
        }
    }

    pub fn fit_bounds(&mut self, bounds: Bounds) {
        self.target = bounds.center();
        let radius = bounds.radius().max(0.25);
        let vertical_half_fov = self.fov_y_radians * 0.5;
        let horizontal_half_fov = (vertical_half_fov.tan() * self.aspect_ratio.max(0.1)).atan();
        let limiting_half_fov = vertical_half_fov.min(horizontal_half_fov);
        let fit_distance = radius / limiting_half_fov.tan();
        self.distance = (fit_distance * 1.35).clamp(MIN_DISTANCE, MAX_DISTANCE);
    }

    pub fn orbit(&mut self, delta: Vec2) {
        self.azimuth -= delta.x * ROTATE_SPEED;
        self.elevation = (self.elevation - delta.y * ROTATE_SPEED).clamp(MIN_PITCH, MAX_PITCH);
    }

    pub fn pan(&mut self, delta: Vec2) {
        let eye = self.eye_position();
        let forward = (self.target - eye).normalize_or_zero();
        let right = forward.cross(Vec3::Y).normalize_or_zero();
        let up = right.cross(forward).normalize_or_zero();
        let scale = self.distance.max(0.25) * PAN_SPEED;
        self.target += (-delta.x * scale) * right + (delta.y * scale) * up;
    }

    pub fn zoom(&mut self, delta: f32) {
        let factor = (1.0 - delta * ZOOM_SPEED).clamp(0.2, 5.0);
        self.distance = (self.distance * factor).clamp(MIN_DISTANCE, MAX_DISTANCE);
    }

    fn eye_position(&self) -> Vec3 {
        let x = self.distance * self.elevation.cos() * self.azimuth.cos();
        let y = self.distance * self.elevation.sin();
        let z = self.distance * self.elevation.cos() * self.azimuth.sin();
        self.target + Vec3::new(x, y, z)
    }
}

impl CameraInteraction {
    pub fn handle_event(&mut self, camera: &mut OrbitalCamera, event: &WindowEvent) -> bool {
        match event {
            WindowEvent::MouseInput { state, button, .. } => self.handle_mouse_input(*state, *button),
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
