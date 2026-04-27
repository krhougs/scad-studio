#[cfg(target_arch = "wasm32")]
use app_server_protocol::{
    AppConfigDto, ClientCommand, ClientEnvelope, ClientRequestEnvelope, HostLocalPath, PathHandle,
    ServerEnvelope, WorkspaceId, decode_client_frame, decode_server_frame, encode_client_frame,
    encode_server_frame,
};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(start)]
pub fn protocol_wasm_start() {}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn protocol_decode_client_frame(bytes: &[u8]) -> Result<JsValue, JsValue> {
    let envelope = decode_client_frame(bytes).map_err(protocol_error)?;
    to_js_value(&envelope)
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn protocol_decode_server_frame(bytes: &[u8]) -> Result<JsValue, JsValue> {
    let envelope = decode_server_frame(bytes).map_err(protocol_error)?;
    to_js_value(&envelope)
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn protocol_encode_close_frame() -> Result<Vec<u8>, JsValue> {
    encode_client_frame(&ClientEnvelope::Close).map_err(protocol_error)
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn protocol_encode_handshake_frame(request: JsValue) -> Result<Vec<u8>, JsValue> {
    let request = from_js_value(request, "invalid handshake request")?;
    encode_client_frame(&ClientEnvelope::Handshake(request)).map_err(protocol_error)
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn protocol_encode_reconnect_frame(request: JsValue) -> Result<Vec<u8>, JsValue> {
    let request = from_js_value(request, "invalid reconnect request")?;
    encode_client_frame(&ClientEnvelope::Reconnect(request)).map_err(protocol_error)
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn protocol_encode_workspace_current_request(request_id: u64) -> Result<Vec<u8>, JsValue> {
    encode_command(request_id, ClientCommand::WorkspaceCurrent)
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn protocol_encode_workspace_list_request(
    request_id: u64,
    request: JsValue,
) -> Result<Vec<u8>, JsValue> {
    encode_typed_command(
        request_id,
        request,
        "invalid workspace.list request",
        ClientCommand::WorkspaceList,
    )
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn protocol_encode_config_load_request(request_id: u64) -> Result<Vec<u8>, JsValue> {
    encode_command(request_id, ClientCommand::ConfigLoad)
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn protocol_encode_config_save_request(
    request_id: u64,
    request: JsValue,
) -> Result<Vec<u8>, JsValue> {
    encode_typed_command(
        request_id,
        request,
        "invalid config.save request",
        ClientCommand::ConfigSave,
    )
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn protocol_encode_file_read_request(
    request_id: u64,
    request: JsValue,
) -> Result<Vec<u8>, JsValue> {
    encode_typed_command(
        request_id,
        request,
        "invalid file.read request",
        ClientCommand::FileRead,
    )
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn protocol_encode_file_write_text_request(
    request_id: u64,
    request: JsValue,
) -> Result<Vec<u8>, JsValue> {
    encode_typed_command(
        request_id,
        request,
        "invalid file.write_text request",
        ClientCommand::FileWriteText,
    )
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn protocol_encode_preview_request(
    request_id: u64,
    request: JsValue,
) -> Result<Vec<u8>, JsValue> {
    encode_typed_command(
        request_id,
        request,
        "invalid preview.request request",
        ClientCommand::PreviewRequest,
    )
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn protocol_encode_cadquery_execute_request(
    request_id: u64,
    request: JsValue,
) -> Result<Vec<u8>, JsValue> {
    encode_typed_command(
        request_id,
        request,
        "invalid cadquery.execute request",
        ClientCommand::CadQueryExecute,
    )
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn protocol_encode_cadquery_preview_request(
    request_id: u64,
    request: JsValue,
) -> Result<Vec<u8>, JsValue> {
    encode_typed_command(
        request_id,
        request,
        "invalid cadquery.preview request",
        ClientCommand::CadQueryPreview,
    )
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn protocol_encode_cadquery_result_get_request(
    request_id: u64,
    request: JsValue,
) -> Result<Vec<u8>, JsValue> {
    encode_typed_command(
        request_id,
        request,
        "invalid cadquery.result.get request",
        ClientCommand::CadQueryResultGet,
    )
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn protocol_encode_slicer_list_request(
    request_id: u64,
    request: JsValue,
) -> Result<Vec<u8>, JsValue> {
    encode_typed_command(
        request_id,
        request,
        "invalid slicer.list request",
        ClientCommand::SlicerList,
    )
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn protocol_encode_export_run_request(
    request_id: u64,
    request: JsValue,
) -> Result<Vec<u8>, JsValue> {
    encode_typed_command(
        request_id,
        request,
        "invalid export.run request",
        ClientCommand::ExportRun,
    )
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn protocol_encode_watch_subscribe_request(
    request_id: u64,
    request: JsValue,
) -> Result<Vec<u8>, JsValue> {
    encode_typed_command(
        request_id,
        request,
        "invalid watch.subscribe request",
        ClientCommand::WatchSubscribe,
    )
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn protocol_encode_watch_unsubscribe_request(
    request_id: u64,
    request: JsValue,
) -> Result<Vec<u8>, JsValue> {
    encode_typed_command(
        request_id,
        request,
        "invalid watch.unsubscribe request",
        ClientCommand::WatchUnsubscribe,
    )
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn protocol_encode_cancel_request(
    request_id: u64,
    request: JsValue,
) -> Result<Vec<u8>, JsValue> {
    encode_typed_command(
        request_id,
        request,
        "invalid cancel request",
        ClientCommand::Cancel,
    )
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn protocol_encode_session_reclaim_request(
    request_id: u64,
    request: JsValue,
) -> Result<Vec<u8>, JsValue> {
    encode_typed_command(
        request_id,
        request,
        "invalid session.reclaim request",
        ClientCommand::SessionReclaim,
    )
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn protocol_path_handle(workspace_id: &str, segments: JsValue) -> Result<JsValue, JsValue> {
    let segments: Vec<String> = serde_wasm_bindgen::from_value(segments).map_err(|error| {
        js_error(
            "invalid_path_handle",
            format!("invalid path segments: {error}"),
        )
    })?;
    let handle = PathHandle::new(WorkspaceId::new(workspace_id), segments).map_err(path_error)?;
    to_js_value(&handle)
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn protocol_resolve_relative_link(base: JsValue, link: &str) -> Result<JsValue, JsValue> {
    let base: PathHandle = serde_wasm_bindgen::from_value(base)
        .map_err(|error| js_error("invalid_path_handle", format!("invalid base path: {error}")))?;
    let resolved = PathHandle::resolve_relative_link(&base, link).map_err(path_error)?;
    to_js_value(&resolved)
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn protocol_validate_host_local_path(path: &str) -> Result<JsValue, JsValue> {
    let path = HostLocalPath::new(path).map_err(protocol_error)?;
    to_js_value(&path)
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn protocol_validate_finite_number(value: f64, field: &str) -> Result<f64, JsValue> {
    app_server_protocol::validate_f32(value as f32, field)
        .map(|_| value)
        .map_err(protocol_error)
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn protocol_validate_config(value: JsValue) -> Result<Vec<u8>, JsValue> {
    let config: AppConfigDto = from_js_value(value, "invalid config DTO")?;
    let envelope = ServerEnvelope::Response(app_server_protocol::ServerResponseEnvelope {
        request_id: app_server_protocol::RequestId(0),
        result: Ok(app_server_protocol::CommandSuccess::ConfigLoaded(
            app_server_protocol::ConfigLoadResponse { config },
        )),
    });
    encode_server_frame(&envelope).map_err(protocol_error)
}

#[cfg(target_arch = "wasm32")]
fn encode_typed_command<T>(
    request_id: u64,
    request: JsValue,
    label: &str,
    command: impl FnOnce(T) -> ClientCommand,
) -> Result<Vec<u8>, JsValue>
where
    T: serde::de::DeserializeOwned,
{
    let request = from_js_value(request, label)?;
    encode_command(request_id, command(request))
}

#[cfg(target_arch = "wasm32")]
fn encode_command(request_id: u64, command: ClientCommand) -> Result<Vec<u8>, JsValue> {
    let envelope = ClientEnvelope::Request(ClientRequestEnvelope {
        request_id: app_server_protocol::RequestId(request_id),
        command,
    });
    encode_client_frame(&envelope).map_err(protocol_error)
}

#[cfg(target_arch = "wasm32")]
fn from_js_value<T: serde::de::DeserializeOwned>(
    value: JsValue,
    label: &str,
) -> Result<T, JsValue> {
    serde_wasm_bindgen::from_value(value)
        .map_err(|error| js_error("invalid_js_value", format!("{label}: {error}")))
}

#[cfg(target_arch = "wasm32")]
fn to_js_value<T: serde::Serialize>(value: &T) -> Result<JsValue, JsValue> {
    serde_wasm_bindgen::to_value(value).map_err(|error| {
        js_error(
            "invalid_js_value",
            format!("serialize to JS failed: {error}"),
        )
    })
}

#[cfg(target_arch = "wasm32")]
fn protocol_error(error: app_server_protocol::ProtocolError) -> JsValue {
    js_error(protocol_error_code(error.code), error.message)
}

#[cfg(target_arch = "wasm32")]
fn path_error(error: app_server_protocol::PathHandleValidationError) -> JsValue {
    js_error(path_error_code(&error), error.to_string())
}

#[cfg(target_arch = "wasm32")]
fn js_error(code: &'static str, message: impl Into<String>) -> JsValue {
    #[derive(serde::Serialize)]
    struct JsProtocolError {
        code: &'static str,
        message: String,
    }

    serde_wasm_bindgen::to_value(&JsProtocolError {
        code,
        message: message.into(),
    })
    .unwrap_or_else(|_| JsValue::from_str(code))
}

#[cfg(target_arch = "wasm32")]
fn protocol_error_code(code: app_server_protocol::ProtocolErrorCode) -> &'static str {
    match code {
        app_server_protocol::ProtocolErrorCode::InvalidCommand => "invalid_command",
        app_server_protocol::ProtocolErrorCode::InvalidPathHandle => "invalid_path_handle",
        app_server_protocol::ProtocolErrorCode::UnsupportedFileTypeForClient => {
            "unsupported_file_type_for_client"
        }
        app_server_protocol::ProtocolErrorCode::StalePathHandle => "stale_path_handle",
        app_server_protocol::ProtocolErrorCode::Cancelled => "cancelled",
        app_server_protocol::ProtocolErrorCode::SessionExpired => "session_expired",
        app_server_protocol::ProtocolErrorCode::UnsupportedProtocolVersion => {
            "unsupported_protocol_version"
        }
        app_server_protocol::ProtocolErrorCode::NotFound => "not_found",
        app_server_protocol::ProtocolErrorCode::Internal => "internal",
        app_server_protocol::ProtocolErrorCode::InvalidWireFrame => "invalid_wire_frame",
        app_server_protocol::ProtocolErrorCode::UnsupportedWireVersion => {
            "unsupported_wire_version"
        }
        app_server_protocol::ProtocolErrorCode::InvalidNumericValue => "invalid_numeric_value",
        app_server_protocol::ProtocolErrorCode::InvalidHostLocalPath => "invalid_host_local_path",
    }
}

#[cfg(target_arch = "wasm32")]
fn path_error_code(error: &app_server_protocol::PathHandleValidationError) -> &'static str {
    match error {
        app_server_protocol::PathHandleValidationError::EmptySegment => "empty_segment",
        app_server_protocol::PathHandleValidationError::SingleDotSegment => "single_dot_segment",
        app_server_protocol::PathHandleValidationError::DotDotSegment => "dot_dot_segment",
        app_server_protocol::PathHandleValidationError::NativeSeparator => "native_separator",
        app_server_protocol::PathHandleValidationError::LeadingDisallowed => "leading_disallowed",
        app_server_protocol::PathHandleValidationError::TrailingSpaceOrDot => {
            "trailing_space_or_dot"
        }
        app_server_protocol::PathHandleValidationError::SegmentTooLong => "segment_too_long",
        app_server_protocol::PathHandleValidationError::TooDeep => "too_deep",
        app_server_protocol::PathHandleValidationError::PathTooLong => "path_too_long",
        app_server_protocol::PathHandleValidationError::DisallowedCharacter => {
            "disallowed_character"
        }
        app_server_protocol::PathHandleValidationError::WindowsReservedName => {
            "windows_reserved_name"
        }
        app_server_protocol::PathHandleValidationError::RelativePathEscapesRoot => {
            "relative_path_escapes_root"
        }
        app_server_protocol::PathHandleValidationError::RelativeLinkNotPortable => {
            "relative_link_not_portable"
        }
        app_server_protocol::PathHandleValidationError::RelativeLinkQuery => "relative_link_query",
        app_server_protocol::PathHandleValidationError::InvalidPercentEncoding => {
            "invalid_percent_encoding"
        }
    }
}
