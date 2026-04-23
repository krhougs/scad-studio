use serde::{Deserialize, Deserializer, Serialize};
use std::fmt;
use unicode_normalization::UnicodeNormalization;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct WorkspaceId(pub String);

impl WorkspaceId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct PathHandle {
    workspace_id: WorkspaceId,
    path_segments: Vec<String>,
}

impl PathHandle {
    pub fn new(
        workspace_id: WorkspaceId,
        path_segments: impl IntoIterator<Item = impl Into<String>>,
    ) -> Result<Self, PathHandleValidationError> {
        let normalized = path_segments
            .into_iter()
            .map(|segment| normalize_segment(segment.into()))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            workspace_id,
            path_segments: normalized,
        })
    }

    pub fn display_path(&self) -> String {
        self.path_segments.join("/")
    }

    pub fn workspace_id(&self) -> &WorkspaceId {
        &self.workspace_id
    }

    pub fn path_segments(&self) -> &[String] {
        &self.path_segments
    }
}

impl<'de> Deserialize<'de> for PathHandle {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct RawPathHandle {
            workspace_id: WorkspaceId,
            path_segments: Vec<String>,
        }

        let raw = RawPathHandle::deserialize(deserializer)?;
        PathHandle::new(raw.workspace_id, raw.path_segments).map_err(serde::de::Error::custom)
    }
}

fn normalize_segment(segment: String) -> Result<String, PathHandleValidationError> {
    let normalized = segment.nfc().collect::<String>();
    if normalized.is_empty() {
        return Err(PathHandleValidationError::EmptySegment);
    }
    if normalized == "." {
        return Err(PathHandleValidationError::SingleDotSegment);
    }
    if normalized == ".." {
        return Err(PathHandleValidationError::DotDotSegment);
    }
    if normalized.contains('/') || normalized.contains('\\') {
        return Err(PathHandleValidationError::NativeSeparator);
    }
    Ok(normalized)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathHandleValidationError {
    EmptySegment,
    SingleDotSegment,
    DotDotSegment,
    NativeSeparator,
}

impl fmt::Display for PathHandleValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::EmptySegment => "path segment cannot be empty",
            Self::SingleDotSegment => "path segment cannot be '.'",
            Self::DotDotSegment => "path segment cannot be '..'",
            Self::NativeSeparator => "path segment cannot contain native separators",
        };
        f.write_str(message)
    }
}

impl std::error::Error for PathHandleValidationError {}
