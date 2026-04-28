use app_server_protocol::{
    CapabilityHandshakeRequest, ClientCapabilities, ClientCommand, ClientPlatform,
    ClientRequestEnvelope, FileReadCapability, ProtocolErrorCode, ProtocolVersionRange, RequestId,
    ServerResponseEnvelope, WIRE_MAGIC, WIRE_VERSION,
};
use app_server_transport::{
    ClientEnvelope, ServerEnvelope, TransportErrorFrame, decode_client_envelope_binary,
    decode_server_envelope_binary, encode_client_envelope_binary, encode_server_envelope_binary,
};

#[test]
fn client_websocket_wire_roundtrips_binary_handshake_and_request() {
    let handshake = ClientEnvelope::Handshake(CapabilityHandshakeRequest {
        capabilities: ClientCapabilities {
            client_name: "studio-web".into(),
            platform: ClientPlatform::Web,
            protocol_version: ProtocolVersionRange::new(3, 3),
            file_read: FileReadCapability {
                denied_extensions: vec![".scad".into(), ".stl".into()],
            },
            supported_preview_kinds: vec![
                app_server_protocol::PreviewRequestKind::GeometryArtifact,
            ],
        },
    });
    let request = ClientEnvelope::Request(ClientRequestEnvelope {
        request_id: RequestId(9),
        command: ClientCommand::WorkspaceCurrent,
    });

    let handshake_bytes = encode_client_envelope_binary(&handshake).expect("encode handshake");
    let request_bytes = encode_client_envelope_binary(&request).expect("encode request");

    assert_eq!(&handshake_bytes[0..4], WIRE_MAGIC);
    assert_eq!(handshake_bytes[4], WIRE_VERSION);

    assert_eq!(
        decode_client_envelope_binary(&handshake_bytes).expect("decode handshake"),
        handshake
    );
    assert_eq!(
        decode_client_envelope_binary(&request_bytes).expect("decode request"),
        request
    );
}

#[test]
fn server_websocket_wire_roundtrips_binary_transport_error_frame() {
    let envelope = ServerEnvelope::TransportError(TransportErrorFrame {
        message: "preview failed".into(),
    });
    let bytes = encode_server_envelope_binary(&envelope).expect("encode error frame");

    assert_eq!(
        decode_server_envelope_binary(&bytes).expect("decode error frame"),
        envelope
    );

    let response = ServerEnvelope::Response(ServerResponseEnvelope {
        request_id: RequestId(11),
        result: Err(app_server_protocol::ProtocolError::new(
            app_server_protocol::ProtocolErrorCode::Internal,
            "boom",
        )),
    });
    let response_bytes = encode_server_envelope_binary(&response).expect("encode response");
    assert_eq!(
        decode_server_envelope_binary(&response_bytes).expect("decode response"),
        response
    );
}

#[test]
fn client_websocket_wire_rejects_json_text_bytes() {
    let error = decode_client_envelope_binary(br#"{"kind":"close"}"#).unwrap_err();
    assert_eq!(error.code(), ProtocolErrorCode::InvalidWireFrame);
}

#[test]
fn client_websocket_wire_rejects_unsupported_wire_version() {
    let mut bytes = encode_client_envelope_binary(&ClientEnvelope::Close).expect("encode close");
    bytes[4] = WIRE_VERSION.saturating_add(1);
    let error = decode_client_envelope_binary(&bytes).unwrap_err();
    assert_eq!(error.code(), ProtocolErrorCode::UnsupportedWireVersion);
}
