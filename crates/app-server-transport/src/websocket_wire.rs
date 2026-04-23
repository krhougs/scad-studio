use crate::{ClientEnvelope, ServerEnvelope};

pub fn encode_client_envelope_text(message: &ClientEnvelope) -> Result<String, serde_json::Error> {
    serde_json::to_string(message)
}

pub fn decode_client_envelope_text(text: &str) -> Result<ClientEnvelope, serde_json::Error> {
    serde_json::from_str(text)
}

pub fn encode_server_envelope_text(message: &ServerEnvelope) -> Result<String, serde_json::Error> {
    serde_json::to_string(message)
}

pub fn decode_server_envelope_text(text: &str) -> Result<ServerEnvelope, serde_json::Error> {
    serde_json::from_str(text)
}
