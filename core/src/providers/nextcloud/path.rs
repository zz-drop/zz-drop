use percent_encoding::{AsciiSet, CONTROLS, utf8_percent_encode};
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PathError {
    #[error("filename is empty")]
    Empty,

    #[error("filename contains a path separator")]
    HasSeparator,

    #[error("filename is `.` or `..`")]
    ParentReference,

    #[error("filename contains a NUL byte")]
    HasNul,
}

/// Characters that must be percent-encoded inside a single path
/// segment. This is conservative: anything outside an unreserved subset
/// or that has special meaning in a URL gets encoded. Slashes are NOT
/// allowed inside a segment (validate_filename rejects them).
const SEGMENT_RESERVED: &AsciiSet = &CONTROLS
    .add(b' ')
    .add(b'"')
    .add(b'#')
    .add(b'<')
    .add(b'>')
    .add(b'?')
    .add(b'`')
    .add(b'{')
    .add(b'}')
    .add(b'%')
    .add(b'^')
    .add(b'|')
    .add(b'\\')
    .add(b'[')
    .add(b']')
    .add(b'@')
    .add(b':')
    .add(b';')
    .add(b'=')
    .add(b'&')
    .add(b'+');

/// Validate that a single filename component is safe to use as a remote
/// path segment.
pub fn validate_filename(name: &str) -> Result<(), PathError> {
    if name.is_empty() {
        return Err(PathError::Empty);
    }
    if name == "." || name == ".." {
        return Err(PathError::ParentReference);
    }
    if name.contains('/') || name.contains('\\') {
        return Err(PathError::HasSeparator);
    }
    if name.contains('\0') {
        return Err(PathError::HasNul);
    }
    Ok(())
}

/// Percent-encode a single filename component for use in a URL path.
/// Slashes are not preserved — call this on each segment separately.
pub fn encode_segment(segment: &str) -> String {
    utf8_percent_encode(segment, SEGMENT_RESERVED).to_string()
}

/// Encode a multi-segment path. Segments are joined with `/`. Each
/// segment is validated and encoded.
pub fn encode_path(segments: &[&str]) -> Result<String, PathError> {
    if segments.is_empty() {
        return Err(PathError::Empty);
    }
    let mut out = String::new();
    for (i, seg) in segments.iter().enumerate() {
        validate_filename(seg)?;
        if i > 0 {
            out.push('/');
        }
        out.push_str(&encode_segment(seg));
    }
    Ok(out)
}

/// Encode a remote root path like `/zz-drop` keeping a leading slash
/// and validating each non-empty segment. Returns `/` for an empty or
/// slash-only input.
pub fn encode_remote_root(root: &str) -> Result<String, PathError> {
    let trimmed = root.trim_matches('/');
    if trimmed.is_empty() {
        return Ok("/".to_string());
    }
    let segments: Vec<&str> = trimmed.split('/').collect();
    let body = encode_path(&segments)?;
    Ok(format!("/{body}"))
}
