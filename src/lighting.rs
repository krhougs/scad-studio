use bytemuck::{Pod, Zeroable};
use glam::Vec3;

pub const MAX_LIGHTS: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LightKind {
    Ambient = 0,
    Directional = 1,
    Spot = 2,
    Point = 3,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Light {
    pub kind: LightKind,
    pub color: Vec3,
    pub intensity: f32,
    pub position: Vec3,
    pub direction: Vec3,
    pub range: f32,
    pub spot_inner_cos: f32,
    pub spot_outer_cos: f32,
    pub casts_shadow: bool,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct LightRaw {
    pub kind_flags: [u32; 4],
    pub color_intensity: [f32; 4],
    pub position_range: [f32; 4],
    pub direction_spot: [f32; 4],
    pub extra: [f32; 4],
}

#[derive(Debug, Clone, Copy)]
pub struct LightingState {
    pub lights: [LightRaw; MAX_LIGHTS],
    pub light_count: u32,
    pub shadow_light_index: u32,
}

pub fn default_lights() -> [Light; 2] {
    [
        Light {
            kind: LightKind::Ambient,
            color: Vec3::splat(1.0),
            intensity: 0.18,
            position: Vec3::ZERO,
            direction: Vec3::ZERO,
            range: 0.0,
            spot_inner_cos: 0.0,
            spot_outer_cos: 0.0,
            casts_shadow: false,
        },
        Light {
            kind: LightKind::Directional,
            color: Vec3::splat(1.0),
            intensity: 0.82,
            position: Vec3::ZERO,
            direction: Vec3::new(-0.5, -0.8, -0.2).normalize(),
            range: 0.0,
            spot_inner_cos: 0.0,
            spot_outer_cos: 0.0,
            casts_shadow: true,
        },
    ]
}

pub fn encode_lights(lights: &[Light]) -> LightingState {
    let mut raw = [LightRaw::zeroed(); MAX_LIGHTS];
    let mut shadow_light_index = 0;
    let mut shadow_found = false;
    let mut count = 0;
    for (index, light) in lights.iter().take(MAX_LIGHTS).enumerate() {
        raw[index] = encode_light(*light);
        if light.casts_shadow && !shadow_found {
            shadow_light_index = index as u32;
            shadow_found = true;
        }
        count += 1;
    }
    LightingState {
        lights: raw,
        light_count: count,
        shadow_light_index: if shadow_found { shadow_light_index + 1 } else { 0 },
    }
}

impl LightingState {
    pub fn primary_shadow_light(&self) -> Option<Light> {
        if self.shadow_light_index == 0 {
            return None;
        }
        let raw = self.lights[(self.shadow_light_index - 1) as usize];
        Some(Light {
            kind: match raw.kind_flags[0] {
                0 => LightKind::Ambient,
                1 => LightKind::Directional,
                2 => LightKind::Spot,
                _ => LightKind::Point,
            },
            color: Vec3::new(
                raw.color_intensity[0],
                raw.color_intensity[1],
                raw.color_intensity[2],
            ),
            intensity: raw.color_intensity[3],
            position: Vec3::new(
                raw.position_range[0],
                raw.position_range[1],
                raw.position_range[2],
            ),
            direction: Vec3::new(
                raw.direction_spot[0],
                raw.direction_spot[1],
                raw.direction_spot[2],
            ),
            range: raw.position_range[3],
            spot_inner_cos: raw.direction_spot[3],
            spot_outer_cos: raw.extra[0],
            casts_shadow: raw.kind_flags[1] == 1,
        })
    }
}

fn encode_light(light: Light) -> LightRaw {
    LightRaw {
        kind_flags: [
            light.kind as u32,
            u32::from(light.casts_shadow),
            0,
            0,
        ],
        color_intensity: [light.color.x, light.color.y, light.color.z, light.intensity],
        position_range: [light.position.x, light.position.y, light.position.z, light.range],
        direction_spot: [
            light.direction.x,
            light.direction.y,
            light.direction.z,
            light.spot_inner_cos,
        ],
        extra: [light.spot_outer_cos, 0.0, 0.0, 0.0],
    }
}
