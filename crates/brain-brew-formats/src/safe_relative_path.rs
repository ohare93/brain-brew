//! Portable syntax for paths that may be authorized beneath a selected root.
//!
//! This module deliberately performs no filesystem I/O. The CLI binds a parsed
//! path to a canonical root and checks link-aware containment before access.

use std::fmt;
use std::path::{Path, PathBuf};

/// A non-empty UTF-8 relative path made only from portable normal components.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SafeRelativePath(String);

impl SafeRelativePath {
    /// Validate portable syntax before any path is joined to a filesystem root.
    pub fn new(raw: impl AsRef<str>) -> Result<Self, SafeRelativePathError> {
        let raw = raw.as_ref();
        if raw.is_empty() {
            return Err(SafeRelativePathError::Empty);
        }
        if raw.contains('\0') {
            return Err(SafeRelativePathError::Nul);
        }
        if raw.chars().any(char::is_control) {
            return Err(SafeRelativePathError::Control);
        }
        if raw.chars().any(is_ambiguous_format_control) {
            return Err(SafeRelativePathError::FormatControl);
        }
        if raw.starts_with("//") || raw.starts_with('/') {
            return Err(SafeRelativePathError::Absolute);
        }
        if raw.starts_with("\\\\") {
            return Err(SafeRelativePathError::WindowsUnc);
        }
        let bytes = raw.as_bytes();
        if bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
            return Err(SafeRelativePathError::WindowsDrivePrefix);
        }
        if raw.contains('\\') {
            return Err(SafeRelativePathError::Backslash);
        }
        if raw.contains(':') {
            return Err(SafeRelativePathError::UrlSchemeDelimiter);
        }
        for component in raw.split('/') {
            match component {
                "" => return Err(SafeRelativePathError::EmptyComponent),
                "." => return Err(SafeRelativePathError::DotComponent),
                ".." => return Err(SafeRelativePathError::ParentComponent),
                _ if component.ends_with([' ', '.']) => {
                    return Err(SafeRelativePathError::TrailingDotOrSpace);
                }
                _ if is_windows_reserved_name(component) => {
                    return Err(SafeRelativePathError::WindowsReservedName);
                }
                _ => {}
            }
        }
        Ok(Self(raw.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn as_path(&self) -> &Path {
        Path::new(&self.0)
    }

    pub fn to_path_buf(&self) -> PathBuf {
        PathBuf::from(&self.0)
    }
}

impl AsRef<Path> for SafeRelativePath {
    fn as_ref(&self) -> &Path {
        self.as_path()
    }
}

impl fmt::Display for SafeRelativePath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// Stable reason a path cannot be represented as portable safe-relative syntax.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SafeRelativePathError {
    Empty,
    Nul,
    Control,
    FormatControl,
    Absolute,
    WindowsDrivePrefix,
    WindowsUnc,
    Backslash,
    UrlSchemeDelimiter,
    EmptyComponent,
    DotComponent,
    ParentComponent,
    TrailingDotOrSpace,
    WindowsReservedName,
}

impl fmt::Display for SafeRelativePathError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Empty => "path is empty",
            Self::Nul => "path contains a NUL byte",
            Self::Control => "path contains a control character",
            Self::FormatControl => "path contains an ambiguous Unicode format control",
            Self::Absolute => "path is absolute or rooted",
            Self::WindowsDrivePrefix => "path has a Windows drive prefix",
            Self::WindowsUnc => "path is a Windows UNC path",
            Self::Backslash => "path contains an ambiguous backslash separator",
            Self::UrlSchemeDelimiter => "path contains an ambiguous URL scheme delimiter (`:`)",
            Self::EmptyComponent => "path contains an empty component",
            Self::DotComponent => "path contains a current-directory (`.`) component",
            Self::ParentComponent => "path contains a parent-directory (`..`) component",
            Self::TrailingDotOrSpace => "path component ends in a non-portable dot or space",
            Self::WindowsReservedName => "path contains a Windows reserved device name",
        })
    }
}

impl std::error::Error for SafeRelativePathError {}

fn is_ambiguous_format_control(ch: char) -> bool {
    matches!(
        ch,
        '\u{200b}'..='\u{200f}'
            | '\u{202a}'..='\u{202e}'
            | '\u{2060}'..='\u{206f}'
            | '\u{feff}'
    )
}

fn is_windows_reserved_name(component: &str) -> bool {
    let stem = component.split('.').next().unwrap_or(component);
    matches!(
        stem.to_ascii_uppercase().as_str(),
        "CON"
            | "PRN"
            | "AUX"
            | "NUL"
            | "COM1"
            | "COM2"
            | "COM3"
            | "COM4"
            | "COM5"
            | "COM6"
            | "COM7"
            | "COM8"
            | "COM9"
            | "LPT1"
            | "LPT2"
            | "LPT3"
            | "LPT4"
            | "LPT5"
            | "LPT6"
            | "LPT7"
            | "LPT8"
            | "LPT9"
    )
}
