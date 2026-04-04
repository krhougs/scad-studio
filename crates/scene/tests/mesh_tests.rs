use glam::Vec3;
use scene::mesh::{MeshData, MeshTriangle};

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
