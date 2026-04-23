use app_server_host::{WebSocketHostConfig, run_websocket_host_once};
use app_server_protocol::{
    CapabilityHandshakeRequest, ClientCapabilities, ClientCommand, ClientPlatform,
    ClientRequestEnvelope, PreviewRequest, PreviewRequestKind, ProtocolVersionRange, RequestId,
    web_file_read_capability,
};
use app_server_transport::{
    ClientEnvelope, ServerEnvelope, decode_server_envelope_text, encode_client_envelope_text,
};
use futures_util::{SinkExt, StreamExt};
use tokio::runtime::Runtime;
use tokio_tungstenite::{connect_async, tungstenite::Message};

#[test]
fn websocket_smoke_roundtrip() {
    let runtime = Runtime::new().unwrap();
    runtime.block_on(async {
        let workspace = temp_workspace();
        let url = run_websocket_host_once(WebSocketHostConfig {
            bind_addr: "127.0.0.1:0".into(),
            workspace_path: workspace.clone(),
        })
        .await
        .unwrap();

        let (mut socket, _) = connect_async(&url).await.unwrap();
        let handshake = ClientEnvelope::Handshake(CapabilityHandshakeRequest {
            capabilities: ClientCapabilities {
                client_name: "desktop-smoke".into(),
                platform: ClientPlatform::Desktop,
                protocol_version: ProtocolVersionRange::new(1, 1),
                file_read: web_file_read_capability(),
                supported_preview_kinds: vec![PreviewRequestKind::GeometryArtifact],
            },
        });
        socket
            .send(Message::Text(
                encode_client_envelope_text(&handshake).unwrap().into(),
            ))
            .await
            .unwrap();
        let _ = recv_server_message(&mut socket).await;

        send_request(&mut socket, RequestId(1), ClientCommand::WorkspaceCurrent).await;
        let current = recv_server_message(&mut socket).await;
        let ServerEnvelope::Response(current) = current else {
            panic!("expected response")
        };
        let workspace_id = match current.result.unwrap() {
            app_server_protocol::CommandSuccess::WorkspaceCurrent(response) => {
                response.workspace_id
            }
            other => panic!("unexpected response: {other:?}"),
        };

        send_request(
            &mut socket,
            RequestId(2),
            ClientCommand::WorkspaceList(app_server_protocol::WorkspaceListRequest {
                directory: None,
            }),
        )
        .await;
        let list = recv_server_message(&mut socket).await;
        let ServerEnvelope::Response(list) = list else {
            panic!("expected response")
        };
        let entries = match list.result.unwrap() {
            app_server_protocol::CommandSuccess::WorkspaceList(response) => response.entries,
            other => panic!("unexpected response: {other:?}"),
        };

        let readme = entries
            .iter()
            .find(|entry| entry.path.display_path() == "README.md")
            .unwrap()
            .path
            .clone();
        let model = entries
            .iter()
            .find(|entry| entry.path.display_path() == "model.stl")
            .unwrap()
            .path
            .clone();
        assert_eq!(readme.workspace_id().0, workspace_id.0);

        send_request(
            &mut socket,
            RequestId(3),
            ClientCommand::FileRead(app_server_protocol::FileReadRequest { path: readme }),
        )
        .await;
        let file_read = recv_server_message(&mut socket).await;
        let ServerEnvelope::Response(file_read) = file_read else {
            panic!("expected response")
        };
        match file_read.result.unwrap() {
            app_server_protocol::CommandSuccess::FileRead(response) => match response.contents {
                app_server_protocol::FileReadContents::Utf8Text(text) => {
                    assert!(text.contains("hello"))
                }
                other => panic!("unexpected file contents: {other:?}"),
            },
            other => panic!("unexpected response: {other:?}"),
        }

        send_request(
            &mut socket,
            RequestId(4),
            ClientCommand::PreviewRequest(PreviewRequest {
                source: model,
                defines: vec![],
                kind: PreviewRequestKind::GeometryArtifact,
                configured_openscad_path: None,
            }),
        )
        .await;
        let preview = recv_server_message(&mut socket).await;
        let ServerEnvelope::Response(preview) = preview else {
            panic!("expected response")
        };
        match preview.result.unwrap() {
            app_server_protocol::CommandSuccess::PreviewReady(response) => {
                match response.artifact {
                    app_server_protocol::PreviewArtifact::Mesh(mesh) => {
                        assert!(!mesh.positions.is_empty())
                    }
                    other => panic!("unexpected artifact: {other:?}"),
                }
            }
            other => panic!("unexpected response: {other:?}"),
        }

        let _ = socket
            .send(Message::Text(
                encode_client_envelope_text(&ClientEnvelope::Close)
                    .unwrap()
                    .into(),
            ))
            .await;
        let _ = std::fs::remove_file(workspace.join("README.md"));
        let _ = std::fs::remove_file(workspace.join("model.stl"));
        let _ = std::fs::remove_dir(workspace);
    });
}

async fn send_request(
    socket: &mut tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
    request_id: RequestId,
    command: ClientCommand,
) {
    let message = ClientEnvelope::Request(ClientRequestEnvelope {
        request_id,
        command,
    });
    socket
        .send(Message::Text(
            encode_client_envelope_text(&message).unwrap().into(),
        ))
        .await
        .unwrap();
}

async fn recv_server_message(
    socket: &mut tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
) -> ServerEnvelope {
    let message = socket.next().await.unwrap().unwrap();
    let text = message.into_text().unwrap();
    decode_server_envelope_text(&text).unwrap()
}

fn temp_workspace() -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!("websocket-smoke-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("README.md"), "hello websocket").unwrap();

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
