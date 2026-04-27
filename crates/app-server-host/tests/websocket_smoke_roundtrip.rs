use app_server_host::{WebSocketHostConfig, run_websocket_host_once};
use app_server_protocol::{
    CapabilityHandshakeRequest, ClientCapabilities, ClientCommand, ClientPlatform,
    ClientRequestEnvelope, PreviewRequest, PreviewRequestKind, ProtocolVersionRange, RequestId,
    web_file_read_capability,
};
use app_server_transport::{
    ClientEnvelope, ServerEnvelope, decode_server_envelope_binary, encode_client_envelope_binary,
};
use futures_util::{SinkExt, StreamExt};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::runtime::Runtime;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use yawc::frame::Frame;
use yawc::{CompressionLevel, Options, WebSocket};

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
                protocol_version: ProtocolVersionRange::new(2, 2),
                file_read: web_file_read_capability(),
                supported_preview_kinds: vec![PreviewRequestKind::GeometryArtifact],
            },
        });
        let handshake_bytes = encode_client_envelope_binary(&handshake).unwrap();
        socket
            .send(Message::Binary(handshake_bytes.into()))
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
            .find(|entry| {
                entry
                    .path
                    .as_ref()
                    .is_some_and(|path| path.display_path() == "README.md")
            })
            .unwrap()
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
            .unwrap()
            .path
            .as_ref()
            .expect("model entry should be operable")
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
                    app_server_protocol::PreviewArtifact::Stl(stl) => {
                        assert!(!stl.bytes.is_empty())
                    }
                    other => panic!("unexpected artifact: {other:?}"),
                }
            }
            other => panic!("unexpected response: {other:?}"),
        }

        let _ = socket
            .send(Message::Binary(
                encode_client_envelope_binary(&ClientEnvelope::Close)
                    .unwrap()
                    .into(),
            ))
            .await;
        let _ = std::fs::remove_file(workspace.join("README.md"));
        let _ = std::fs::remove_file(workspace.join("model.stl"));
        let _ = std::fs::remove_dir(workspace);
    });
}

#[test]
fn websocket_negotiates_permessage_deflate() {
    let runtime = Runtime::new().unwrap();
    runtime.block_on(async {
        let workspace = temp_workspace();
        let url = run_websocket_host_once(WebSocketHostConfig {
            bind_addr: "127.0.0.1:0".into(),
            workspace_path: workspace.clone(),
        })
        .await
        .unwrap();
        let address = url.strip_prefix("ws://").unwrap();
        let mut stream = tokio::net::TcpStream::connect(address).await.unwrap();
        let request = format!(
            "GET / HTTP/1.1\r\nHost: {address}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 13\r\nSec-WebSocket-Extensions: permessage-deflate\r\n\r\n"
        );
        stream.write_all(request.as_bytes()).await.unwrap();
        let mut buffer = vec![0; 2048];
        let read = stream.read(&mut buffer).await.unwrap();
        let response = String::from_utf8_lossy(&buffer[..read]).to_ascii_lowercase();
        assert!(response.contains("101 switching protocols"));
        assert!(response.contains("sec-websocket-extensions: permessage-deflate"));

        cleanup_workspace(workspace);
    });
}

#[test]
fn websocket_compressed_client_roundtrip_handles_large_frame() {
    let runtime = Runtime::new().unwrap();
    runtime.block_on(async {
        let workspace = temp_workspace();
        let large_bytes = vec![0xff_u8; 2 * 1024 * 1024];
        std::fs::write(workspace.join("large.bin"), &large_bytes).unwrap();
        let url = run_websocket_host_once(WebSocketHostConfig {
            bind_addr: "127.0.0.1:0".into(),
            workspace_path: workspace.clone(),
        })
        .await
        .unwrap();

        let mut socket = WebSocket::connect(url.parse().unwrap())
            .with_options(
                Options::default()
                    .with_compression_level(CompressionLevel::fast())
                    .with_max_payload_read(64 * 1024 * 1024)
                    .with_max_read_buffer(64 * 1024 * 1024),
            )
            .await
            .unwrap();
        let handshake = ClientEnvelope::Handshake(CapabilityHandshakeRequest {
            capabilities: ClientCapabilities {
                client_name: "compressed-smoke".into(),
                platform: ClientPlatform::Web,
                protocol_version: ProtocolVersionRange::new(2, 2),
                file_read: web_file_read_capability(),
                supported_preview_kinds: vec![PreviewRequestKind::GeometryArtifact],
            },
        });
        socket
            .send(Frame::binary(
                encode_client_envelope_binary(&handshake).unwrap(),
            ))
            .await
            .unwrap();
        let _ = recv_yawc_server_message(&mut socket).await;

        send_yawc_request(&mut socket, RequestId(1), ClientCommand::WorkspaceCurrent).await;
        let current = recv_yawc_server_message(&mut socket).await;
        let ServerEnvelope::Response(current) = current else {
            panic!("expected response")
        };
        let workspace_id = match current.result.unwrap() {
            app_server_protocol::CommandSuccess::WorkspaceCurrent(response) => {
                response.workspace_id
            }
            other => panic!("unexpected response: {other:?}"),
        };

        let large_path = app_server_protocol::PathHandle::new(workspace_id, ["large.bin"])
            .expect("large file path should be valid");
        send_yawc_request(
            &mut socket,
            RequestId(2),
            ClientCommand::FileRead(app_server_protocol::FileReadRequest { path: large_path }),
        )
        .await;
        let file_read = recv_yawc_server_message(&mut socket).await;
        let ServerEnvelope::Response(file_read) = file_read else {
            panic!("expected response")
        };
        match file_read.result.unwrap() {
            app_server_protocol::CommandSuccess::FileRead(response) => match response.contents {
                app_server_protocol::FileReadContents::Binary(bytes) => {
                    assert_eq!(bytes.len(), large_bytes.len());
                    assert_eq!(bytes[0], 0xff);
                }
                other => panic!("unexpected file contents: {other:?}"),
            },
            other => panic!("unexpected response: {other:?}"),
        }

        let _ = socket
            .send(Frame::binary(
                encode_client_envelope_binary(&ClientEnvelope::Close).unwrap(),
            ))
            .await;
        cleanup_workspace(workspace);
    });
}

#[test]
fn websocket_accepts_large_inbound_frame_before_protocol_decode() {
    let runtime = Runtime::new().unwrap();
    runtime.block_on(async {
        let workspace = temp_workspace();
        let url = run_websocket_host_once(WebSocketHostConfig {
            bind_addr: "127.0.0.1:0".into(),
            workspace_path: workspace.clone(),
        })
        .await
        .unwrap();

        let mut socket = WebSocket::connect(url.parse().unwrap())
            .with_options(
                Options::default()
                    .with_compression_level(CompressionLevel::fast())
                    .with_max_payload_read(64 * 1024 * 1024)
                    .with_max_read_buffer(64 * 1024 * 1024),
            )
            .await
            .unwrap();
        let large_invalid_frame = vec![0_u8; 2 * 1024 * 1024];
        socket
            .send(Frame::binary(large_invalid_frame))
            .await
            .unwrap();

        let rejected = recv_yawc_server_message(&mut socket).await;
        let ServerEnvelope::TransportError(error) = rejected else {
            panic!("expected transport error")
        };
        assert!(error.message.contains("magic"));

        cleanup_workspace(workspace);
    });
}

#[test]
fn websocket_rejects_text_handshake_frame() {
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
        socket
            .send(Message::Text(r#"{"kind":"close"}"#.into()))
            .await
            .unwrap();
        let rejected = recv_server_message(&mut socket).await;
        let ServerEnvelope::TransportError(error) = rejected else {
            panic!("expected transport error")
        };
        assert!(error.message.contains("binary"));

        cleanup_workspace(workspace);
    });
}

#[test]
fn websocket_rejects_unsupported_wire_version_before_dispatch() {
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
        let mut frame = encode_client_envelope_binary(&ClientEnvelope::Close).unwrap();
        frame[4] = app_server_protocol::WIRE_VERSION.saturating_add(1);
        socket.send(Message::Binary(frame.into())).await.unwrap();
        let rejected = recv_server_message(&mut socket).await;
        let ServerEnvelope::TransportError(error) = rejected else {
            panic!("expected transport error")
        };
        assert!(error.message.contains("wire version"));

        cleanup_workspace(workspace);
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
    let bytes = encode_client_envelope_binary(&message).unwrap();
    socket.send(Message::Binary(bytes.into())).await.unwrap();
}

async fn recv_server_message(
    socket: &mut tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
) -> ServerEnvelope {
    let message = socket.next().await.unwrap().unwrap();
    let bytes = message.into_data();
    decode_server_envelope_binary(&bytes).unwrap()
}

async fn send_yawc_request<S>(socket: &mut S, request_id: RequestId, command: ClientCommand)
where
    S: futures_util::Sink<Frame, Error = yawc::WebSocketError> + Unpin,
{
    let message = ClientEnvelope::Request(ClientRequestEnvelope {
        request_id,
        command,
    });
    let bytes = encode_client_envelope_binary(&message).unwrap();
    socket.send(Frame::binary(bytes)).await.unwrap();
}

async fn recv_yawc_server_message<S>(socket: &mut S) -> ServerEnvelope
where
    S: futures_util::Stream<Item = Frame> + Unpin,
{
    let message = socket.next().await.unwrap();
    decode_server_envelope_binary(message.payload()).unwrap()
}

fn temp_workspace() -> std::path::PathBuf {
    static NEXT_WORKSPACE_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    let sequence = NEXT_WORKSPACE_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "websocket-smoke-{}-{sequence}-{unique}",
        std::process::id()
    ));
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

fn cleanup_workspace(workspace: std::path::PathBuf) {
    let _ = std::fs::remove_dir_all(workspace);
}
