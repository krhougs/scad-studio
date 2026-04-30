use std::collections::BTreeMap;

use app_server_protocol::{
    CadQueryArtifactExport, CadQueryArtifactRelation, CadQueryFeatureFaces, CadQueryMeshPayload,
    CadQueryObjectKind, CadQueryPartMesh, CadQueryResultReady, EdgeGroup, FaceGroup, PathHandle,
    PreviewUnit, ProtocolError, ProtocolErrorCode, VertexPoint, WorkspaceId, validate_f32,
};
use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

#[derive(Debug, Deserialize)]
struct RunnerSuccess {
    status: String,
    result_id: String,
    build_id: String,
    unit: String,
    root_ref_text: String,
    root_object_kind: String,
    parts: Vec<RunnerPart>,
    exports: BTreeMap<String, String>,
    manifest: RunnerManifest,
}

#[derive(Debug, Deserialize)]
struct RunnerManifest {
    source_path: String,
    source_hash: String,
    params: Value,
    params_hash: String,
    dependencies: Vec<RunnerDependency>,
    deps_hash: String,
    export_hashes: BTreeMap<String, String>,
}

#[derive(Debug, Deserialize)]
struct RunnerDependency {
    path: String,
    hash: String,
}

#[derive(Debug, Deserialize)]
struct RunnerPart {
    name: String,
    object_kind: String,
    ref_text: String,
    instance_path: Option<String>,
    transform: Option<Vec<f32>>,
    mesh: RunnerMesh,
    #[serde(default)]
    feature_map: BTreeMap<String, RunnerFeatureMap>,
}

#[derive(Debug, Deserialize)]
struct RunnerMesh {
    faces: Vec<RunnerFace>,
    #[serde(default)]
    edges: Vec<EdgeGroup>,
    #[serde(default)]
    vertices: Vec<VertexPoint>,
}

#[derive(Debug, Deserialize)]
struct RunnerFace {
    face_idx: u32,
    positions: Vec<f32>,
    normals: Vec<f32>,
    #[serde(default)]
    features: Vec<String>,
    #[serde(default)]
    ambiguous: bool,
}

#[derive(Debug, Deserialize)]
struct RunnerFeatureMap {
    #[serde(default)]
    face_indices: Vec<u32>,
}

pub fn parse_cadquery_success_json(text: &str) -> Result<CadQueryMeshPayload, ProtocolError> {
    let decoded: RunnerSuccess = serde_json::from_str(text).map_err(|error| {
        ProtocolError::new(
            ProtocolErrorCode::InvalidWireFrame,
            format!("CadQuery runner JSON 解析失败: {error}"),
        )
    })?;
    if decoded.status != "success" {
        return Err(ProtocolError::new(
            ProtocolErrorCode::InvalidWireFrame,
            "CadQuery runner JSON 不是 success payload",
        ));
    }
    if decoded.unit != "millimeter" {
        return Err(ProtocolError::new(
            ProtocolErrorCode::InvalidWireFrame,
            "CadQuery runner unit 必须是 millimeter",
        ));
    }
    validate_manifest(&decoded)?;
    let artifact_relation = convert_artifact_relation(&decoded)?;
    let mut parts = Vec::with_capacity(decoded.parts.len());
    for part in decoded.parts {
        parts.push(convert_part(part)?);
    }
    let payload = CadQueryMeshPayload {
        result_id: decoded.result_id,
        build_id: decoded.build_id,
        unit: PreviewUnit::Millimeter,
        root_ref_text: decoded.root_ref_text,
        root_object_kind: parse_object_kind(&decoded.root_object_kind)?,
        artifact_relation: Some(artifact_relation),
        parts,
    };
    validate_cadquery_mesh_payload(&payload)?;
    Ok(payload)
}

fn validate_manifest(decoded: &RunnerSuccess) -> Result<(), ProtocolError> {
    let manifest = &decoded.manifest;
    validate_manifest_path(&manifest.source_path, "source_path")?;
    validate_hash(&manifest.source_hash, "source_hash")?;
    validate_hash(&manifest.params_hash, "params_hash")?;
    validate_hash(&manifest.deps_hash, "deps_hash")?;
    validate_dependencies(manifest)?;
    validate_exports(&decoded.exports, &manifest.export_hashes)?;
    let params_hash = sha256_json_value(&manifest.params)?;
    if params_hash != manifest.params_hash {
        return invalid("CadQuery manifest params_hash 与 params 不匹配");
    }
    let deps_hash = sha256_dependencies(&manifest.dependencies)?;
    if deps_hash != manifest.deps_hash {
        return invalid("CadQuery manifest deps_hash 与 dependencies 不匹配");
    }
    let build_id = sha256_build_id(
        &manifest.source_hash,
        &manifest.params_hash,
        &manifest.deps_hash,
    )?;
    if build_id != decoded.build_id {
        return invalid("CadQuery build_id 与 manifest hash 不匹配");
    }
    Ok(())
}

fn validate_dependencies(manifest: &RunnerManifest) -> Result<(), ProtocolError> {
    let mut found_source = false;
    for dependency in &manifest.dependencies {
        validate_manifest_path(&dependency.path, "dependency path")?;
        validate_hash(&dependency.hash, "dependency hash")?;
        if dependency.path == manifest.source_path {
            found_source = true;
            if dependency.hash != manifest.source_hash {
                return invalid("CadQuery manifest source_hash 与 dependencies 不匹配");
            }
        }
    }
    if found_source {
        Ok(())
    } else {
        invalid("CadQuery manifest dependencies 必须包含 source_path")
    }
}

fn validate_exports(
    exports: &BTreeMap<String, String>,
    export_hashes: &BTreeMap<String, String>,
) -> Result<(), ProtocolError> {
    for (name, path) in exports {
        if name.is_empty() {
            return invalid("CadQuery export 名称不能为空");
        }
        validate_manifest_path(path, "export path")?;
        let Some(hash) = export_hashes.get(name) else {
            return invalid("CadQuery export_hashes 必须包含每个 export");
        };
        validate_hash(hash, "export hash")?;
    }
    for name in export_hashes.keys() {
        if !exports.contains_key(name) {
            return invalid("CadQuery export_hashes 不能包含未知 export");
        }
    }
    Ok(())
}

fn validate_manifest_path(path: &str, field: &str) -> Result<(), ProtocolError> {
    if path.is_empty() {
        return invalid(format!("CadQuery manifest {field} 路径不能为空"));
    }
    PathHandle::new(WorkspaceId("cadquery-manifest".into()), path.split('/')).map_err(|error| {
        ProtocolError::new(
            ProtocolErrorCode::InvalidWireFrame,
            format!("CadQuery manifest {field} 路径不是有效 workspace 相对路径: {error}"),
        )
    })?;
    Ok(())
}

fn convert_artifact_relation(
    decoded: &RunnerSuccess,
) -> Result<CadQueryArtifactRelation, ProtocolError> {
    let mut exports = Vec::with_capacity(decoded.exports.len());
    for (name, path) in &decoded.exports {
        let Some(hash) = decoded.manifest.export_hashes.get(name) else {
            return invalid("CadQuery export_hashes 必须包含每个 export");
        };
        exports.push(CadQueryArtifactExport {
            name: name.clone(),
            path: path.clone(),
            hash: hash.clone(),
        });
    }
    Ok(CadQueryArtifactRelation {
        source_path: decoded.manifest.source_path.clone(),
        exports,
    })
}

pub fn cadquery_result_ready(payload: &CadQueryMeshPayload) -> CadQueryResultReady {
    let part_count = payload.parts.len() as u32;
    let face_count = payload
        .parts
        .iter()
        .map(|part| part.faces.len() as u32)
        .sum();
    let edge_count = payload
        .parts
        .iter()
        .map(|part| part.edges.len() as u32)
        .sum();
    let vertex_count = payload
        .parts
        .iter()
        .map(|part| part.vertices.len() as u32)
        .sum();
    CadQueryResultReady {
        result_id: payload.result_id.clone(),
        build_id: payload.build_id.clone(),
        part_count,
        face_count,
        edge_count,
        vertex_count,
        artifact_relation: payload.artifact_relation.clone(),
    }
}

pub fn validate_cadquery_mesh_payload(payload: &CadQueryMeshPayload) -> Result<(), ProtocolError> {
    for part in &payload.parts {
        validate_transform(part.transform)?;
        for face in &part.faces {
            validate_index(face.face_idx, part.faces.len(), "CadQuery face index")?;
            validate_flat_xyz(&face.positions, "cadquery face positions")?;
            validate_flat_xyz(&face.normals, "cadquery face normals")?;
            if face.positions.len() != face.normals.len() {
                return invalid("CadQuery face positions/normals 长度必须一致");
            }
        }
        for edge in &part.edges {
            validate_index(edge.edge_idx, part.edges.len(), "CadQuery edge index")?;
            validate_flat_xyz(&edge.polyline, "cadquery edge polyline")?;
            for face_idx in &edge.adjacent_faces {
                validate_face_index(*face_idx, part.faces.len())?;
            }
        }
        for vertex in &part.vertices {
            validate_index(
                vertex.vertex_idx,
                part.vertices.len(),
                "CadQuery vertex index",
            )?;
            for value in vertex.position {
                validate_f32(value, "cadquery vertex position")?;
            }
            for edge_idx in &vertex.adjacent_edges {
                if (*edge_idx as usize) >= part.edges.len() {
                    return invalid("CadQuery vertex adjacent edge index 越界");
                }
            }
        }
        for feature in &part.feature_map {
            for face_idx in &feature.face_indices {
                validate_face_index(*face_idx, part.faces.len())?;
            }
        }
    }
    Ok(())
}

fn convert_part(part: RunnerPart) -> Result<CadQueryPartMesh, ProtocolError> {
    let mut feature_map = Vec::with_capacity(part.feature_map.len());
    for (feature, value) in part.feature_map {
        feature_map.push(CadQueryFeatureFaces {
            feature,
            face_indices: value.face_indices,
        });
    }
    Ok(CadQueryPartMesh {
        name: part.name,
        object_kind: parse_object_kind(&part.object_kind)?,
        ref_text: part.ref_text,
        instance_path: part.instance_path,
        transform: parse_transform(part.transform)?,
        faces: part.mesh.faces.into_iter().map(convert_face).collect(),
        edges: part.mesh.edges,
        vertices: part.mesh.vertices,
        feature_map,
    })
}

fn convert_face(face: RunnerFace) -> FaceGroup {
    FaceGroup {
        face_idx: face.face_idx,
        positions: face.positions,
        normals: face.normals,
        features: face.features,
        ambiguous: face.ambiguous,
    }
}

fn parse_object_kind(value: &str) -> Result<CadQueryObjectKind, ProtocolError> {
    match value {
        "part" => Ok(CadQueryObjectKind::Part),
        "component" => Ok(CadQueryObjectKind::Component),
        "assembly" => Ok(CadQueryObjectKind::Assembly),
        _ => Err(ProtocolError::new(
            ProtocolErrorCode::InvalidWireFrame,
            format!("未知 CadQuery object_kind: {value}"),
        )),
    }
}

fn parse_transform(value: Option<Vec<f32>>) -> Result<Option<[f32; 16]>, ProtocolError> {
    let Some(value) = value else {
        return Ok(None);
    };
    let array: [f32; 16] = value.try_into().map_err(|_| {
        ProtocolError::new(
            ProtocolErrorCode::InvalidWireFrame,
            "transform 必须有 16 个数",
        )
    })?;
    validate_transform(Some(array))?;
    Ok(Some(array))
}

fn validate_transform(value: Option<[f32; 16]>) -> Result<(), ProtocolError> {
    if let Some(transform) = value {
        for value in transform {
            validate_f32(value, "cadquery transform")?;
        }
    }
    Ok(())
}

fn validate_flat_xyz(values: &[f32], field: &str) -> Result<(), ProtocolError> {
    if values.len() % 3 != 0 {
        return invalid(format!("{field} 长度必须是 3 的倍数"));
    }
    for value in values {
        validate_f32(*value, field)?;
    }
    Ok(())
}

fn validate_face_index(index: u32, face_count: usize) -> Result<(), ProtocolError> {
    validate_index(index, face_count, "CadQuery face index")
}

fn validate_index(index: u32, len: usize, field: &str) -> Result<(), ProtocolError> {
    if (index as usize) < len {
        return Ok(());
    }
    invalid(format!("{field} 越界"))
}

fn invalid<T>(message: impl Into<String>) -> Result<T, ProtocolError> {
    Err(ProtocolError::new(
        ProtocolErrorCode::InvalidWireFrame,
        message,
    ))
}

fn validate_hash(value: &str, field: &str) -> Result<(), ProtocolError> {
    let digest = value.strip_prefix("sha256:").ok_or_else(|| {
        ProtocolError::new(
            ProtocolErrorCode::InvalidWireFrame,
            format!("{field} 必须使用 sha256: 前缀"),
        )
    })?;
    if digest.len() == 64 && digest.chars().all(|value| value.is_ascii_hexdigit()) {
        Ok(())
    } else {
        invalid(format!("{field} 必须是 sha256 hex digest"))
    }
}

fn sha256_build_id(
    source_hash: &str,
    params_hash: &str,
    deps_hash: &str,
) -> Result<String, ProtocolError> {
    let value = format!(
        "{{\"deps_hash\":{},\"params_hash\":{},\"source_hash\":{}}}",
        json_string(deps_hash)?,
        json_string(params_hash)?,
        json_string(source_hash)?,
    );
    Ok(sha256_bytes(value.as_bytes()))
}

fn sha256_dependencies(dependencies: &[RunnerDependency]) -> Result<String, ProtocolError> {
    let mut items = Vec::with_capacity(dependencies.len());
    for dependency in dependencies {
        items.push(format!(
            "{{\"hash\":{},\"path\":{}}}",
            json_string(&dependency.hash)?,
            json_string(&dependency.path)?,
        ));
    }
    Ok(sha256_bytes(format!("[{}]", items.join(",")).as_bytes()))
}

fn sha256_json_value(value: &Value) -> Result<String, ProtocolError> {
    Ok(sha256_bytes(canonical_json(value)?.as_bytes()))
}

fn sha256_bytes(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn canonical_json(value: &Value) -> Result<String, ProtocolError> {
    match value {
        Value::Null => Ok("null".into()),
        Value::Bool(value) => Ok(value.to_string()),
        Value::Number(value) => Ok(value.to_string()),
        Value::String(value) => json_string(value),
        Value::Array(values) => {
            let mut items = Vec::with_capacity(values.len());
            for value in values {
                items.push(canonical_json(value)?);
            }
            Ok(format!("[{}]", items.join(",")))
        }
        Value::Object(values) => {
            let mut items = Vec::with_capacity(values.len());
            for (key, value) in values {
                items.push(format!("{}:{}", json_string(key)?, canonical_json(value)?));
            }
            Ok(format!("{{{}}}", items.join(",")))
        }
    }
}

fn json_string(value: &str) -> Result<String, ProtocolError> {
    serde_json::to_string(value).map_err(|error| {
        ProtocolError::new(
            ProtocolErrorCode::InvalidWireFrame,
            format!("CadQuery manifest JSON 字符串编码失败: {error}"),
        )
    })
}
