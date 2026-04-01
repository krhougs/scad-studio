#![allow(dead_code)]

#[path = "../src/lighting.rs"]
mod lighting;

use glam::Vec3;
use lighting::{Light, LightKind, default_lights, encode_lights};

#[test]
fn default_lights_include_ambient_and_directional() {
    let lights = default_lights();

    assert_eq!(lights[0].kind, LightKind::Ambient);
    assert_eq!(lights[1].kind, LightKind::Directional);
    assert!(lights[1].casts_shadow);
}

#[test]
fn encode_lights_marks_shadow_light_index_from_first_shadow_caster() {
    let lights = [
        Light {
            kind: LightKind::Ambient,
            color: Vec3::splat(1.0),
            intensity: 0.2,
            position: Vec3::ZERO,
            direction: Vec3::ZERO,
            range: 0.0,
            spot_inner_cos: 0.0,
            spot_outer_cos: 0.0,
            casts_shadow: false,
        },
        Light {
            kind: LightKind::Directional,
            color: Vec3::ONE,
            intensity: 1.0,
            position: Vec3::ZERO,
            direction: Vec3::new(-1.0, -1.0, 0.0).normalize(),
            range: 0.0,
            spot_inner_cos: 0.0,
            spot_outer_cos: 0.0,
            casts_shadow: true,
        },
    ];

    let encoded = encode_lights(&lights);

    assert_eq!(encoded.light_count, 2);
    assert_eq!(encoded.shadow_light_index, 2);
}

#[test]
fn encode_lights_preserves_spotlight_cone_cosines() {
    let lights = [Light {
        kind: LightKind::Spot,
        color: Vec3::ONE,
        intensity: 2.0,
        position: Vec3::new(1.0, 2.0, 3.0),
        direction: Vec3::new(0.0, -1.0, 0.0),
        range: 10.0,
        spot_inner_cos: 0.9,
        spot_outer_cos: 0.7,
        casts_shadow: false,
    }];

    let encoded = encode_lights(&lights);

    assert_eq!(encoded.lights[0].direction_spot[3], 0.9);
    assert_eq!(encoded.lights[0].extra[0], 0.7);
    assert_eq!(encoded.lights[0].position_range[3], 10.0);
}
