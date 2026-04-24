use app_server_protocol::{
    PreviewArtifact, PreviewMeshPayload, PreviewReadyResponse, PreviewRequestKind, PreviewUnit,
    ProtocolErrorCode, ServerEnvelope, decode_server_frame, encode_server_frame,
};

fn mesh_with_position(value: f32) -> ServerEnvelope {
    ServerEnvelope::Response(app_server_protocol::ServerResponseEnvelope {
        request_id: app_server_protocol::RequestId(1),
        result: Ok(app_server_protocol::CommandSuccess::PreviewReady(
            PreviewReadyResponse {
                requested_kind: PreviewRequestKind::GeometryArtifact,
                artifact: PreviewArtifact::Mesh(PreviewMeshPayload {
                    unit: PreviewUnit::Millimeter,
                    positions: vec![[value, 0.0, 0.0]],
                    normals: vec![[0.0, 1.0, 0.0]],
                    vertex_colors: vec![[1.0, 1.0, 1.0, 1.0]],
                    indices: vec![0],
                }),
            },
        )),
    })
}

#[test]
fn preview_mesh_finite_values_encode_and_decode() {
    let envelope = mesh_with_position(1.0);
    let decoded = decode_server_frame(&encode_server_frame(&envelope).unwrap()).unwrap();
    assert_eq!(decoded, envelope);
}

#[test]
fn preview_mesh_rejects_nan_and_infinity_before_encoding() {
    let error = encode_server_frame(&mesh_with_position(f32::NAN))
        .expect_err("NaN should not enter wire payload");
    assert_eq!(error.code(), ProtocolErrorCode::InvalidNumericValue);

    let error = encode_server_frame(&mesh_with_position(f32::INFINITY))
        .expect_err("Inf should not enter wire payload");
    assert_eq!(error.code(), ProtocolErrorCode::InvalidNumericValue);
}
