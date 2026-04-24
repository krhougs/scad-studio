use borsh::{BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Deserializer, Serialize};
use std::{fmt, io};
use unicode_casefold::UnicodeCaseFold;
use unicode_general_category::{GeneralCategory, get_general_category};
use unicode_normalization::UnicodeNormalization;
use unicode_segmentation::UnicodeSegmentation;

const MAX_SEGMENTS: usize = 32;
const MAX_SEGMENT_GRAPHEMES: usize = 80;
const MAX_SEGMENT_BYTES: usize = 180;
const MAX_DISPLAY_BYTES: usize = 240;

#[derive(
    Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, BorshSerialize, BorshDeserialize,
)]
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
        validate_path_shape(&normalized)?;
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

    pub fn case_fold_key(&self) -> String {
        self.path_segments
            .iter()
            .map(|segment| segment.as_str().case_fold().collect::<String>())
            .collect::<Vec<_>>()
            .join("/")
    }

    pub fn resolve_relative_link(
        base: &Self,
        link: &str,
    ) -> Result<Self, PathHandleValidationError> {
        let path = strip_fragment(link)?;
        validate_relative_link_prefix(path)?;
        let decoded = percent_decode(path)?;
        let mut segments = base
            .path_segments
            .get(..base.path_segments.len().saturating_sub(1))
            .unwrap_or(&[])
            .to_vec();

        for component in decoded.split('/') {
            match component {
                "" | "." => {}
                ".." => {
                    if segments.pop().is_none() {
                        return Err(PathHandleValidationError::RelativePathEscapesRoot);
                    }
                }
                value => segments.push(value.to_string()),
            }
        }

        PathHandle::new(base.workspace_id.clone(), segments)
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

#[derive(BorshSerialize, BorshDeserialize)]
struct RawPathHandle {
    workspace_id: WorkspaceId,
    path_segments: Vec<String>,
}

impl BorshSerialize for PathHandle {
    fn serialize<W: io::Write>(&self, writer: &mut W) -> io::Result<()> {
        RawPathHandle {
            workspace_id: self.workspace_id.clone(),
            path_segments: self.path_segments.clone(),
        }
        .serialize(writer)
    }
}

impl BorshDeserialize for PathHandle {
    fn deserialize_reader<R: io::Read>(reader: &mut R) -> io::Result<Self> {
        let raw = RawPathHandle::deserialize_reader(reader)?;
        PathHandle::new(raw.workspace_id, raw.path_segments)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
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
    if normalized.starts_with("..") {
        return Err(PathHandleValidationError::DotDotSegment);
    }
    if normalized.contains('/') || normalized.contains('\\') {
        return Err(PathHandleValidationError::NativeSeparator);
    }
    validate_segment_length(&normalized)?;
    validate_segment_edges(&normalized)?;
    validate_reserved_name(&normalized)?;
    validate_segment_characters(&normalized)?;
    Ok(normalized)
}

fn validate_path_shape(segments: &[String]) -> Result<(), PathHandleValidationError> {
    if segments.len() > MAX_SEGMENTS {
        return Err(PathHandleValidationError::TooDeep);
    }
    if segments.join("/").len() > MAX_DISPLAY_BYTES {
        return Err(PathHandleValidationError::PathTooLong);
    }
    Ok(())
}

fn validate_segment_length(segment: &str) -> Result<(), PathHandleValidationError> {
    let grapheme_count = segment.graphemes(true).count();
    if grapheme_count == 0 {
        return Err(PathHandleValidationError::EmptySegment);
    }
    if grapheme_count > MAX_SEGMENT_GRAPHEMES || segment.len() > MAX_SEGMENT_BYTES {
        return Err(PathHandleValidationError::SegmentTooLong);
    }
    Ok(())
}

fn validate_segment_edges(segment: &str) -> Result<(), PathHandleValidationError> {
    let first = segment
        .graphemes(true)
        .next()
        .ok_or(PathHandleValidationError::EmptySegment)?;
    if matches!(first, " " | "-" | "]" | ")" | "=") || starts_with_mark(first) {
        return Err(PathHandleValidationError::LeadingDisallowed);
    }
    let last = segment
        .graphemes(true)
        .last()
        .ok_or(PathHandleValidationError::EmptySegment)?;
    if last == " " || last == "." {
        return Err(PathHandleValidationError::TrailingSpaceOrDot);
    }
    Ok(())
}

fn starts_with_mark(grapheme: &str) -> bool {
    grapheme
        .chars()
        .next()
        .is_some_and(|value| is_mark(get_general_category(value)))
}

fn validate_reserved_name(segment: &str) -> Result<(), PathHandleValidationError> {
    let stem = segment.split('.').next().unwrap_or(segment);
    let uppercase = stem.to_ascii_uppercase();
    let reserved = matches!(uppercase.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || reserved_numbered_name(&uppercase, "COM")
        || reserved_numbered_name(&uppercase, "LPT");
    if reserved {
        Err(PathHandleValidationError::WindowsReservedName)
    } else {
        Ok(())
    }
}

fn reserved_numbered_name(value: &str, prefix: &str) -> bool {
    value.len() == 4 && value.starts_with(prefix) && matches!(value.as_bytes()[3], b'1'..=b'9')
}

fn validate_segment_characters(segment: &str) -> Result<(), PathHandleValidationError> {
    for grapheme in segment.graphemes(true) {
        if is_allowed_ascii_grapheme(grapheme) || emojis::get(grapheme).is_some() {
            continue;
        }
        if grapheme.chars().all(is_allowed_unicode_char) {
            continue;
        }
        return Err(PathHandleValidationError::DisallowedCharacter);
    }
    Ok(())
}

fn is_allowed_ascii_grapheme(grapheme: &str) -> bool {
    grapheme.len() == 1
        && grapheme.chars().next().is_some_and(|ch| {
            ch.is_ascii_alphanumeric()
                || matches!(
                    ch,
                    ' ' | '.' | '_' | '-' | '@' | '+' | '$' | '[' | ']' | '(' | ')' | '='
                )
        })
}

fn is_allowed_unicode_char(ch: char) -> bool {
    let category = get_general_category(ch);
    is_letter(category) || is_mark(category) || is_number(category)
}

fn is_letter(category: GeneralCategory) -> bool {
    matches!(
        category,
        GeneralCategory::UppercaseLetter
            | GeneralCategory::LowercaseLetter
            | GeneralCategory::TitlecaseLetter
            | GeneralCategory::ModifierLetter
            | GeneralCategory::OtherLetter
    )
}

fn is_mark(category: GeneralCategory) -> bool {
    matches!(
        category,
        GeneralCategory::NonspacingMark
            | GeneralCategory::SpacingMark
            | GeneralCategory::EnclosingMark
    )
}

fn is_number(category: GeneralCategory) -> bool {
    matches!(
        category,
        GeneralCategory::DecimalNumber
            | GeneralCategory::LetterNumber
            | GeneralCategory::OtherNumber
    )
}

fn strip_fragment(link: &str) -> Result<&str, PathHandleValidationError> {
    let path = link.split_once('#').map_or(link, |(path, _)| path);
    if path.contains('?') {
        return Err(PathHandleValidationError::RelativeLinkQuery);
    }
    Ok(path)
}

fn validate_relative_link_prefix(path: &str) -> Result<(), PathHandleValidationError> {
    if path.starts_with('/')
        || path.starts_with('\\')
        || path.starts_with("//")
        || path.contains("://")
        || looks_like_windows_drive_path(path)
    {
        return Err(PathHandleValidationError::RelativeLinkNotPortable);
    }
    Ok(())
}

fn looks_like_windows_drive_path(path: &str) -> bool {
    let bytes = path.as_bytes();
    bytes.len() >= 3 && bytes[1] == b':' && bytes[0].is_ascii_alphabetic()
}

fn percent_decode(value: &str) -> Result<String, PathHandleValidationError> {
    let bytes = value.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            if index + 2 >= bytes.len() {
                return Err(PathHandleValidationError::InvalidPercentEncoding);
            }
            let high = hex_value(bytes[index + 1])?;
            let low = hex_value(bytes[index + 2])?;
            output.push((high << 4) | low);
            index += 3;
        } else {
            output.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(output).map_err(|_| PathHandleValidationError::InvalidPercentEncoding)
}

fn hex_value(value: u8) -> Result<u8, PathHandleValidationError> {
    match value {
        b'0'..=b'9' => Ok(value - b'0'),
        b'a'..=b'f' => Ok(value - b'a' + 10),
        b'A'..=b'F' => Ok(value - b'A' + 10),
        _ => Err(PathHandleValidationError::InvalidPercentEncoding),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathHandleValidationError {
    EmptySegment,
    SingleDotSegment,
    DotDotSegment,
    NativeSeparator,
    LeadingDisallowed,
    TrailingSpaceOrDot,
    SegmentTooLong,
    TooDeep,
    PathTooLong,
    DisallowedCharacter,
    WindowsReservedName,
    RelativePathEscapesRoot,
    RelativeLinkNotPortable,
    RelativeLinkQuery,
    InvalidPercentEncoding,
}

impl fmt::Display for PathHandleValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::EmptySegment => "path segment cannot be empty",
            Self::SingleDotSegment => "path segment cannot be '.'",
            Self::DotDotSegment => "path segment cannot be '..'",
            Self::NativeSeparator => "path segment cannot contain native separators",
            Self::LeadingDisallowed => "path segment starts with a disallowed character",
            Self::TrailingSpaceOrDot => "path segment cannot end with a space or dot",
            Self::SegmentTooLong => "path segment is too long",
            Self::TooDeep => "path is too deep",
            Self::PathTooLong => "path is too long",
            Self::DisallowedCharacter => "path segment contains a disallowed character",
            Self::WindowsReservedName => "path segment uses a Windows reserved name",
            Self::RelativePathEscapesRoot => "relative path escapes workspace root",
            Self::RelativeLinkNotPortable => "relative link is not a portable workspace path",
            Self::RelativeLinkQuery => "relative link query string is not supported",
            Self::InvalidPercentEncoding => "relative link contains invalid percent encoding",
        };
        f.write_str(message)
    }
}

impl std::error::Error for PathHandleValidationError {}
