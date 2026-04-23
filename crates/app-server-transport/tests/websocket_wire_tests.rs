use app_server_protocol::{
    CapabilityHandshakeRequest, ClientCapabilities, ClientCommand, ClientPlatform,
    ClientRequestEnvelope, FileReadCapability, ProtocolVersionRange, RequestId,
    ServerResponseEnvelope,
};
use app_server_transport::{
    ClientEnvelope, ServerEnvelope, TransportErrorFrame, decode_client_envelope_text,
    decode_server_envelope_text, encode_client_envelope_text, encode_server_envelope_text,
};

#[test]
fn client_websocket_wire_roundtrips_handshake_and_request() {
    let handshake = ClientEnvelope::Handshake(CapabilityHandshakeRequest {
        capabilities: ClientCapabilities {
            client_name: "studio-web".into(),
            platform: ClientPlatform::Web,
            protocol_version: ProtocolVersionRange::new(1, 1),
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

    let handshake_text = encode_client_envelope_text(&handshake).expect("encode handshake");
    let request_text = encode_client_envelope_text(&request).expect("encode request");

    assert_eq!(
        decode_client_envelope_text(&handshake_text).expect("decode handshake"),
        handshake
    );
    assert_eq!(
        decode_client_envelope_text(&request_text).expect("decode request"),
        request
    );
}

#[test]
fn server_websocket_wire_roundtrips_transport_error_frame() {
    let envelope = ServerEnvelope::TransportError(TransportErrorFrame {
        message: "preview failed".into(),
    });
    let text = encode_server_envelope_text(&envelope).expect("encode error frame");

    assert_eq!(
        decode_server_envelope_text(&text).expect("decode error frame"),
        envelope
    );

    let response = ServerEnvelope::Response(ServerResponseEnvelope {
        request_id: RequestId(11),
        result: Err(app_server_protocol::ProtocolError::new(
            app_server_protocol::ProtocolErrorCode::Internal,
            "boom",
        )),
    });
    let response_text = encode_server_envelope_text(&response).expect("encode response");
    assert_eq!(
        decode_server_envelope_text(&response_text).expect("decode response"),
        response
    );
}
