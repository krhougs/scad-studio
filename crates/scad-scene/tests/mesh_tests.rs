use glam::Vec3;
use scad_scene::{MeshData, MeshTriangle};
use std::io::Cursor;
use stl_io::{Normal, Triangle, Vertex};

#[test]
fn from_triangles_builds_bounds_and_indices() {
    let triangles = [MeshTriangle {
        positions: [[0.0, 0.0, 0.0], [2.0, 0.0, 0.0], [0.0, 3.0, 0.0]],
        normal: [0.0, 0.0, 1.0],
        colors: [Some([1.0, 0.0, 0.0, 1.0]); 3],
    }];

    let mesh = MeshData::from_triangles(&triangles).expect("triangle mesh should build");

    assert_eq!(mesh.vertices.len(), 3);
    assert_eq!(mesh.indices, vec![0, 1, 2]);
    assert_eq!(mesh.bounds.min, Vec3::ZERO);
    assert_eq!(mesh.bounds.max, Vec3::new(2.0, 3.0, 0.0));
    assert!(
        mesh.vertices
            .iter()
            .all(|vertex| vertex.color == [1.0, 0.0, 0.0, 1.0])
    );
}

#[test]
fn from_indexed_buffers_builds_renderable_mesh_from_raw_buffers() {
    let mesh = MeshData::from_indexed_buffers(
        vec![
            [0.0, 0.0, 0.0],
            [2.0, 0.0, 0.0],
            [0.0, 3.0, 0.0],
            [0.0, 0.0, 4.0],
        ],
        vec![[0.0, 0.0, 1.0]; 4],
        vec![[0.2, 0.4, 0.6, 1.0]; 4],
        vec![0, 1, 2, 0, 2, 3],
    )
    .expect("indexed buffers should build renderable mesh");

    assert_eq!(mesh.vertices.len(), 4);
    assert_eq!(mesh.indices, vec![0, 1, 2, 0, 2, 3]);
    assert_eq!(mesh.bounds.min, Vec3::ZERO);
    assert_eq!(mesh.bounds.max, Vec3::new(2.0, 3.0, 4.0));
    assert_eq!(mesh.vertices[0].normal, [0.0, 0.0, 1.0]);
    assert_eq!(mesh.vertices[0].color, [0.2, 0.4, 0.6, 1.0]);
}

#[test]
fn from_indexed_buffers_defaults_missing_normals_to_project_up() {
    let mesh = MeshData::from_indexed_buffers(
        vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
        Vec::new(),
        Vec::new(),
        vec![0, 1, 2],
    )
    .expect("indexed buffers should build renderable mesh");

    assert!(mesh.vertices.iter().all(|vertex| {
        vertex.normal == [0.0, 0.0, 1.0] && vertex.color == [0.0, 0.0, 0.0, -1.0]
    }));
}

#[test]
fn from_triangles_defaults_degenerate_normals_to_project_up() {
    let triangles = [MeshTriangle {
        positions: [[2.0, 3.0, 5.0], [2.0, 3.0, 5.0], [2.0, 3.0, 5.0]],
        normal: [0.0, 0.0, 0.0],
        colors: [None; 3],
    }];

    let mesh = MeshData::from_triangles(&triangles).expect("triangle mesh should build");

    assert!(
        mesh.vertices
            .iter()
            .all(|vertex| vertex.normal == [0.0, 0.0, 1.0])
    );
}

#[test]
fn from_triangles_smooths_normals_for_shared_vertices_with_small_angle() {
    let triangle_a = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
    let triangle_b = [[1.0, 0.0, 0.0], [1.0, 1.0, 0.4], [0.0, 1.0, 0.0]];
    let normal_a = triangle_normal(triangle_a);
    let normal_b = triangle_normal(triangle_b);
    let triangles = [
        MeshTriangle {
            positions: triangle_a,
            normal: normal_a,
            colors: [None; 3],
        },
        MeshTriangle {
            positions: triangle_b,
            normal: normal_b,
            colors: [None; 3],
        },
    ];

    let mesh = MeshData::from_triangles(&triangles).expect("triangle mesh should build");

    assert!(approx_eq_normal(
        mesh.vertices[1].normal,
        mesh.vertices[3].normal
    ));
    assert!(approx_eq_normal(
        mesh.vertices[2].normal,
        mesh.vertices[5].normal
    ));
    assert!(!approx_eq_normal(mesh.vertices[1].normal, normal_a));
    assert!(!approx_eq_normal(mesh.vertices[3].normal, normal_b));
    assert!(approx_eq_normal(mesh.vertices[0].normal, normal_a));
    assert!(approx_eq_normal(mesh.vertices[4].normal, normal_b));
}

#[test]
fn from_triangles_keeps_sharp_edge_normals_separate() {
    let triangle_a = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
    let triangle_b = [[1.0, 0.0, 0.0], [1.0, 0.0, 1.0], [0.0, 1.0, 0.0]];
    let normal_a = triangle_normal(triangle_a);
    let normal_b = triangle_normal(triangle_b);
    let triangles = [
        MeshTriangle {
            positions: triangle_a,
            normal: normal_a,
            colors: [None; 3],
        },
        MeshTriangle {
            positions: triangle_b,
            normal: normal_b,
            colors: [None; 3],
        },
    ];

    let mesh = MeshData::from_triangles(&triangles).expect("triangle mesh should build");

    assert!(approx_eq_normal(mesh.vertices[1].normal, normal_a));
    assert!(approx_eq_normal(mesh.vertices[3].normal, normal_b));
    assert!(approx_eq_normal(mesh.vertices[2].normal, normal_a));
    assert!(approx_eq_normal(mesh.vertices[5].normal, normal_b));
}

#[test]
fn mesh_data_splits_opaque_and_transparent_triangle_indices() {
    let triangles = [
        MeshTriangle {
            positions: [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            normal: [0.0, 0.0, 1.0],
            colors: [Some([1.0, 0.0, 0.0, 1.0]); 3],
        },
        MeshTriangle {
            positions: [[0.0, 0.0, 1.0], [1.0, 0.0, 1.0], [0.0, 1.0, 1.0]],
            normal: [0.0, 0.0, 1.0],
            colors: [Some([0.0, 0.0, 0.0, 0.5]); 3],
        },
    ];

    let mesh = MeshData::from_triangles(&triangles).expect("triangle mesh should build");
    let (opaque, transparent) = mesh.triangle_index_partitions();

    assert_eq!(opaque, vec![0, 1, 2]);
    assert_eq!(transparent, vec![3, 4, 5]);
}

#[test]
fn mesh_data_sorts_transparent_triangles_back_to_front_for_eye_position() {
    let triangles = [
        MeshTriangle {
            positions: [[0.0, 0.0, 5.0], [1.0, 0.0, 5.0], [0.0, 1.0, 5.0]],
            normal: [0.0, 0.0, 1.0],
            colors: [Some([0.0, 0.0, 0.0, 0.5]); 3],
        },
        MeshTriangle {
            positions: [[0.0, 0.0, 1.0], [1.0, 0.0, 1.0], [0.0, 1.0, 1.0]],
            normal: [0.0, 0.0, 1.0],
            colors: [Some([0.0, 0.0, 0.0, 0.5]); 3],
        },
    ];

    let mesh = MeshData::from_triangles(&triangles).expect("triangle mesh should build");
    let sorted = mesh.sorted_transparent_triangle_indices([0.0, 0.0, 10.0]);

    assert_eq!(sorted, vec![3, 4, 5, 0, 1, 2]);
}

#[test]
fn mesh_data_treats_zero_alpha_vertex_color_as_opaque_for_solid_partition() {
    let triangles = [MeshTriangle {
        positions: [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
        normal: [0.0, 0.0, 1.0],
        colors: [Some([1.0, 0.0, 0.0, 0.0]); 3],
    }];

    let mesh = MeshData::from_triangles(&triangles).expect("triangle mesh should build");
    let (opaque, transparent) = mesh.triangle_index_partitions();

    assert_eq!(opaque, vec![0, 1, 2]);
    assert!(transparent.is_empty());
}

#[test]
fn mesh_data_treats_near_opaque_triangles_as_opaque_for_solid_partition() {
    let triangles = [MeshTriangle {
        positions: [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
        normal: [0.0, 0.0, 1.0],
        colors: [Some([0.1, 0.1, 0.2, 0.95]); 3],
    }];

    let mesh = MeshData::from_triangles(&triangles).expect("triangle mesh should build");
    let (opaque, transparent) = mesh.triangle_index_partitions();

    assert_eq!(opaque, vec![0, 1, 2]);
    assert!(transparent.is_empty());
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

    let mesh =
        scad_scene::mesh::load_stl_from_reader(&mut reader).expect("binary stl should parse");

    assert_eq!(mesh.vertices.len(), 3);
    assert_eq!(mesh.indices, vec![0, 1, 2]);
    assert!(
        mesh.vertices
            .iter()
            .all(|vertex| vertex.color == [0.0, 0.0, 0.0, -1.0])
    );
}

#[test]
fn load_stl_from_reader_preserves_project_coordinates() {
    let positions = [[2.0, 3.0, 5.0], [7.0, 11.0, 13.0], [17.0, 19.0, 29.0]];
    let triangles = [Triangle {
        normal: Normal::new(triangle_normal(positions)),
        vertices: positions.map(Vertex::new),
    }];
    let mut bytes = Vec::new();
    stl_io::write_stl(&mut bytes, triangles.iter()).expect("binary stl should be written");
    let mut reader = Cursor::new(bytes);

    let mesh =
        scad_scene::mesh::load_stl_from_reader(&mut reader).expect("binary stl should parse");

    assert_eq!(mesh.vertices[0].position, [2.0, 3.0, 5.0]);
    assert_eq!(mesh.vertices[1].position, [7.0, 11.0, 13.0]);
    assert_eq!(mesh.vertices[2].position, [17.0, 19.0, 29.0]);
    assert!(approx_eq_normal(mesh.vertices[0].normal, triangle_normal(positions)));
    assert_eq!(mesh.bounds.min, Vec3::new(2.0, 3.0, 5.0));
    assert_eq!(mesh.bounds.max, Vec3::new(17.0, 19.0, 29.0));
    assert!(
        mesh.vertices
            .iter()
            .all(|vertex| vertex.color == [0.0, 0.0, 0.0, -1.0])
    );
}

fn triangle_normal(positions: [[f32; 3]; 3]) -> [f32; 3] {
    let a = Vec3::from_array(positions[0]);
    let b = Vec3::from_array(positions[1]);
    let c = Vec3::from_array(positions[2]);
    (b - a).cross(c - a).normalize().to_array()
}

fn approx_eq_normal(left: [f32; 3], right: [f32; 3]) -> bool {
    Vec3::from_array(left).distance(Vec3::from_array(right)) < 0.0001
}
