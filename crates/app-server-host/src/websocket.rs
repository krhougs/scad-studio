use crate::HostRequestDispatcher;
use app_server_protocol::{SessionToken, web_file_read_capability};
use app_server_transport::{
    ClientEnvelope, ServerEnvelope, decode_client_envelope_text, encode_server_envelope_text,
};
use futures_util::{Sink, SinkExt, StreamExt};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio_tungstenite::{accept_async, tungstenite::Message};

#[derive(Debug, Clone)]
pub struct WebSocketHostConfig {
    pub bind_addr: String,
    pub workspace_path: PathBuf,
}

pub async fn run_websocket_host(config: WebSocketHostConfig) -> Result<String, String> {
    let (listener, url) = bind_listener(&config.bind_addr).await?;
    tokio::spawn(async move {
        loop {
            let Ok((stream, _)) = listener.accept().await else {
                break;
            };
            let workspace_path = config.workspace_path.clone();
            tokio::spawn(async move {
                let Ok(socket) = accept_async(stream).await else {
                    return;
                };
                handle_connection(socket, workspace_path).await;
            });
        }
    });
    Ok(url)
}

pub async fn run_websocket_host_once(config: WebSocketHostConfig) -> Result<String, String> {
    let (listener, url) = bind_listener(&config.bind_addr).await?;
    tokio::spawn(async move {
        let (stream, _) = listener.accept().await.expect("accept should succeed");
        let socket = accept_async(stream)
            .await
            .expect("websocket accept should succeed");
        handle_connection(socket, config.workspace_path).await;
    });
    Ok(url)
}

async fn bind_listener(bind_addr: &str) -> Result<(TcpListener, String), String> {
    let listener = TcpListener::bind(bind_addr)
        .await
        .map_err(|error| format!("绑定 WebSocket host 失败: {error}"))?;
    let local_addr = listener.local_addr().map_err(|error| error.to_string())?;
    Ok((listener, format!("ws://{local_addr}")))
}

async fn handle_connection(
    socket: tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>,
    workspace_path: PathBuf,
) {
    let (mut sink, mut stream) = socket.split();
    let (push_tx, mut push_rx) = tokio::sync::mpsc::unbounded_channel::<ServerEnvelope>();
    let push_sink = {
        let push_tx = push_tx.clone();
        Arc::new(move |push| {
            let _ = push_tx.send(ServerEnvelope::Push(push));
        })
    };
    let mut dispatcher = HostRequestDispatcher::with_session_token(
        Some(workspace_path),
        SessionToken("session-1".into()),
        web_file_read_capability().denied_extensions,
        push_sink,
    );

    let handshake = match stream.next().await {
        Some(Ok(message)) => message,
        _ => return,
    };
    let text = match handshake.into_text() {
        Ok(text) => text,
        Err(_) => return,
    };
    let request = match decode_client_envelope_text(&text) {
        Ok(ClientEnvelope::Handshake(request)) | Ok(ClientEnvelope::Reconnect(request)) => request,
        _ => return,
    };
    let response = ServerEnvelope::HandshakeAck(dispatcher.handshake(request));
    if send_server_message(&mut sink, response).await.is_err() {
        return;
    }

    loop {
        tokio::select! {
            maybe_push = push_rx.recv() => {
                let Some(push) = maybe_push else {
                    dispatcher.disconnect();
                    break;
                };
                if send_server_message(&mut sink, push).await.is_err() {
                    dispatcher.disconnect();
                    break;
                }
            }
            maybe_message = stream.next() => {
                let Some(message) = maybe_message else {
                    dispatcher.disconnect();
                    break;
                };
                let Ok(message) = message else {
                    dispatcher.disconnect();
                    break;
                };
                let Ok(text) = message.into_text() else {
                    dispatcher.disconnect();
                    break;
                };
                let Ok(request) = decode_client_envelope_text(&text) else {
                    dispatcher.disconnect();
                    break;
                };
                match request {
                    ClientEnvelope::Request(envelope) => {
                        let response = ServerEnvelope::Response(dispatcher.dispatch_envelope(envelope));
                        if send_server_message(&mut sink, response).await.is_err() {
                            dispatcher.disconnect();
                            break;
                        }
                    }
                    ClientEnvelope::Close => {
                        dispatcher.disconnect();
                        break;
                    }
                    ClientEnvelope::Handshake(_) | ClientEnvelope::Reconnect(_) => {
                        dispatcher.disconnect();
                        break;
                    }
                }
            }
        }
    }
}

async fn send_server_message<S>(
    sink: &mut S,
    message: ServerEnvelope,
) -> Result<(), tokio_tungstenite::tungstenite::Error>
where
    S: Sink<Message, Error = tokio_tungstenite::tungstenite::Error> + Unpin,
{
    let text =
        encode_server_envelope_text(&message).expect("server websocket payload should serialize");
    sink.send(Message::Text(text.into())).await
}
