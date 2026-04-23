#![cfg(all(target_arch = "wasm32", feature = "browser-smoke"))]

use app_server_protocol::{
    CapabilityHandshakeRequest, ClientCapabilities, ClientCommand, ClientPlatform,
    ClientRequestEnvelope, CommandSuccess, FileWriteTextRequest, PathHandle, PreviewRequestKind,
    ProtocolVersionRange, RequestId, ServerResponseEnvelope, web_file_read_capability,
};
use app_server_transport::{ClientTransport, ServerEnvelope, WebSocketClientTransport};
use gloo_timers::future::TimeoutFuture;
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

const SMOKE_WS_URL: &str = "ws://127.0.0.1:39180";
const WATCH_SMOKE_FILENAME: &str = "watch-smoke-generated.txt";

#[wasm_bindgen_test(async)]
async fn browser_smoke_refreshes_listing_after_watch_change() {
    ensure_root();

    studio_web_wasm::boot_studio_web(SMOKE_WS_URL).expect("boot wasm app");

    wait_for_text("README.md").await;
    assert!(
        !body_text().contains(WATCH_SMOKE_FILENAME),
        "watch smoke file should not be present before background mutation"
    );

    write_watch_smoke_file().await;
    wait_for_text(WATCH_SMOKE_FILENAME).await;
}

async fn write_watch_smoke_file() {
    let mut transport = WebSocketClientTransport::connect(SMOKE_WS_URL).expect("connect writer");
    wait_for_socket_open(&transport).await;

    ClientTransport::handshake(&mut transport, handshake_request()).expect("send handshake");
    wait_for_handshake_ack(&mut transport).await;

    let workspace = match request_success(
        &mut transport,
        RequestId(1),
        ClientCommand::WorkspaceCurrent,
    )
    .await
    {
        CommandSuccess::WorkspaceCurrent(current) => current,
        other => panic!("unexpected workspace.current response: {other:?}"),
    };

    let file_handle = PathHandle::new(
        workspace.workspace_id,
        vec![WATCH_SMOKE_FILENAME.to_string()],
    )
    .expect("watch smoke file handle should be valid");

    match request_success(
        &mut transport,
        RequestId(2),
        ClientCommand::FileWriteText(FileWriteTextRequest {
            path: file_handle,
            contents: "watch smoke mutation\n".into(),
        }),
    )
    .await
    {
        CommandSuccess::FileWritten(_) => {}
        other => panic!("unexpected file.write_text response: {other:?}"),
    }

    let _ = ClientTransport::close(&mut transport);
}

fn ensure_root() {
    let document = web_sys::window()
        .and_then(|window| window.document())
        .expect("document should exist");
    document.set_title("studio-web browser watch smoke");
    if document.get_element_by_id("studio-web-root").is_none() {
        let root = document.create_element("div").expect("create smoke root");
        root.set_id("studio-web-root");
        document
            .body()
            .expect("body should exist")
            .append_child(&root)
            .expect("append smoke root");
    }
}

fn body_text() -> String {
    web_sys::window()
        .and_then(|window| window.document())
        .and_then(|document| document.body())
        .and_then(|body| body.text_content())
        .unwrap_or_default()
}

async fn wait_for_text(expected: &str) {
    let mut last_body = String::new();
    for _ in 0..120 {
        let body = body_text();
        if body.contains(expected) {
            return;
        }
        last_body = body;
        TimeoutFuture::new(50).await;
    }
    panic!("expected text not found: {expected}; current body: {last_body}");
}

async fn wait_for_socket_open(transport: &WebSocketClientTransport) {
    for _ in 0..120 {
        if transport.is_open() {
            return;
        }
        TimeoutFuture::new(50).await;
    }
    panic!("writer websocket did not become ready");
}

async fn wait_for_handshake_ack(transport: &mut WebSocketClientTransport) {
    for _ in 0..120 {
        if let Some(message) = ClientTransport::next_server_message(transport).expect("poll server")
        {
            match message {
                ServerEnvelope::HandshakeAck(_) => return,
                ServerEnvelope::TransportError(error) => {
                    panic!("writer handshake transport error: {}", error.message)
                }
                _ => {}
            }
        }
        TimeoutFuture::new(50).await;
    }
    panic!("writer handshake ack timed out");
}

async fn request_success(
    transport: &mut WebSocketClientTransport,
    request_id: RequestId,
    command: ClientCommand,
) -> CommandSuccess {
    ClientTransport::request(
        transport,
        ClientRequestEnvelope {
            request_id,
            command,
        },
    )
    .expect("send command over writer transport");

    wait_for_response(transport, request_id)
        .await
        .result
        .expect("protocol success")
}

async fn wait_for_response(
    transport: &mut WebSocketClientTransport,
    request_id: RequestId,
) -> ServerResponseEnvelope {
    for _ in 0..120 {
        if let Some(message) = ClientTransport::next_server_message(transport).expect("poll server")
        {
            match message {
                ServerEnvelope::Response(response) if response.request_id == request_id => {
                    return response;
                }
                ServerEnvelope::TransportError(error) => {
                    panic!("writer transport error: {}", error.message)
                }
                _ => {}
            }
        }
        TimeoutFuture::new(50).await;
    }
    panic!("writer response timed out for request {}", request_id.0);
}

fn handshake_request() -> CapabilityHandshakeRequest {
    CapabilityHandshakeRequest {
        capabilities: ClientCapabilities {
            client_name: "studio-web-watch-smoke-writer".into(),
            platform: ClientPlatform::Web,
            protocol_version: ProtocolVersionRange::new(1, 1),
            file_read: web_file_read_capability(),
            supported_preview_kinds: vec![PreviewRequestKind::GeometryArtifact],
        },
    }
}
