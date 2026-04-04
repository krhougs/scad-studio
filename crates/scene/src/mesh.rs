use std::fmt;

use bytemuck::{Pod, Zeroable};
use glam::Vec3;

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub color: [f32; 4],
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bounds {
    pub min: Vec3,
    pub max: Vec3,
}

#[derive(Debug, Clone)]
pub struct MeshData {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
    pub bounds: Bounds,
}

#[derive(Debug, Clone, Copy)]
pub struct MeshTriangle {
    pub positions: [[f32; 3]; 3],
    pub normal: [f32; 3],
    pub colors: [Option<[f32; 4]>; 3],
}

#[derive(Debug)]
pub struct MeshError(String);

impl MeshData {
    pub fn from_triangles(triangles: &[MeshTriangle]) -> Result<Self, MeshError> {
        if triangles.is_empty() {
            return Err(MeshError("网格中没有可渲染的三角面".into()));
        }
        let mut vertices = Vec::with_capacity(triangles.len() * 3);
        let mut indices = Vec::with_capacity(triangles.len() * 3);
        let mut bounds = Bounds::empty();
        for (triangle_index, triangle) in triangles.iter().enumerate() {
            for (vertex_index, position) in triangle.positions.iter().copied().enumerate() {
                bounds.include(Vec3::from_array(position));
                vertices.push(Vertex {
                    position,
                    normal: triangle.normal,
                    color: triangle.colors[vertex_index].unwrap_or([0.0, 0.0, 0.0, -1.0]),
                });
            }
            let base = (triangle_index * 3) as u32;
            indices.extend_from_slice(&[base, base + 1, base + 2]);
        }
        Ok(Self {
            vertices,
            indices,
            bounds,
        })
    }
}

impl Bounds {
    pub fn center(&self) -> Vec3 {
        (self.min + self.max) * 0.5
    }

    pub fn radius(&self) -> f32 {
        (self.max - self.min).length() * 0.5
    }

    fn empty() -> Self {
        Self {
            min: Vec3::splat(f32::INFINITY),
            max: Vec3::splat(f32::NEG_INFINITY),
        }
    }

    fn include(&mut self, point: Vec3) {
        self.min = self.min.min(point);
        self.max = self.max.max(point);
    }
}

#[allow(dead_code)]
pub(crate) fn openscad_to_viewer(vector: [f32; 3]) -> [f32; 3] {
    [vector[0], vector[2], -vector[1]]
}

impl std::error::Error for MeshError {}

impl fmt::Display for MeshError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
