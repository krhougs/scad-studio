use crate::{
    AppConfigDto, ClientEnvelope, CommandSuccess, PreviewArtifact, PreviewMeshPayload,
    ProtocolError, ProtocolErrorCode, ServerEnvelope,
};

pub const WIRE_MAGIC: [u8; 4] = *b"BDNP";
pub const WIRE_VERSION: u8 = 1;
const HEADER_LEN: usize = 5;

pub fn encode_client_frame(envelope: &ClientEnvelope) -> Result<Vec<u8>, ProtocolError> {
    validate_client_envelope(envelope)?;
    encode_frame(envelope)
}

pub fn decode_client_frame(bytes: &[u8]) -> Result<ClientEnvelope, ProtocolError> {
    let envelope = decode_frame(bytes)?;
    validate_client_envelope(&envelope)?;
    Ok(envelope)
}

pub fn encode_server_frame(envelope: &ServerEnvelope) -> Result<Vec<u8>, ProtocolError> {
    validate_server_envelope(envelope)?;
    encode_frame(envelope)
}

pub fn decode_server_frame(bytes: &[u8]) -> Result<ServerEnvelope, ProtocolError> {
    let envelope = decode_frame(bytes)?;
    validate_server_envelope(&envelope)?;
    Ok(envelope)
}

fn encode_frame<T: borsh::BorshSerialize>(value: &T) -> Result<Vec<u8>, ProtocolError> {
    let payload = borsh::to_vec(value).map_err(|error| {
        ProtocolError::new(
            ProtocolErrorCode::InvalidWireFrame,
            format!("编码 wire frame 失败: {error}"),
        )
    })?;
    let mut frame = Vec::with_capacity(HEADER_LEN + payload.len());
    frame.extend_from_slice(&WIRE_MAGIC);
    frame.push(WIRE_VERSION);
    frame.extend(payload);
    Ok(frame)
}

fn decode_frame<T: borsh::BorshDeserialize>(bytes: &[u8]) -> Result<T, ProtocolError> {
    if bytes.len() < HEADER_LEN {
        return Err(ProtocolError::new(
            ProtocolErrorCode::InvalidWireFrame,
            "wire frame header 不完整",
        ));
    }
    if bytes[0..4] != WIRE_MAGIC {
        return Err(ProtocolError::new(
            ProtocolErrorCode::InvalidWireFrame,
            "wire frame magic 不匹配",
        ));
    }
    if bytes[4] != WIRE_VERSION {
        return Err(ProtocolError::new(
            ProtocolErrorCode::UnsupportedWireVersion,
            format!("不支持的 wire version: {}", bytes[4]),
        ));
    }
    borsh::from_slice(&bytes[HEADER_LEN..]).map_err(|error| {
        ProtocolError::new(
            classify_decode_error(&error.to_string()),
            format!("解码 wire frame 失败: {error}"),
        )
    })
}

fn classify_decode_error(message: &str) -> ProtocolErrorCode {
    if message.contains("path segment") {
        ProtocolErrorCode::InvalidPathHandle
    } else {
        ProtocolErrorCode::InvalidWireFrame
    }
}

fn validate_client_envelope(envelope: &ClientEnvelope) -> Result<(), ProtocolError> {
    match envelope {
        ClientEnvelope::Request(request) => match &request.command {
            crate::ClientCommand::ConfigSave(request) => validate_config_dto(&request.config),
            crate::ClientCommand::PreviewRequest(request) => {
                validate_host_local_path_option(&request.configured_openscad_path)
            }
            crate::ClientCommand::SlicerList(request) => request
                .configured
                .iter()
                .try_for_each(|slicer| validate_host_local_path(&slicer.path)),
            crate::ClientCommand::ExportRun(request) => {
                validate_host_local_path_option(&request.configured_openscad_path)?;
                request
                    .configured_slicers
                    .iter()
                    .try_for_each(|slicer| validate_host_local_path(&slicer.path))
            }
            _ => Ok(()),
        },
        _ => Ok(()),
    }
}

fn validate_server_envelope(envelope: &ServerEnvelope) -> Result<(), ProtocolError> {
    match envelope {
        ServerEnvelope::Response(response) => {
            if let Ok(success) = &response.result {
                validate_command_success(success)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn validate_command_success(success: &CommandSuccess) -> Result<(), ProtocolError> {
    match success {
        CommandSuccess::ConfigLoaded(response) => validate_config_dto(&response.config),
        CommandSuccess::PreviewReady(response) => match &response.artifact {
            PreviewArtifact::Mesh(mesh) => validate_mesh(mesh),
            _ => Ok(()),
        },
        CommandSuccess::SlicerListed(response) => response
            .slicers
            .iter()
            .try_for_each(|slicer| validate_host_local_path(&slicer.path)),
        _ => Ok(()),
    }
}

fn validate_config_dto(config: &AppConfigDto) -> Result<(), ProtocolError> {
    validate_host_local_path_option(&config.openscad_path)?;
    config
        .slicers
        .iter()
        .try_for_each(|slicer| validate_host_local_path(&slicer.path))?;
    config
        .recent_workspaces
        .iter()
        .try_for_each(validate_host_local_path)?;
    validate_f32(config.floating_panel_opacity, "floating_panel_opacity")?;
    validate_f32(config.left_panel_width, "left_panel_width")?;
    validate_f32(config.right_panel_width, "right_panel_width")?;
    validate_optional_pair(config.camera_overlay_pos, "camera_overlay_pos")?;
    validate_optional_pair(config.camera_overlay_size, "camera_overlay_size")?;
    validate_optional_pair(config.param_panel_pos, "param_panel_pos")?;
    validate_optional_pair(config.param_panel_size, "param_panel_size")?;
    validate_optional_pair(config.log_panel_pos, "log_panel_pos")?;
    validate_optional_pair(config.log_panel_size, "log_panel_size")
}

fn validate_mesh(mesh: &PreviewMeshPayload) -> Result<(), ProtocolError> {
    for position in &mesh.positions {
        validate_array(position, "positions")?;
    }
    for normal in &mesh.normals {
        validate_array(normal, "normals")?;
    }
    for color in &mesh.vertex_colors {
        validate_array(color, "vertex_colors")?;
    }
    Ok(())
}

fn validate_array<const N: usize>(values: &[f32; N], field: &str) -> Result<(), ProtocolError> {
    for value in values {
        validate_f32(*value, field)?;
    }
    Ok(())
}

fn validate_optional_pair(values: Option<[f32; 2]>, field: &str) -> Result<(), ProtocolError> {
    if let Some(values) = values {
        validate_array(&values, field)?;
    }
    Ok(())
}

pub fn validate_f32(value: f32, field: &str) -> Result<(), ProtocolError> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(ProtocolError::new(
            ProtocolErrorCode::InvalidNumericValue,
            format!("{field} 必须是有限浮点数"),
        ))
    }
}

fn validate_host_local_path_option(
    path: &Option<crate::HostLocalPath>,
) -> Result<(), ProtocolError> {
    if let Some(path) = path {
        validate_host_local_path(path)?;
    }
    Ok(())
}

fn validate_host_local_path(path: &crate::HostLocalPath) -> Result<(), ProtocolError> {
    crate::HostLocalPath::new(path.as_str()).map(|_| ())
}
