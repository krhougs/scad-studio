use app_server_protocol::{
    FileReadContents, FileReadResponse, PathHandle, ProtocolError, ProtocolErrorCode,
};
use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::resolve_workspace_path;

pub fn read_text_file(path: &Path, context: &str) -> Result<String, String> {
    fs::read_to_string(path).map_err(|error| format!("读取{context}失败: {error}"))
}

pub fn read_binary_file(path: &Path, context: &str) -> Result<Vec<u8>, String> {
    fs::read(path).map_err(|error| format!("读取{context}失败: {error}"))
}

pub fn canonicalize_or_original(path: PathBuf) -> PathBuf {
    fs::canonicalize(&path).unwrap_or(path)
}

pub fn read_file_response(
    workspace_root: &Path,
    handle: &PathHandle,
    denied_extensions: &[String],
) -> Result<FileReadResponse, ProtocolError> {
    let resolved = resolve_workspace_path(workspace_root, handle)?;
    let ext = resolved
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| format!(".{}", value.to_ascii_lowercase()));
    if let Some(ext) = ext.as_ref()
        && denied_extensions.iter().any(|value| value == ext)
    {
        return Err(ProtocolError::new(
            ProtocolErrorCode::UnsupportedFileTypeForClient,
            format!("当前 client 不允许读取 {ext} 文件"),
        ));
    }
    let bytes = fs::read(&resolved).map_err(|error| {
        ProtocolError::new(
            ProtocolErrorCode::NotFound,
            format!("读取文件失败: {error}"),
        )
    })?;
    let contents = match String::from_utf8(bytes.clone()) {
        Ok(text) => FileReadContents::Utf8Text(text),
        Err(_) => FileReadContents::Binary(bytes),
    };
    Ok(FileReadResponse {
        path: handle.clone(),
        media_type: media_type_for_path(&resolved).to_string(),
        contents,
    })
}

fn media_type_for_path(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase())
    {
        Some(ext) if ext == "md" => "text/markdown",
        Some(ext) if ext == "txt" || ext == "scad" => "text/plain",
        Some(ext) if ext == "png" => "image/png",
        Some(ext) if ext == "jpg" || ext == "jpeg" => "image/jpeg",
        _ => "application/octet-stream",
    }
}
