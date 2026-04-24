use std::{
    collections::HashMap,
    fmt,
    fs::File,
    io::{BufReader, Read, Seek},
    path::Path,
};

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

const NORMAL_SMOOTHING_DOT_THRESHOLD: f32 = 0.5;
const SOLID_TRANSPARENCY_ALPHA_THRESHOLD: f32 = 0.9;
const DEFAULT_NORMAL: [f32; 3] = [0.0, 0.0, 1.0];

#[derive(Clone, Copy)]
struct VertexNormalSample {
    vertex_index: usize,
    face_normal: Vec3,
    weighted_normal: Vec3,
}

impl MeshData {
    pub fn from_triangles(triangles: &[MeshTriangle]) -> Result<Self, MeshError> {
        if triangles.is_empty() {
            return Err(MeshError("网格中没有可渲染的三角面".into()));
        }
        let mut vertices = Vec::with_capacity(triangles.len() * 3);
        let mut indices = Vec::with_capacity(triangles.len() * 3);
        let mut bounds = Bounds::empty();
        let mut shared_normals: HashMap<[u32; 3], Vec<VertexNormalSample>> = HashMap::new();
        for (triangle_index, triangle) in triangles.iter().enumerate() {
            let face_normal = normalized_normal(triangle.normal);
            let weighted_normal = triangle_weighted_normal(triangle, face_normal);
            for (vertex_index, position) in triangle.positions.iter().copied().enumerate() {
                bounds.include(Vec3::from_array(position));
                let output_index = vertices.len();
                vertices.push(Vertex {
                    position,
                    normal: face_normal.to_array(),
                    color: triangle.colors[vertex_index].unwrap_or([0.0, 0.0, 0.0, -1.0]),
                });
                shared_normals
                    .entry(position_key(position))
                    .or_default()
                    .push(VertexNormalSample {
                        vertex_index: output_index,
                        face_normal,
                        weighted_normal,
                    });
            }
            let base = (triangle_index * 3) as u32;
            indices.extend_from_slice(&[base, base + 1, base + 2]);
        }
        apply_smoothed_normals(&mut vertices, &shared_normals);
        Ok(Self {
            vertices,
            indices,
            bounds,
        })
    }

    pub fn from_indexed_buffers(
        positions: Vec<[f32; 3]>,
        normals: Vec<[f32; 3]>,
        vertex_colors: Vec<[f32; 4]>,
        indices: Vec<u32>,
    ) -> Result<Self, MeshError> {
        if positions.is_empty() {
            return Err(MeshError("网格中没有可渲染的顶点".into()));
        }
        if indices.is_empty() {
            return Err(MeshError("网格中没有可渲染的索引".into()));
        }
        if indices.len() % 3 != 0 {
            return Err(MeshError("网格索引数量不是 3 的倍数".into()));
        }
        if indices
            .iter()
            .any(|index| *index as usize >= positions.len())
        {
            return Err(MeshError("网格索引引用了不存在的顶点".into()));
        }

        let mut bounds = Bounds::empty();
        let vertices = positions
            .into_iter()
            .enumerate()
            .map(|(index, position)| {
                bounds.include(Vec3::from_array(position));
                Vertex {
                    position,
                    normal: normals.get(index).copied().unwrap_or(DEFAULT_NORMAL),
                    color: vertex_colors
                        .get(index)
                        .copied()
                        .unwrap_or([0.0, 0.0, 0.0, -1.0]),
                }
            })
            .collect();

        Ok(Self {
            vertices,
            indices,
            bounds,
        })
    }

    pub fn triangle_index_partitions(&self) -> (Vec<u32>, Vec<u32>) {
        let mut opaque = Vec::with_capacity(self.indices.len());
        let mut transparent = Vec::new();
        for triangle in self.indices.chunks_exact(3) {
            if self.triangle_is_transparent(triangle) {
                transparent.extend_from_slice(triangle);
            } else {
                opaque.extend_from_slice(triangle);
            }
        }
        (opaque, transparent)
    }

    pub fn sorted_transparent_triangle_indices(&self, eye_position: [f32; 3]) -> Vec<u32> {
        let eye = Vec3::from_array(eye_position);
        let mut triangles = self
            .indices
            .chunks_exact(3)
            .filter(|triangle| self.triangle_is_transparent(triangle))
            .map(|triangle| {
                let centroid = triangle_centroid(&self.vertices, triangle);
                let distance_sq = centroid.distance_squared(eye);
                ([triangle[0], triangle[1], triangle[2]], distance_sq)
            })
            .collect::<Vec<_>>();
        triangles.sort_by(|left, right| right.1.total_cmp(&left.1));
        triangles
            .into_iter()
            .flat_map(|(triangle, _)| triangle)
            .collect()
    }

    fn triangle_is_transparent(&self, triangle: &[u32]) -> bool {
        triangle.iter().any(|index| {
            let alpha = self.vertices[*index as usize].color[3];
            alpha > 0.0 && alpha < SOLID_TRANSPARENCY_ALPHA_THRESHOLD
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
pub fn load_stl(path: &Path) -> Result<MeshData, MeshError> {
    let file = File::open(path).map_err(|error| MeshError(format!("打开 STL 失败: {error}")))?;
    let mut reader = BufReader::new(file);
    load_stl_from_reader(&mut reader)
}

#[allow(dead_code)]
pub fn load_stl_from_reader<R>(reader: &mut R) -> Result<MeshData, MeshError>
where
    R: Read + Seek,
{
    let mesh =
        stl_io::read_stl(reader).map_err(|error| MeshError(format!("解析 STL 失败: {error}")))?;
    let triangles = mesh
        .faces
        .iter()
        .map(|face| MeshTriangle {
            positions: [
                mesh.vertices[face.vertices[0]].into(),
                mesh.vertices[face.vertices[1]].into(),
                mesh.vertices[face.vertices[2]].into(),
            ],
            normal: face.normal.into(),
            colors: [None; 3],
        })
        .collect::<Vec<_>>();
    MeshData::from_triangles(&triangles)
}

fn apply_smoothed_normals(
    vertices: &mut [Vertex],
    shared_normals: &HashMap<[u32; 3], Vec<VertexNormalSample>>,
) {
    for samples in shared_normals.values() {
        for sample in samples {
            vertices[sample.vertex_index].normal = smoothed_normal(*sample, samples);
        }
    }
}

fn smoothed_normal(sample: VertexNormalSample, samples: &[VertexNormalSample]) -> [f32; 3] {
    let blended = samples
        .iter()
        .filter(|candidate| {
            sample.face_normal.dot(candidate.face_normal) >= NORMAL_SMOOTHING_DOT_THRESHOLD
        })
        .fold(Vec3::ZERO, |sum, candidate| sum + candidate.weighted_normal);
    if blended == Vec3::ZERO {
        sample.face_normal.to_array()
    } else {
        blended.normalize().to_array()
    }
}

fn normalized_normal(normal: [f32; 3]) -> Vec3 {
    let normal = Vec3::from_array(normal).normalize_or_zero();
    if normal == Vec3::ZERO {
        Vec3::Z
    } else {
        normal
    }
}

fn triangle_weighted_normal(triangle: &MeshTriangle, fallback: Vec3) -> Vec3 {
    let a = Vec3::from_array(triangle.positions[0]);
    let b = Vec3::from_array(triangle.positions[1]);
    let c = Vec3::from_array(triangle.positions[2]);
    let weighted = (b - a).cross(c - a);
    if weighted == Vec3::ZERO {
        fallback
    } else {
        weighted
    }
}

fn position_key(position: [f32; 3]) -> [u32; 3] {
    position.map(|value| if value == 0.0 { 0.0 } else { value }.to_bits())
}

fn triangle_centroid(vertices: &[Vertex], triangle: &[u32]) -> Vec3 {
    triangle.iter().fold(Vec3::ZERO, |sum, index| {
        sum + Vec3::from_array(vertices[*index as usize].position)
    }) / 3.0
}

impl std::error::Error for MeshError {}

impl fmt::Display for MeshError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
