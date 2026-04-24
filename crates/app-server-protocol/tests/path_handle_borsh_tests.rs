use app_server_protocol::{
    ClientCommand, ClientEnvelope, ClientRequestEnvelope, PathHandle, ProtocolErrorCode, RequestId,
    WorkspaceId, WorkspaceListRequest, decode_client_frame, encode_client_frame,
};

#[test]
fn path_handle_borsh_roundtrips_root_and_nested_path() {
    let root = PathHandle::new(WorkspaceId::new("ws"), Vec::<String>::new()).unwrap();
    let nested = PathHandle::new(WorkspaceId::new("ws"), ["src", "main.scad"]).unwrap();

    for path in [root, nested] {
        let envelope = ClientEnvelope::Request(ClientRequestEnvelope {
            request_id: RequestId(1),
            command: ClientCommand::WorkspaceList(WorkspaceListRequest {
                directory: Some(path.clone()),
            }),
        });
        let decoded = decode_client_frame(&encode_client_frame(&envelope).unwrap()).unwrap();
        assert_eq!(decoded, envelope);
    }
}

#[test]
fn path_handle_borsh_decode_rejects_invalid_segment() {
    let valid = ClientEnvelope::Request(ClientRequestEnvelope {
        request_id: RequestId(1),
        command: ClientCommand::WorkspaceList(WorkspaceListRequest {
            directory: Some(PathHandle::new(WorkspaceId::new("ws"), ["src"]).unwrap()),
        }),
    });
    let mut encoded = encode_client_frame(&valid).unwrap();

    let needle = b"src";
    let start = encoded
        .windows(needle.len())
        .position(|window| window == needle)
        .expect("encoded path should contain test segment");
    encoded[start..start + 3].copy_from_slice(b"../");

    let error = decode_client_frame(&encoded).expect_err("invalid path should fail on decode");
    assert_eq!(error.code(), ProtocolErrorCode::InvalidPathHandle);
}
