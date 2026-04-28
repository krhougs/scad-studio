use app_server_protocol::{ProtocolError, SessionToken};
use app_server_transport::{ClientEnvelope, ServerEnvelope, TransportErrorFrame};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use crate::{HostRequestDispatcher, InProcessHost, MpscTransportAdapter, MpscTransportHarness};

pub fn spawn_in_process_mpsc_host() -> Result<(InProcessHost, MpscTransportAdapter), String> {
    let (transport, harness) = MpscTransportAdapter::pair();
    let (workspace_tx, workspace_rx) = tokio::sync::mpsc::unbounded_channel();
    let host = InProcessHost::with_workspace_binding(workspace_tx);
    std::thread::Builder::new()
        .name("studio-app-server-host".into())
        .spawn(move || {
            let runtime = tokio::runtime::Runtime::new().expect("host runtime should initialize");
            runtime.block_on(async move {
                host_loop(harness, workspace_rx).await;
            });
        })
        .map_err(|error| format!("启动 in-process host 失败: {error}"))?;
    Ok((host, transport))
}

async fn host_loop(
    harness: MpscTransportHarness,
    mut workspace_rx: tokio::sync::mpsc::UnboundedReceiver<PathBuf>,
) {
    let push_harness = harness.clone();
    let push_sink = Arc::new(move |push| {
        let _ = push_harness.push_server_message(ServerEnvelope::Push(push));
    });
    let mut dispatcher = HostRequestDispatcher::with_session_token(
        None,
        SessionToken("desktop-session-1".into()),
        Vec::new(),
        push_sink,
    );

    loop {
        while let Ok(path) = workspace_rx.try_recv() {
            dispatcher.rebind_workspace(path);
        }

        match harness.pop_client_message() {
            Some(ClientEnvelope::Handshake(request)) | Some(ClientEnvelope::Reconnect(request)) => {
                match dispatcher.handshake(request) {
                    Ok(response) => {
                        let _ = harness.push_server_message(ServerEnvelope::HandshakeAck(response));
                    }
                    Err(error) => {
                        let _ = harness.push_server_message(ServerEnvelope::TransportError(
                            TransportErrorFrame {
                                message: handshake_error_message(&error),
                            },
                        ));
                        dispatcher.disconnect();
                        break;
                    }
                }
            }
            Some(ClientEnvelope::Request(envelope)) => {
                let response = dispatcher.dispatch_envelope(envelope);
                let _ = harness.push_server_message(ServerEnvelope::Response(response));
            }
            Some(ClientEnvelope::Close) => {
                dispatcher.disconnect();
                break;
            }
            None => tokio::time::sleep(Duration::from_millis(10)).await,
        }
    }
}

fn handshake_error_message(error: &ProtocolError) -> String {
    format!("handshake failed: {:?}: {}", error.code, error.message)
}
