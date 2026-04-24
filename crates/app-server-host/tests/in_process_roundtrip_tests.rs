use app_server_host::{ClientTransport, spawn_in_process_mpsc_host};
use app_server_protocol::{
    CapabilityHandshakeRequest, ClientCapabilities, ClientCommand, ClientPlatform,
    ClientRequestEnvelope, PreviewRequest, PreviewRequestKind, ProtocolVersionRange, RequestId,
    web_file_read_capability,
};
use app_server_transport::ServerEnvelope;

#[test]
fn spawned_in_process_host_roundtrips_workspace_file_and_preview() {
    let workspace = temp_workspace();
    let (mut host, mut transport) = spawn_in_process_mpsc_host().unwrap();
    host.rebind_workspace(workspace.clone());

    transport.handshake(handshake_request()).unwrap();
    let _ = recv_server_message(&mut transport).expect("handshake ack");

    transport
        .request(ClientRequestEnvelope {
            request_id: RequestId(1),
            command: ClientCommand::WorkspaceCurrent,
        })
        .unwrap();
    let current = recv_server_message(&mut transport).expect("workspace current response");
    let workspace_id = match current {
        ServerEnvelope::Response(envelope) => match envelope.result.unwrap() {
            app_server_protocol::CommandSuccess::WorkspaceCurrent(current) => current.workspace_id,
            other => panic!("unexpected workspace current response: {other:?}"),
        },
        other => panic!("unexpected server message: {other:?}"),
    };

    transport
        .request(ClientRequestEnvelope {
            request_id: RequestId(2),
            command: ClientCommand::WorkspaceList(app_server_protocol::WorkspaceListRequest {
                directory: None,
            }),
        })
        .unwrap();
    let list = recv_server_message(&mut transport).expect("workspace list response");
    let entries = match list {
        ServerEnvelope::Response(envelope) => match envelope.result.unwrap() {
            app_server_protocol::CommandSuccess::WorkspaceList(response) => response.entries,
            other => panic!("unexpected workspace list response: {other:?}"),
        },
        other => panic!("unexpected server message: {other:?}"),
    };
    let readme = entries
        .iter()
        .find(|entry| {
            entry
                .path
                .as_ref()
                .is_some_and(|path| path.display_path() == "README.md")
        })
        .expect("README entry should exist")
        .path
        .as_ref()
        .expect("README entry should be operable")
        .clone();
    let model = entries
        .iter()
        .find(|entry| {
            entry
                .path
                .as_ref()
                .is_some_and(|path| path.display_path() == "model.stl")
        })
        .expect("model entry should exist")
        .path
        .as_ref()
        .expect("model entry should be operable")
        .clone();
    assert_eq!(readme.workspace_id().0, workspace_id.0);

    transport
        .request(ClientRequestEnvelope {
            request_id: RequestId(3),
            command: ClientCommand::FileRead(app_server_protocol::FileReadRequest { path: readme }),
        })
        .unwrap();
    let file_read = recv_server_message(&mut transport).expect("file read response");
    match file_read {
        ServerEnvelope::Response(envelope) => match envelope.result.unwrap() {
            app_server_protocol::CommandSuccess::FileRead(response) => match response.contents {
                app_server_protocol::FileReadContents::Utf8Text(text) => {
                    assert!(text.contains("hello"))
                }
                other => panic!("unexpected file contents: {other:?}"),
            },
            other => panic!("unexpected file read response: {other:?}"),
        },
        other => panic!("unexpected server message: {other:?}"),
    }

    transport
        .request(ClientRequestEnvelope {
            request_id: RequestId(4),
            command: ClientCommand::PreviewRequest(PreviewRequest {
                source: model,
                defines: vec![],
                kind: PreviewRequestKind::GeometryArtifact,
                configured_openscad_path: None,
            }),
        })
        .unwrap();
    let preview = recv_server_message(&mut transport).expect("preview response");
    match preview {
        ServerEnvelope::Response(envelope) => match envelope.result.unwrap() {
            app_server_protocol::CommandSuccess::PreviewReady(response) => {
                match response.artifact {
                    app_server_protocol::PreviewArtifact::Mesh(mesh) => {
                        assert!(!mesh.positions.is_empty())
                    }
                    other => panic!("unexpected preview artifact: {other:?}"),
                }
            }
            other => panic!("unexpected preview response: {other:?}"),
        },
        other => panic!("unexpected server message: {other:?}"),
    }

    cleanup_workspace(&workspace);
}

fn handshake_request() -> CapabilityHandshakeRequest {
    CapabilityHandshakeRequest {
        capabilities: ClientCapabilities {
            client_name: "studio-app-tests".into(),
            platform: ClientPlatform::Desktop,
            protocol_version: ProtocolVersionRange::new(1, 1),
            file_read: web_file_read_capability(),
            supported_preview_kinds: vec![PreviewRequestKind::GeometryArtifact],
        },
    }
}

fn recv_server_message(transport: &mut dyn ClientTransport) -> Option<ServerEnvelope> {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    while std::time::Instant::now() < deadline {
        if let Some(message) = transport.next_server_message().unwrap() {
            return Some(message);
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    None
}

fn temp_workspace() -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!(
        "in-process-roundtrip-{}-{}",
        std::process::id(),
        std::thread::current().name().unwrap_or("test")
    ));
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("README.md"), "hello in-process").unwrap();

    let triangles = [stl_io::Triangle {
        normal: stl_io::Normal::new([0.0, 0.0, 1.0]),
        vertices: [
            stl_io::Vertex::new([0.0, 0.0, 0.0]),
            stl_io::Vertex::new([1.0, 0.0, 0.0]),
            stl_io::Vertex::new([0.0, 1.0, 0.0]),
        ],
    }];
    let mut bytes = Vec::new();
    stl_io::write_stl(&mut bytes, triangles.iter()).unwrap();
    std::fs::write(root.join("model.stl"), bytes).unwrap();
    root
}

fn cleanup_workspace(root: &std::path::Path) {
    let _ = std::fs::remove_file(root.join("README.md"));
    let _ = std::fs::remove_file(root.join("model.stl"));
    let _ = std::fs::remove_dir(root);
}
