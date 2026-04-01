#![allow(dead_code)]

#[path = "../src/mesh.rs"]
mod mesh;

use glam::Vec3;
use mesh::{MeshData, MeshTriangle};
use std::io::Cursor;
use stl_io::{Normal, Triangle, Vertex};

#[test]
fn from_triangles_builds_bounds_and_indices() {
    let triangles = [MeshTriangle {
        positions: [[0.0, 0.0, 0.0], [2.0, 0.0, 0.0], [0.0, 3.0, 0.0]],
        normal: [0.0, 0.0, 1.0],
    }];

    let mesh = MeshData::from_triangles(&triangles).expect("triangle mesh should build");

    assert_eq!(mesh.vertices.len(), 3);
    assert_eq!(mesh.indices, vec![0, 1, 2]);
    assert_eq!(mesh.bounds.min, Vec3::ZERO);
    assert_eq!(mesh.bounds.max, Vec3::new(2.0, 3.0, 0.0));
}

#[test]
fn load_stl_from_reader_parses_binary_stl_bytes() {
    let triangles = [Triangle {
        normal: Normal::new([0.0, 0.0, 1.0]),
        vertices: [
            Vertex::new([0.0, 0.0, 0.0]),
            Vertex::new([1.0, 0.0, 0.0]),
            Vertex::new([0.0, 1.0, 0.0]),
        ],
    }];
    let mut bytes = Vec::new();
    stl_io::write_stl(&mut bytes, triangles.iter()).expect("binary stl should be written");
    let mut reader = Cursor::new(bytes);

    let mesh = mesh::load_stl_from_reader(&mut reader).expect("binary stl should parse");

    assert_eq!(mesh.vertices.len(), 3);
    assert_eq!(mesh.indices, vec![0, 1, 2]);
}
