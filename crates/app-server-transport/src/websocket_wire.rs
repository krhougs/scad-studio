use crate::{ClientEnvelope, ServerEnvelope};
use app_server_protocol::{
    ProtocolError, decode_client_frame, decode_server_frame, encode_client_frame,
    encode_server_frame,
};

pub fn encode_client_envelope_binary(message: &ClientEnvelope) -> Result<Vec<u8>, ProtocolError> {
    encode_client_frame(message)
}

pub fn decode_client_envelope_binary(bytes: &[u8]) -> Result<ClientEnvelope, ProtocolError> {
    decode_client_frame(bytes)
}

pub fn encode_server_envelope_binary(message: &ServerEnvelope) -> Result<Vec<u8>, ProtocolError> {
    encode_server_frame(message)
}

pub fn decode_server_envelope_binary(bytes: &[u8]) -> Result<ServerEnvelope, ProtocolError> {
    decode_server_frame(bytes)
}
