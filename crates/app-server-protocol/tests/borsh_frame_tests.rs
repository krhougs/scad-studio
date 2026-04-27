use app_server_protocol::{
    CapabilityHandshakeRequest, ClientCapabilities, ClientEnvelope, ClientPlatform, CommandSuccess,
    FileReadCapability, ProtocolError, ProtocolErrorCode, ProtocolVersionRange, RequestId,
    ServerEnvelope, ServerPushEnvelope, ServerPushEvent, ServerResponseEnvelope,
    TransportErrorFrame, WatchErrorEvent, decode_client_frame, decode_server_frame,
    encode_client_frame, encode_server_frame,
};

fn handshake() -> ClientEnvelope {
    ClientEnvelope::Handshake(CapabilityHandshakeRequest {
        capabilities: ClientCapabilities {
            client_name: "web".into(),
            platform: ClientPlatform::Web,
            protocol_version: ProtocolVersionRange::new(2, 2),
            file_read: FileReadCapability {
                denied_extensions: Vec::new(),
            },
            supported_preview_kinds: Vec::new(),
        },
    })
}

#[test]
fn client_frame_roundtrips_with_magic_and_wire_version() {
    let envelope = handshake();

    let encoded = encode_client_frame(&envelope).expect("client frame should encode");
    assert_eq!(&encoded[0..4], b"BDNP");
    assert_eq!(encoded[4], app_server_protocol::WIRE_VERSION);

    let decoded = decode_client_frame(&encoded).expect("client frame should decode");
    assert_eq!(decoded, envelope);
}

#[test]
fn server_frame_roundtrips_success_error_and_push() {
    let response = ServerEnvelope::Response(ServerResponseEnvelope {
        request_id: RequestId(7),
        result: Ok(CommandSuccess::ConfigSaved),
    });
    let decoded_response =
        decode_server_frame(&encode_server_frame(&response).unwrap()).expect("response decodes");
    assert_eq!(decoded_response, response);

    let error = ServerEnvelope::Response(ServerResponseEnvelope {
        request_id: RequestId(8),
        result: Err(ProtocolError::new(
            ProtocolErrorCode::InvalidCommand,
            "bad command",
        )),
    });
    let decoded_error =
        decode_server_frame(&encode_server_frame(&error).unwrap()).expect("error decodes");
    assert_eq!(decoded_error, error);

    let push = ServerEnvelope::Push(ServerPushEnvelope {
        event: ServerPushEvent::WatchError(WatchErrorEvent {
            subscription_id: app_server_protocol::SubscriptionId("sub-1".into()),
            message: "watch failed".into(),
        }),
    });
    let decoded_push = decode_server_frame(&encode_server_frame(&push).unwrap()).unwrap();
    assert_eq!(decoded_push, push);
}

#[test]
fn frame_decode_rejects_wrong_magic_and_wire_version() {
    let mut encoded = encode_client_frame(&handshake()).unwrap();
    encoded[0] = b'X';
    let error = decode_client_frame(&encoded).expect_err("wrong magic should fail");
    assert_eq!(error.code(), ProtocolErrorCode::InvalidWireFrame);

    let mut encoded = encode_client_frame(&handshake()).unwrap();
    encoded[4] = 255;
    let error = decode_client_frame(&encoded).expect_err("unsupported version should fail");
    assert_eq!(error.code(), ProtocolErrorCode::UnsupportedWireVersion);
}

#[test]
fn transport_error_frame_roundtrips() {
    let envelope = ServerEnvelope::TransportError(TransportErrorFrame {
        message: "decode failed".into(),
    });

    let decoded = decode_server_frame(&encode_server_frame(&envelope).unwrap()).unwrap();
    assert_eq!(decoded, envelope);
}

#[test]
fn golden_client_close_frame_locks_magic_version_and_enum_discriminant() {
    let encoded = encode_client_frame(&ClientEnvelope::Close).unwrap();
    assert_eq!(encoded, vec![b'B', b'D', b'N', b'P', 2, 3]);
}

#[test]
fn golden_request_frame_locks_core_command_discriminant() {
    let envelope = ClientEnvelope::Request(app_server_protocol::ClientRequestEnvelope {
        request_id: RequestId(42),
        command: app_server_protocol::ClientCommand::WorkspaceCurrent,
    });

    let encoded = encode_client_frame(&envelope).unwrap();

    assert_eq!(
        encoded,
        vec![b'B', b'D', b'N', b'P', 2, 2, 42, 0, 0, 0, 0, 0, 0, 0, 0]
    );
}

#[test]
fn golden_server_success_frame_locks_response_and_success_discriminants() {
    let envelope = ServerEnvelope::Response(ServerResponseEnvelope {
        request_id: RequestId(7),
        result: Ok(CommandSuccess::ConfigSaved),
    });

    let encoded = encode_server_frame(&envelope).unwrap();

    assert_eq!(
        encoded,
        vec![b'B', b'D', b'N', b'P', 2, 1, 7, 0, 0, 0, 0, 0, 0, 0, 1, 3]
    );
}
