use app_server_protocol::{
    CancelRequest, CapabilityHandshakeRequest, CapabilityHandshakeResponse, ClientCommand,
    ClientRequestEnvelope, RequestId, ServerPushEnvelope, ServerResponseEnvelope,
    WatchSubscribeRequest,
};
use serde::{Deserialize, Serialize};

use super::types::ClientError;

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "frame", content = "payload", rename_all = "snake_case")]
pub enum OutboundFrame {
    Handshake(CapabilityHandshakeRequest),
    Request(ClientRequestEnvelope),
}

#[derive(Debug)]
pub enum InboundFrame {
    HandshakeAck(CapabilityHandshakeResponse),
    Response(ServerResponseEnvelope),
    Push(ServerPushEnvelope),
}

pub fn encode_handshake(request: &CapabilityHandshakeRequest) -> Vec<u8> {
    let frame = OutboundFrame::Handshake(request.clone());
    serde_json::to_vec(&frame).expect("handshake frame serialization is infallible")
}

pub fn encode_request_envelope(envelope: &ClientRequestEnvelope) -> Vec<u8> {
    let frame = OutboundFrame::Request(envelope.clone());
    serde_json::to_vec(&frame).expect("request frame serialization is infallible")
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
    let value: serde_json::Value = serde_json::from_slice(bytes).map_err(|err| {
        ClientError::DecodeError {
            context: format!("inbound json parse: {err}"),
        }
    })?;

    if let Some(tag) = value
        .get("frame")
        .and_then(|tag| tag.as_str())
        .map(|tag| tag.to_string())
    {
        return decode_tagged_inbound(&tag, &value);
    }

    decode_untagged_inbound(&value)
}

fn decode_tagged_inbound(tag: &str, value: &serde_json::Value) -> Result<InboundFrame, ClientError> {
    let payload = value
        .get("payload")
        .cloned()
        .ok_or_else(|| ClientError::DecodeError {
            context: format!("inbound frame '{tag}' missing payload"),
        })?;
    match tag {
        "handshake_ack" => {
            let ack: CapabilityHandshakeResponse =
                serde_json::from_value(payload).map_err(|err| ClientError::DecodeError {
                    context: format!("handshake_ack payload: {err}"),
                })?;
            Ok(InboundFrame::HandshakeAck(ack))
        }
        "response" => {
            let response: ServerResponseEnvelope =
                serde_json::from_value(payload).map_err(|err| ClientError::DecodeError {
                    context: format!("response payload: {err}"),
                })?;
            Ok(InboundFrame::Response(response))
        }
        "push" => {
            let push: ServerPushEnvelope =
                serde_json::from_value(payload).map_err(|err| ClientError::DecodeError {
                    context: format!("push payload: {err}"),
                })?;
            Ok(InboundFrame::Push(push))
        }
        other => Err(ClientError::DecodeError {
            context: format!("unknown inbound frame tag '{other}'"),
        }),
    }
}

fn decode_untagged_inbound(value: &serde_json::Value) -> Result<InboundFrame, ClientError> {
    if value.get("event").is_some() {
        let push: ServerPushEnvelope = serde_json::from_value(value.clone()).map_err(|err| {
            ClientError::DecodeError {
                context: format!("inbound push: {err}"),
            }
        })?;
        return Ok(InboundFrame::Push(push));
    }
    if value.get("result").is_some() && value.get("request_id").is_some() {
        let response: ServerResponseEnvelope =
            serde_json::from_value(value.clone()).map_err(|err| ClientError::DecodeError {
                context: format!("inbound response: {err}"),
            })?;
        return Ok(InboundFrame::Response(response));
    }
    if value.get("session_token").is_some() && value.get("server_capabilities").is_some() {
        let ack: CapabilityHandshakeResponse =
            serde_json::from_value(value.clone()).map_err(|err| ClientError::DecodeError {
                context: format!("inbound handshake_ack: {err}"),
            })?;
        return Ok(InboundFrame::HandshakeAck(ack));
    }
    Err(ClientError::DecodeError {
        context: "inbound bytes did not match any known frame shape".into(),
    })
}
