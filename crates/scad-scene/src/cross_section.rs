use glam::{Mat4, Quat, Vec2, Vec3, Vec4Swizzles};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditMode {
    Translate,
    Rotate,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClipPlane {
    pub normal: Vec3,
    pub distance: f32,
    pub visible_extent: f32,
    pub selected: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ray {
    pub origin: Vec3,
    pub direction: Vec3,
}

impl Default for ClipPlane {
    fn default() -> Self {
        Self {
            normal: Vec3::Y,
            distance: 0.0,
            visible_extent: 160.0,
            selected: false,
        }
    }
}

impl ClipPlane {
    pub fn translate_along_normal(&mut self, amount: f32, snap: bool) {
        self.distance += snap_distance(amount, snap);
    }

    pub fn rotate(&mut self, angle_radians: f32, axis: Vec3, snap: bool) {
        let snapped = snap_angle(angle_radians, snap);
        let rotation = Quat::from_axis_angle(axis.normalize_or_zero(), snapped);
        self.normal = (rotation * self.normal).normalize_or_zero();
    }

    pub fn ray_intersection(&self, ray_origin: Vec3, ray_direction: Vec3) -> Option<f32> {
        let denominator = self.normal.dot(ray_direction);
        if denominator.abs() < 1e-5 {
            return None;
        }
        let t = (self.distance - self.normal.dot(ray_origin)) / denominator;
        if t >= 0.0 { Some(t) } else { None }
    }

    pub fn signed_distance(&self, point: Vec3) -> f32 {
        self.normal.dot(point) - self.distance
    }

    pub fn center(&self) -> Vec3 {
        self.normal * self.distance
    }

    pub fn basis(&self) -> (Vec3, Vec3) {
        plane_basis(self.normal)
    }

    pub fn corners(&self) -> [Vec3; 4] {
        let center = self.center();
        let (tangent, bitangent) = self.basis();
        let extent = self.visible_extent;
        [
            center - tangent * extent - bitangent * extent,
            center + tangent * extent - bitangent * extent,
            center + tangent * extent + bitangent * extent,
            center - tangent * extent + bitangent * extent,
        ]
    }

    pub fn contains_point(&self, point: Vec3) -> bool {
        if self.signed_distance(point).abs() > 0.001 {
            return false;
        }
        let center = self.center();
        let (tangent, bitangent) = self.basis();
        let local = point - center;
        local.dot(tangent).abs() <= self.visible_extent + 0.001
            && local.dot(bitangent).abs() <= self.visible_extent + 0.001
    }
}

impl EditMode {
    #[allow(dead_code)]
    pub fn toggle(self) -> Self {
        match self {
            Self::Translate => Self::Rotate,
            Self::Rotate => Self::Translate,
        }
    }
}

fn snap_distance(amount: f32, snap: bool) -> f32 {
    if snap { amount.round() } else { amount }
}

fn snap_angle(angle_radians: f32, snap: bool) -> f32 {
    if !snap {
        return angle_radians;
    }
    let step = 5.0_f32.to_radians();
    (angle_radians / step).round() * step
}

fn plane_basis(normal: Vec3) -> (Vec3, Vec3) {
    let helper = if normal.y.abs() < 0.99 {
        Vec3::Y
    } else {
        Vec3::X
    };
    let tangent = normal.cross(helper).normalize_or_zero();
    let bitangent = normal.cross(tangent).normalize_or_zero();
    (tangent, bitangent)
}

pub fn screen_ray(cursor: Vec2, viewport_size: Vec2, inverse_view_proj: Mat4) -> Option<Ray> {
    if viewport_size.x <= 0.0 || viewport_size.y <= 0.0 {
        return None;
    }
    let ndc = Vec2::new(
        (cursor.x / viewport_size.x) * 2.0 - 1.0,
        1.0 - (cursor.y / viewport_size.y) * 2.0,
    );
    let near = inverse_view_proj * glam::Vec4::new(ndc.x, ndc.y, 0.0, 1.0);
    let far = inverse_view_proj * glam::Vec4::new(ndc.x, ndc.y, 1.0, 1.0);
    if near.w.abs() < 1e-5 || far.w.abs() < 1e-5 {
        return None;
    }
    let origin = (near / near.w).xyz();
    let end = (far / far.w).xyz();
    Some(Ray {
        origin,
        direction: (end - origin).normalize_or_zero(),
    })
}
