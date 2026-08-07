//! Canonical-root filesystem authorization for schema-owned relative paths.

use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use brain_brew_formats::safe_relative_path::SafeRelativePath;

/// A selected, canonical filesystem root. Selecting the root is a caller policy
/// decision; paths authorized beneath it can never select another root.
#[derive(Clone, Debug)]
pub(crate) struct PathAuthorizer {
    root_name: String,
    canonical_root: PathBuf,
}

impl PathAuthorizer {
    pub(crate) fn new(root_name: impl Into<String>, root: &Path) -> Result<Self, String> {
        let root_name = root_name.into();
        let canonical_root = fs::canonicalize(root)
            .map_err(|error| format!("selected {root_name} root {}: {error}", root.display()))?;
        let metadata = fs::metadata(&canonical_root).map_err(|error| {
            format!(
                "selected {root_name} root {}: {error}",
                canonical_root.display()
            )
        })?;
        if !metadata.is_dir() {
            return Err(format!(
                "selected {root_name} root {} is not a directory",
                canonical_root.display()
            ));
        }
        Ok(Self {
            root_name,
            canonical_root,
        })
    }

    /// Authorize an existing file or directory after resolving all symlinks.
    pub(crate) fn authorize_read(
        &self,
        declaring_file: &Path,
        field: impl Into<String>,
        raw: &str,
    ) -> Result<AuthorizedPath, Box<PathAuthorizationError>> {
        let request = self.request(declaring_file, field, raw)?;
        let joined = self.canonical_root.join(request.relative.as_path());
        let resolved = fs::canonicalize(&joined).map_err(|error| {
            request.error(format!("target cannot be resolved for reading: {error}"))
        })?;
        self.require_contained(&request, &resolved)?;
        Ok(AuthorizedPath { resolved })
    }

    /// Authorize an existing path relative to the directory containing its
    /// referring source, while retaining the selected-root confinement policy.
    pub(crate) fn authorize_read_relative_to(
        &self,
        referring_source: &Path,
        field: impl Into<String>,
        raw: &str,
    ) -> Result<AuthorizedPath, Box<PathAuthorizationError>> {
        let request = self.request(referring_source, field, raw)?;
        let referring_source = fs::canonicalize(referring_source).map_err(|error| {
            request.error(format!("referring source cannot be resolved: {error}"))
        })?;
        self.require_contained(&request, &referring_source)?;
        let parent = referring_source
            .parent()
            .ok_or_else(|| request.error("referring source has no parent directory".to_owned()))?;
        let joined = parent.join(request.relative.as_path());
        let resolved = fs::canonicalize(&joined).map_err(|error| {
            request.error(format!("target cannot be resolved for reading: {error}"))
        })?;
        self.require_contained(&request, &resolved)?;
        Ok(AuthorizedPath { resolved })
    }

    /// Authorize an existing or new target by resolving its deepest existing
    /// ancestor. Missing suffixes are appended only after the ancestor is known
    /// to be a contained directory.
    pub(crate) fn authorize_create(
        &self,
        declaring_file: &Path,
        field: impl Into<String>,
        raw: &str,
    ) -> Result<AuthorizedPath, Box<PathAuthorizationError>> {
        let request = self.request(declaring_file, field, raw)?;
        let joined = self.canonical_root.join(request.relative.as_path());
        match fs::symlink_metadata(&joined) {
            Ok(_) => {
                let resolved = fs::canonicalize(&joined).map_err(|error| {
                    request.error(format!("existing target cannot be resolved: {error}"))
                })?;
                self.require_contained(&request, &resolved)?;
                return Ok(AuthorizedPath { resolved });
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(request.error(format!("target cannot be inspected: {error}")));
            }
        }

        let mut ancestor = joined.as_path();
        let mut suffix = Vec::new();
        loop {
            match fs::symlink_metadata(ancestor) {
                Ok(_) => break,
                Err(error) if error.kind() == io::ErrorKind::NotFound => {
                    let name = ancestor.file_name().ok_or_else(|| {
                        request.error("target has no existing ancestor".to_owned())
                    })?;
                    suffix.push(name.to_os_string());
                    ancestor = ancestor.parent().ok_or_else(|| {
                        request.error("target has no existing ancestor".to_owned())
                    })?;
                }
                Err(error) => {
                    return Err(request.error(format!(
                        "target ancestor {} cannot be inspected: {error}",
                        ancestor.display()
                    )));
                }
            }
        }
        let resolved_ancestor = fs::canonicalize(ancestor).map_err(|error| {
            request.error(format!(
                "target ancestor {} cannot be resolved: {error}",
                ancestor.display()
            ))
        })?;
        self.require_contained(&request, &resolved_ancestor)?;
        if !fs::metadata(&resolved_ancestor)
            .map_err(|error| {
                request.error(format!("target ancestor cannot be inspected: {error}"))
            })?
            .is_dir()
        {
            return Err(request.error(format!(
                "deepest existing ancestor {} is not a directory",
                resolved_ancestor.display()
            )));
        }
        let mut resolved = resolved_ancestor;
        for component in suffix.iter().rev() {
            resolved.push(component);
        }
        Ok(AuthorizedPath { resolved })
    }

    fn request(
        &self,
        declaring_file: &Path,
        field: impl Into<String>,
        raw: &str,
    ) -> Result<PathRequest, Box<PathAuthorizationError>> {
        let field = field.into();
        let relative = SafeRelativePath::new(raw).map_err(|reason| {
            Box::new(PathAuthorizationError {
                declaring_file: declaring_file.to_path_buf(),
                field: field.clone(),
                raw: raw.to_owned(),
                root_name: self.root_name.clone(),
                root: self.canonical_root.clone(),
                reason: reason.to_string(),
            })
        })?;
        Ok(PathRequest {
            declaring_file: declaring_file.to_path_buf(),
            field,
            raw: raw.to_owned(),
            root_name: self.root_name.clone(),
            root: self.canonical_root.clone(),
            relative,
        })
    }

    fn require_contained(
        &self,
        request: &PathRequest,
        resolved: &Path,
    ) -> Result<(), Box<PathAuthorizationError>> {
        if resolved.starts_with(&self.canonical_root) {
            Ok(())
        } else {
            Err(request.error(format!(
                "resolved path {} escapes the selected root",
                resolved.display()
            )))
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AuthorizedPath {
    resolved: PathBuf,
}

impl AuthorizedPath {
    #[cfg(test)]
    pub(crate) fn as_path(&self) -> &Path {
        &self.resolved
    }

    pub(crate) fn into_path_buf(self) -> PathBuf {
        self.resolved
    }
}

struct PathRequest {
    declaring_file: PathBuf,
    field: String,
    raw: String,
    root_name: String,
    root: PathBuf,
    relative: SafeRelativePath,
}

impl PathRequest {
    fn error(&self, reason: String) -> Box<PathAuthorizationError> {
        Box::new(PathAuthorizationError {
            declaring_file: self.declaring_file.clone(),
            field: self.field.clone(),
            raw: self.raw.clone(),
            root_name: self.root_name.clone(),
            root: self.root.clone(),
            reason,
        })
    }
}

/// Typed diagnostic for a denied filesystem path.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PathAuthorizationError {
    pub(crate) declaring_file: PathBuf,
    pub(crate) field: String,
    pub(crate) raw: String,
    pub(crate) root_name: String,
    pub(crate) root: PathBuf,
    pub(crate) reason: String,
}

impl fmt::Display for PathAuthorizationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}:{} path {:?} is not authorized under {} root {}: {}",
            self.declaring_file.display(),
            self.field,
            self.raw,
            self.root_name,
            self.root.display(),
            self.reason
        )
    }
}

impl std::error::Error for PathAuthorizationError {}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::PathAuthorizer;

    #[test]
    fn diagnostic_names_declaration_field_value_root_and_reason() {
        let root = TempDir::new().unwrap();
        let authorizer = PathAuthorizer::new("package", root.path()).unwrap();
        let declaring = root.path().join("brainbrew.yaml");
        let error = authorizer
            .authorize_read(&declaring, "base", "../outside.yaml")
            .unwrap_err();
        let message = error.to_string();
        assert!(message.contains(&declaring.display().to_string()));
        assert!(message.contains(":base"));
        assert!(message.contains("\"../outside.yaml\""));
        assert!(message.contains("package root"));
        assert!(message.contains("parent-directory"));
    }

    #[test]
    fn authorizes_existing_and_new_contained_targets() {
        let root = TempDir::new().unwrap();
        fs::create_dir_all(root.path().join("nested")).unwrap();
        fs::write(root.path().join("nested/deck.yaml"), "deck").unwrap();
        let authorizer = PathAuthorizer::new("workspace", root.path()).unwrap();
        assert_eq!(
            authorizer
                .authorize_read(Path::new("brainbrew.yaml"), "base", "nested/deck.yaml")
                .unwrap()
                .as_path(),
            fs::canonicalize(root.path().join("nested/deck.yaml")).unwrap()
        );
        assert_eq!(
            authorizer
                .authorize_create(
                    Path::new("brainbrew.yaml"),
                    "targets.x.exports.crowdanki.out",
                    "nested/new/deck.json",
                )
                .unwrap()
                .as_path(),
            fs::canonicalize(root.path().join("nested"))
                .unwrap()
                .join("new/deck.json")
        );
    }

    #[test]
    fn authorizes_reads_relative_to_the_referring_source_parent() {
        let root = TempDir::new().unwrap();
        fs::create_dir_all(root.path().join("authoring/sources/data")).unwrap();
        let descriptor = root.path().join("authoring/sources/descriptor.yaml");
        fs::write(&descriptor, "descriptor").unwrap();
        fs::write(root.path().join("authoring/sources/data/notes.csv"), "csv").unwrap();
        let authorizer = PathAuthorizer::new("package", root.path()).unwrap();

        assert_eq!(
            authorizer
                .authorize_read_relative_to(&descriptor, "tables.main.path", "data/notes.csv")
                .unwrap()
                .as_path(),
            fs::canonicalize(root.path().join("authoring/sources/data/notes.csv")).unwrap()
        );
    }

    #[cfg(unix)]
    #[test]
    fn relative_reads_reject_referring_sources_and_targets_outside_selected_root() {
        use std::os::unix::fs::symlink;

        let root = TempDir::new().unwrap();
        let outside = TempDir::new().unwrap();
        fs::create_dir_all(root.path().join("authoring")).unwrap();
        let referring = root.path().join("authoring/deck.yaml");
        fs::write(&referring, "deck").unwrap();
        fs::write(outside.path().join("descriptor.yaml"), "descriptor").unwrap();
        symlink(outside.path(), root.path().join("authoring/escape")).unwrap();
        let authorizer = PathAuthorizer::new("package", root.path()).unwrap();

        for result in [
            authorizer.authorize_read_relative_to(
                &referring,
                "notes.descriptor",
                "escape/descriptor.yaml",
            ),
            authorizer.authorize_read_relative_to(
                &outside.path().join("descriptor.yaml"),
                "tables.main.path",
                "descriptor.yaml",
            ),
        ] {
            let error = result.unwrap_err().to_string();
            assert!(error.contains("escapes the selected root"), "{error}");
        }
    }

    #[cfg(unix)]
    #[test]
    fn rejects_existing_and_nonexistent_targets_through_escaping_symlinks() {
        use std::os::unix::fs::symlink;

        let root = TempDir::new().unwrap();
        let outside = TempDir::new().unwrap();
        fs::write(outside.path().join("secret.yaml"), "secret").unwrap();
        symlink(outside.path(), root.path().join("escape")).unwrap();
        let authorizer = PathAuthorizer::new("package", root.path()).unwrap();

        for result in [
            authorizer.authorize_read(Path::new("brainbrew.yaml"), "base", "escape/secret.yaml"),
            authorizer.authorize_create(
                Path::new("brainbrew.yaml"),
                "overlays.x.file",
                "escape/new/overlay.yaml",
            ),
        ] {
            let error = result.unwrap_err().to_string();
            assert!(error.contains("escapes the selected root"), "{error}");
        }
    }

    use std::path::Path;
}
