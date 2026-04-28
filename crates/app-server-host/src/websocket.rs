use crate::HostRequestDispatcher;
use app_server_protocol::{SessionToken, web_file_read_capability};
use app_server_transport::{
    ClientEnvelope, ServerEnvelope, TransportErrorFrame, decode_client_envelope_binary,
    encode_server_envelope_binary,
};
use bytes::Bytes;
use futures_util::{Sink, SinkExt, StreamExt};
use http_body_util::Empty;
use hyper::body::Incoming;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use std::convert::Infallible;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpListener;
use yawc::frame::{Frame, OpCode};
use yawc::{CompressionLevel, Options, WebSocket};

const MAX_WS_PAYLOAD_BYTES: usize = 64 * 1024 * 1024;

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
                serve_websocket_upgrade(stream, workspace_path).await;
            });
        }
    });
    Ok(url)
}

pub async fn run_websocket_host_once(config: WebSocketHostConfig) -> Result<String, String> {
    let (listener, url) = bind_listener(&config.bind_addr).await?;
    tokio::spawn(async move {
        let (stream, _) = listener.accept().await.expect("accept should succeed");
        serve_websocket_upgrade(stream, config.workspace_path).await;
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

async fn serve_websocket_upgrade(stream: tokio::net::TcpStream, workspace_path: PathBuf) {
    let io = TokioIo::new(stream);
    let service = service_fn(move |request| {
        let workspace_path = workspace_path.clone();
        async move { handle_upgrade(request, workspace_path).await }
    });
    let _ = http1::Builder::new()
        .serve_connection(io, service)
        .with_upgrades()
        .await;
}

async fn handle_upgrade(
    mut request: Request<Incoming>,
    workspace_path: PathBuf,
) -> Result<Response<Empty<Bytes>>, Infallible> {
    let options = websocket_options();
    let response = match WebSocket::upgrade_with_options(&mut request, options) {
        Ok((response, upgraded)) => {
            tokio::spawn(async move {
                if let Ok(socket) = upgraded.await {
                    handle_connection(socket, workspace_path).await;
                }
            });
            response
        }
        Err(_) => Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body(Empty::new())
            .expect("websocket bad request response should build"),
    };
    Ok(response)
}

fn websocket_options() -> Options {
    Options::default()
        .with_compression_level(CompressionLevel::fast())
        .with_max_payload_read(MAX_WS_PAYLOAD_BYTES)
        .with_max_read_buffer(MAX_WS_PAYLOAD_BYTES)
}

async fn handle_connection<S>(socket: WebSocket<S>, workspace_path: PathBuf)
where
    S: AsyncRead + AsyncWrite + Unpin,
{
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
        Some(message) => message,
        _ => return,
    };
    let request = match decode_client_message(handshake) {
        Ok(ClientEnvelope::Handshake(request)) | Ok(ClientEnvelope::Reconnect(request)) => request,
        Ok(_) => {
            let _ = send_transport_error(&mut sink, "websocket handshake frame was not handshake")
                .await;
            return;
        }
        Err(error) => {
            let _ = send_server_message(&mut sink, error).await;
            return;
        }
    };
    let response = match dispatcher.handshake(request) {
        Ok(response) => ServerEnvelope::HandshakeAck(response),
        Err(error) => {
            let _ = send_transport_error(
                &mut sink,
                format!("handshake failed: {:?}: {}", error.code, error.message),
            )
            .await;
            return;
        }
    };
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
                let request = match decode_client_message(message) {
                    Ok(request) => request,
                    Err(error) => {
                        let _ = send_server_message(&mut sink, error).await;
                        dispatcher.disconnect();
                        break;
                    }
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
                        let _ = send_transport_error(&mut sink, "unexpected websocket handshake frame").await;
                        dispatcher.disconnect();
                        break;
                    }
                }
            }
        }
    }
}

fn decode_client_message(message: Frame) -> Result<ClientEnvelope, ServerEnvelope> {
    let opcode = message.opcode();
    let bytes = match message {
        message if opcode == OpCode::Binary => message.into_payload(),
        message if opcode == OpCode::Text => {
            let _ = message;
            return Err(transport_error("websocket message must be binary"));
        }
        message if opcode == OpCode::Close => {
            let _ = message;
            return Ok(ClientEnvelope::Close);
        }
        _ => return Err(transport_error("unsupported websocket message kind")),
    };
    decode_client_envelope_binary(&bytes).map_err(|error| {
        transport_error(format!(
            "invalid websocket binary payload: {:?}: {}",
            error.code(),
            error.message
        ))
    })
}

fn transport_error(message: impl Into<String>) -> ServerEnvelope {
    ServerEnvelope::TransportError(TransportErrorFrame {
        message: message.into(),
    })
}

async fn send_transport_error<S>(
    sink: &mut S,
    message: impl Into<String>,
) -> Result<(), yawc::WebSocketError>
where
    S: Sink<Frame, Error = yawc::WebSocketError> + Unpin,
{
    send_server_message(sink, transport_error(message)).await
}

async fn send_server_message<S>(
    sink: &mut S,
    message: ServerEnvelope,
) -> Result<(), yawc::WebSocketError>
where
    S: Sink<Frame, Error = yawc::WebSocketError> + Unpin,
{
    let bytes =
        encode_server_envelope_binary(&message).expect("server websocket payload should serialize");
    sink.send(Frame::binary(bytes)).await
}
