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
        for component in raw.split('/') {
            match component {
                "" => return Err(SafeRelativePathError::EmptyComponent),
                "." => return Err(SafeRelativePathError::DotComponent),
                ".." => return Err(SafeRelativePathError::ParentComponent),
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
    Absolute,
    WindowsDrivePrefix,
    WindowsUnc,
    Backslash,
    EmptyComponent,
    DotComponent,
    ParentComponent,
}

impl fmt::Display for SafeRelativePathError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Empty => "path is empty",
            Self::Nul => "path contains a NUL byte",
            Self::Absolute => "path is absolute or rooted",
            Self::WindowsDrivePrefix => "path has a Windows drive prefix",
            Self::WindowsUnc => "path is a Windows UNC path",
            Self::Backslash => "path contains an ambiguous backslash separator",
            Self::EmptyComponent => "path contains an empty component",
            Self::DotComponent => "path contains a current-directory (`.`) component",
            Self::ParentComponent => "path contains a parent-directory (`..`) component",
        })
    }
}

impl std::error::Error for SafeRelativePathError {}
