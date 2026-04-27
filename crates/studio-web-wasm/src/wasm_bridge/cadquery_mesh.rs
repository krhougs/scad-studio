use app_server_protocol::{CadQueryMeshPayload, CadQueryObjectKind};
use serde::Serialize;
use wasm_bindgen::JsValue;
use wasm_bindgen::prelude::wasm_bindgen;

#[wasm_bindgen]
pub struct CadQueryMeshHandle {
    pub(crate) payload: CadQueryMeshPayload,
}

#[wasm_bindgen]
impl CadQueryMeshHandle {
    #[wasm_bindgen(getter)]
    pub fn result_id(&self) -> String {
        self.payload.result_id.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn build_id(&self) -> String {
        self.payload.build_id.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn root_ref_text(&self) -> String {
        self.payload.root_ref_text.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn root_object_kind(&self) -> String {
        object_kind_label(self.payload.root_object_kind).into()
    }

    #[wasm_bindgen(getter)]
    pub fn part_count(&self) -> u32 {
        self.payload.parts.len() as u32
    }

    pub fn metadata(&self) -> Result<JsValue, JsValue> {
        let parts = self.payload.parts.iter().map(PartMetadata::from).collect();
        let metadata = MeshMetadata {
            result_id: &self.payload.result_id,
            build_id: &self.payload.build_id,
            root_ref_text: &self.payload.root_ref_text,
            root_object_kind: object_kind_label(self.payload.root_object_kind),
            parts,
        };
        serde_wasm_bindgen::to_value(&metadata)
            .map_err(|error| JsValue::from_str(&format!("cadquery metadata serialize: {error}")))
    }

    pub fn face_positions(&self, part_index: u32, face_index: u32) -> Result<Vec<f32>, JsValue> {
        Ok(self.face(part_index, face_index)?.positions.clone())
    }

    pub fn face_normals(&self, part_index: u32, face_index: u32) -> Result<Vec<f32>, JsValue> {
        Ok(self.face(part_index, face_index)?.normals.clone())
    }

    pub fn edge_polyline(&self, part_index: u32, edge_index: u32) -> Result<Vec<f32>, JsValue> {
        let part = self.part(part_index)?;
        part.edges
            .get(edge_index as usize)
            .map(|edge| edge.polyline.clone())
            .ok_or_else(|| JsValue::from_str("cadquery edge index out of range"))
    }

    pub fn vertex_position(&self, part_index: u32, vertex_index: u32) -> Result<Vec<f32>, JsValue> {
        let part = self.part(part_index)?;
        part.vertices
            .get(vertex_index as usize)
            .map(|vertex| vertex.position.to_vec())
            .ok_or_else(|| JsValue::from_str("cadquery vertex index out of range"))
    }

    fn part(&self, part_index: u32) -> Result<&app_server_protocol::CadQueryPartMesh, JsValue> {
        self.payload
            .parts
            .get(part_index as usize)
            .ok_or_else(|| JsValue::from_str("cadquery part index out of range"))
    }

    fn face(
        &self,
        part_index: u32,
        face_index: u32,
    ) -> Result<&app_server_protocol::FaceGroup, JsValue> {
        self.part(part_index)?
            .faces
            .get(face_index as usize)
            .ok_or_else(|| JsValue::from_str("cadquery face index out of range"))
    }
}

#[derive(Serialize)]
struct MeshMetadata<'a> {
    result_id: &'a str,
    build_id: &'a str,
    root_ref_text: &'a str,
    root_object_kind: &'static str,
    parts: Vec<PartMetadata<'a>>,
}

#[derive(Serialize)]
struct PartMetadata<'a> {
    name: &'a str,
    object_kind: &'static str,
    ref_text: &'a str,
    instance_path: &'a Option<String>,
    transform: &'a Option<[f32; 16]>,
    faces: Vec<FaceMetadata<'a>>,
    edges: Vec<EdgeMetadata<'a>>,
    vertices: Vec<VertexMetadata<'a>>,
    feature_map: Vec<FeatureMetadata<'a>>,
}

#[derive(Serialize)]
struct FaceMetadata<'a> {
    face_idx: u32,
    features: &'a [String],
    ambiguous: bool,
}

#[derive(Serialize)]
struct EdgeMetadata<'a> {
    edge_idx: u32,
    adjacent_faces: &'a [u32],
}

#[derive(Serialize)]
struct VertexMetadata<'a> {
    vertex_idx: u32,
    adjacent_edges: &'a [u32],
}

#[derive(Serialize)]
struct FeatureMetadata<'a> {
    feature: &'a str,
    face_indices: &'a [u32],
}

impl<'a> From<&'a app_server_protocol::CadQueryPartMesh> for PartMetadata<'a> {
    fn from(part: &'a app_server_protocol::CadQueryPartMesh) -> Self {
        Self {
            name: &part.name,
            object_kind: object_kind_label(part.object_kind),
            ref_text: &part.ref_text,
            instance_path: &part.instance_path,
            transform: &part.transform,
            faces: part.faces.iter().map(FaceMetadata::from).collect(),
            edges: part.edges.iter().map(EdgeMetadata::from).collect(),
            vertices: part.vertices.iter().map(VertexMetadata::from).collect(),
            feature_map: part.feature_map.iter().map(FeatureMetadata::from).collect(),
        }
    }
}

impl<'a> From<&'a app_server_protocol::FaceGroup> for FaceMetadata<'a> {
    fn from(face: &'a app_server_protocol::FaceGroup) -> Self {
        Self {
            face_idx: face.face_idx,
            features: &face.features,
            ambiguous: face.ambiguous,
        }
    }
}

impl<'a> From<&'a app_server_protocol::EdgeGroup> for EdgeMetadata<'a> {
    fn from(edge: &'a app_server_protocol::EdgeGroup) -> Self {
        Self {
            edge_idx: edge.edge_idx,
            adjacent_faces: &edge.adjacent_faces,
        }
    }
}

impl<'a> From<&'a app_server_protocol::VertexPoint> for VertexMetadata<'a> {
    fn from(vertex: &'a app_server_protocol::VertexPoint) -> Self {
        Self {
            vertex_idx: vertex.vertex_idx,
            adjacent_edges: &vertex.adjacent_edges,
        }
    }
}

impl<'a> From<&'a app_server_protocol::CadQueryFeatureFaces> for FeatureMetadata<'a> {
    fn from(feature: &'a app_server_protocol::CadQueryFeatureFaces) -> Self {
        Self {
            feature: &feature.feature,
            face_indices: &feature.face_indices,
        }
    }
}

fn object_kind_label(kind: CadQueryObjectKind) -> &'static str {
    match kind {
        CadQueryObjectKind::Part => "part",
        CadQueryObjectKind::Component => "component",
        CadQueryObjectKind::Assembly => "assembly",
    }
}
