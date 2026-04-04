#![allow(dead_code)]

#[path = "../src/grid.rs"]
mod grid;

use grid::{BUILD_PLATE_SIZE, generate_build_plate_vertices, generate_grid_vertices};

#[test]
fn grid_vertices_cover_both_axes_symmetrically() {
    let vertices = generate_grid_vertices(1, 10.0);

    assert_eq!(vertices.len(), 12);
    assert!(
        vertices
            .iter()
            .any(|vertex| vertex.position == [-10.0, 0.0, -10.0])
    );
    assert!(
        vertices
            .iter()
            .any(|vertex| vertex.position == [10.0, 0.0, 10.0])
    );
}

#[test]
fn build_plate_vertices_match_256_square_outline() {
    let vertices = generate_build_plate_vertices(BUILD_PLATE_SIZE);

    assert_eq!(vertices.len(), 8);
    assert!(
        vertices
            .iter()
            .any(|vertex| vertex.position == [-128.0, 0.0, -128.0])
    );
    assert!(
        vertices
            .iter()
            .any(|vertex| vertex.position == [128.0, 0.0, 128.0])
    );
}
