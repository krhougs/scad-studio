use std::sync::{Arc, Mutex};

use app_server_host::HostRequestDispatcher;
use app_server_protocol::{
    CapabilityHandshakeRequest, ClientCapabilities, ClientCommand, ClientPlatform,
    ClientRequestEnvelope, CommandSuccess, PreviewArtifact, PreviewRequest, PreviewRequestKind,
    ProtocolVersionRange, RequestId, ServerPushEnvelope, SessionToken, WorkspaceListRequest,
    web_file_read_capability,
};

#[test]
fn shared_dispatcher_roundtrips_handshake_workspace_file_and_preview() {
    let workspace = temp_workspace("shared-dispatcher");
    let pushes = Arc::new(Mutex::new(Vec::<ServerPushEnvelope>::new()));
    let push_sink = {
        let pushes = Arc::clone(&pushes);
        Arc::new(move |push: ServerPushEnvelope| {
            pushes.lock().expect("push buffer lock").push(push);
        })
    };
    let mut dispatcher = HostRequestDispatcher::with_session_token(
        Some(workspace.clone()),
        SessionToken("session-1".into()),
        Vec::new(),
        push_sink,
    );

    let handshake = dispatcher.handshake(handshake_request());
    assert_eq!(handshake.session_token.0, "session-1");

    let current = dispatcher.dispatch_envelope(ClientRequestEnvelope {
        request_id: RequestId(1),
        command: ClientCommand::WorkspaceCurrent,
    });
    let workspace_id = match current.result.expect("workspace current should succeed") {
        CommandSuccess::WorkspaceCurrent(response) => {
            assert_eq!(
                response.root_name,
                workspace.file_name().unwrap().to_string_lossy()
            );
            response.workspace_id
        }
        other => panic!("unexpected workspace current response: {other:?}"),
    };

    let list = dispatcher.dispatch_envelope(ClientRequestEnvelope {
        request_id: RequestId(2),
        command: ClientCommand::WorkspaceList(WorkspaceListRequest { directory: None }),
    });
    let entries = match list.result.expect("workspace list should succeed") {
        CommandSuccess::WorkspaceList(response) => response.entries,
        other => panic!("unexpected workspace list response: {other:?}"),
    };
    let readme = entries
        .iter()
        .find(|entry| entry.path.display_path() == "README.md")
        .expect("README entry should exist")
        .path
        .clone();
    let model = entries
        .iter()
        .find(|entry| entry.path.display_path() == "model.stl")
        .expect("model entry should exist")
        .path
        .clone();
    assert_eq!(readme.workspace_id().0, workspace_id.0);

    let file_read = dispatcher.dispatch_envelope(ClientRequestEnvelope {
        request_id: RequestId(3),
        command: ClientCommand::FileRead(app_server_protocol::FileReadRequest { path: readme }),
    });
    match file_read.result.expect("file read should succeed") {
        CommandSuccess::FileRead(response) => match response.contents {
            app_server_protocol::FileReadContents::Utf8Text(text) => {
                assert!(text.contains("hello"))
            }
            other => panic!("unexpected file contents: {other:?}"),
        },
        other => panic!("unexpected file read response: {other:?}"),
    }

    let preview = dispatcher.dispatch_envelope(ClientRequestEnvelope {
        request_id: RequestId(4),
        command: ClientCommand::PreviewRequest(PreviewRequest {
            source: model,
            defines: vec![],
            kind: PreviewRequestKind::GeometryArtifact,
            configured_openscad_path: None,
        }),
    });
    match preview.result.expect("preview should succeed") {
        CommandSuccess::PreviewReady(response) => match response.artifact {
            PreviewArtifact::Mesh(mesh) => assert!(!mesh.positions.is_empty()),
            other => panic!("unexpected preview artifact: {other:?}"),
        },
        other => panic!("unexpected preview response: {other:?}"),
    }

    assert!(pushes.lock().expect("push buffer lock").is_empty());
    cleanup_workspace(&workspace);
}

fn handshake_request() -> CapabilityHandshakeRequest {
    CapabilityHandshakeRequest {
        capabilities: ClientCapabilities {
            client_name: "dispatcher-test".into(),
            platform: ClientPlatform::Desktop,
            protocol_version: ProtocolVersionRange::new(1, 1),
            file_read: web_file_read_capability(),
            supported_preview_kinds: vec![PreviewRequestKind::GeometryArtifact],
        },
    }
}

fn temp_workspace(label: &str) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!(
        "{label}-{}-{}",
        std::process::id(),
        std::thread::current().name().unwrap_or("test")
    ));
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("README.md"), "hello dispatcher").unwrap();

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
