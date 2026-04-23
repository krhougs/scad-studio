use app_server_protocol::{ClientCommand, ClientRequestEnvelope, RequestId};

#[test]
fn workspace_open_variant_does_not_exist() {
    let source = include_str!("../src/protocol.rs");
    assert!(!source.contains("WorkspaceOpen"));
    assert!(!source.contains("workspace.open"));
}

#[test]
fn workspace_open_serde_unknown_variant() {
    let json = r#"{
        "request_id": 1,
        "command": {
            "command": "workspace.open",
            "payload": {"path": "/tmp/ws"}
        }
    }"#;

    let error = serde_json::from_str::<ClientRequestEnvelope>(json)
        .expect_err("workspace.open must remain unsupported in protocol");
    let message = error.to_string();
    assert!(message.contains("workspace.open") || message.contains("unknown variant"));
}

#[test]
fn workspace_current_roundtrip_still_works() {
    let envelope = ClientRequestEnvelope {
        request_id: RequestId(1),
        command: ClientCommand::WorkspaceCurrent,
    };
    let json = serde_json::to_string(&envelope).unwrap();
    let decoded: ClientRequestEnvelope = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded, envelope);
}
