use app_server_protocol::{
    CancelRequest, CapabilityHandshakeRequest, CapabilityHandshakeResponse, ClientCommand,
    ClientEnvelope, ClientRequestEnvelope, RequestId, ServerEnvelope, ServerPushEnvelope,
    ServerResponseEnvelope, TransportErrorFrame, WatchSubscribeRequest, decode_server_frame,
    encode_client_frame,
};

use super::types::ClientError;

#[derive(Debug)]
pub enum InboundFrame {
    HandshakeAck(CapabilityHandshakeResponse),
    Response(ServerResponseEnvelope),
    Push(ServerPushEnvelope),
    TransportError(TransportErrorFrame),
    Closed,
}

pub fn encode_handshake(request: &CapabilityHandshakeRequest) -> Vec<u8> {
    let frame = ClientEnvelope::Handshake(request.clone());
    encode_client_frame(&frame).expect("handshake frame encoding is infallible")
}

pub fn encode_reconnect_envelope(request: &CapabilityHandshakeRequest) -> Vec<u8> {
    let frame = ClientEnvelope::Reconnect(request.clone());
    encode_client_frame(&frame).expect("reconnect frame encoding is infallible")
}

pub fn encode_request_envelope(envelope: &ClientRequestEnvelope) -> Vec<u8> {
    let frame = ClientEnvelope::Request(envelope.clone());
    encode_client_frame(&frame).expect("request frame encoding is infallible")
}

pub fn build_request_envelope(request_id: RequestId, command: ClientCommand) -> Vec<u8> {
    encode_request_envelope(&ClientRequestEnvelope {
        request_id,
        command,
    })
}

pub fn build_watch_subscribe_envelope(
    request_id: RequestId,
    request: WatchSubscribeRequest,
) -> Vec<u8> {
    build_request_envelope(request_id, ClientCommand::WatchSubscribe(request))
}

pub fn build_cancel_envelope(cancel_request_id: RequestId, target: RequestId) -> Vec<u8> {
    build_request_envelope(
        cancel_request_id,
        ClientCommand::Cancel(CancelRequest { request_id: target }),
    )
}

pub fn decode_inbound(bytes: &[u8]) -> Result<InboundFrame, ClientError> {
    let envelope: ServerEnvelope =
        decode_server_frame(bytes).map_err(|err| ClientError::DecodeError {
            context: format!("inbound binary frame: {:?}: {}", err.code(), err.message),
        })?;
    Ok(match envelope {
        ServerEnvelope::HandshakeAck(ack) => InboundFrame::HandshakeAck(ack),
        ServerEnvelope::Response(response) => InboundFrame::Response(response),
        ServerEnvelope::Push(push) => InboundFrame::Push(push),
        ServerEnvelope::TransportError(frame) => InboundFrame::TransportError(frame),
        ServerEnvelope::Closed => InboundFrame::Closed,
    })
}
