use app_server_protocol::{
    ClientCommand, ClientEnvelope, ClientRequestEnvelope, RequestId, decode_client_frame,
    encode_client_frame,
};

#[test]
fn workspace_open_variant_does_not_exist() {
    let source = include_str!("../src/protocol.rs");
    assert!(!source.contains("WorkspaceOpen"));
    assert!(!source.contains("workspace.open"));
}

#[test]
fn workspace_current_frame_roundtrip_still_works() {
    let envelope = ClientRequestEnvelope {
        request_id: RequestId(1),
        command: ClientCommand::WorkspaceCurrent,
    };
    let frame = ClientEnvelope::Request(envelope.clone());
    let decoded = decode_client_frame(&encode_client_frame(&frame).unwrap()).unwrap();
    assert_eq!(decoded, ClientEnvelope::Request(envelope));
}
