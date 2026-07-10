use std::collections::BTreeMap;
use std::fmt;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use serde::Deserialize;

use crate::yaml_scalar::{
    is_emittable_key as is_emittable_yaml_key, key as yaml_key, scalar as yaml_scalar,
};

/// Current fail-closed federation lock schema version.
pub const LOCKFILE_VERSION: u32 = 2;

/// Reproducible source lock for a set of Federated Deck package inputs.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FederationLock {
    pub version: u32,
    pub packages: BTreeMap<String, LockedPackage>,
}

/// One locked package input.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LockedPackage {
    pub manifest: String,
    pub package: LockedPackageMetadata,
    pub original: OriginalSource,
    pub locked: LockedSource,
}

/// Package metadata captured at lock time.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LockedPackageMetadata {
    pub version: String,
}

/// Maintainer-requested source selector retained for lock updates and review.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OriginalSource {
    Path {
        path: String,
    },
    Git {
        url: String,
        reference: Option<String>,
        rev: Option<String>,
    },
    Tarball {
        url: String,
    },
}

impl OriginalSource {
    pub fn source_type(&self) -> &'static str {
        match self {
            Self::Path { .. } => "path",
            Self::Git { .. } => "git",
            Self::Tarball { .. } => "tarball",
        }
    }
}

/// Immutable source identity used to authenticate one package tree.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LockedSource {
    Path {
        path: String,
        nar_hash: String,
    },
    Git {
        url: String,
        rev: String,
        nar_hash: String,
    },
    Tarball {
        url: String,
        nar_hash: String,
    },
}

impl LockedSource {
    pub fn source_type(&self) -> &'static str {
        match self {
            Self::Path { .. } => "path",
            Self::Git { .. } => "git",
            Self::Tarball { .. } => "tarball",
        }
    }

    pub fn nar_hash(&self) -> &str {
        match self {
            Self::Path { nar_hash, .. }
            | Self::Git { nar_hash, .. }
            | Self::Tarball { nar_hash, .. } => nar_hash,
        }
    }
}

/// Parse a federation lock file from strict YAML.
pub fn from_str(input: &str) -> Result<FederationLock, LockfileError> {
    crate::strict_yaml::reject_duplicate_keys(input).map_err(LockfileError::Parse)?;

    // Check the version before interpreting source fields. In particular, never
    // reinterpret the optional-field v1 schema as a weaker v2 entry.
    let header: LockVersionYaml = serde_yaml::from_str(input).map_err(LockfileError::Parse)?;
    validate_version(header.version)?;

    let yaml: FederationLockYaml = serde_yaml::from_str(input).map_err(LockfileError::Parse)?;
    crate::strict_yaml::reject_unintended_scalars(
        input,
        crate::strict_yaml::ScalarPolicy::Lockfile,
    )
    .map_err(LockfileError::Parse)?;
    yaml.into_lock()
}

/// Parse and re-emit a federation lock file using deterministic formatting.
pub fn format_str(input: &str) -> Result<String, LockfileError> {
    to_string(&from_str(input)?)
}

/// Emit a federation lock file as deterministic YAML.
pub fn to_string(lock: &FederationLock) -> Result<String, LockfileError> {
    validate_version(lock.version)?;
    for (id, package) in &lock.packages {
        validate_yaml_key("packages", id)?;
        validate_package(id, package)?;
    }

    let mut out = String::new();
    out.push_str(&format!("version: {}\n", lock.version));
    if lock.packages.is_empty() {
        out.push_str("packages: {}\n");
        return Ok(out);
    }
    out.push_str("packages:\n");
    for (id, package) in &lock.packages {
        out.push_str(&format!("  {}:\n", emitted_key(id)));
        out.push_str(&format!(
            "    manifest: {}\n",
            yaml_scalar(&package.manifest)
        ));
        out.push_str("    package:\n");
        out.push_str(&format!(
            "      version: {}\n",
            yaml_scalar(&package.package.version)
        ));
        out.push_str("    original:\n");
        write_original_source(&mut out, "      ", &package.original);
        out.push_str("    locked:\n");
        write_locked_source(&mut out, "      ", &package.locked);
    }
    Ok(out)
}

fn emitted_key(value: &str) -> String {
    yaml_key(value).expect("lockfile key was not prevalidated")
}

fn validate_yaml_key(section: &'static str, key: &str) -> Result<(), LockfileError> {
    if is_emittable_yaml_key(key) {
        Ok(())
    } else {
        Err(LockfileError::UnemittableYamlKey {
            section,
            key: key.to_owned(),
        })
    }
}

fn write_original_source(out: &mut String, indent: &str, source: &OriginalSource) {
    out.push_str(&format!("{indent}type: {}\n", source.source_type()));
    match source {
        OriginalSource::Path { path } => {
            out.push_str(&format!("{indent}path: {}\n", yaml_scalar(path)));
        }
        OriginalSource::Git {
            url,
            reference,
            rev,
        } => {
            out.push_str(&format!("{indent}url: {}\n", yaml_scalar(url)));
            if let Some(reference) = reference {
                out.push_str(&format!("{indent}ref: {}\n", yaml_scalar(reference)));
            }
            if let Some(rev) = rev {
                out.push_str(&format!("{indent}rev: {}\n", yaml_scalar(rev)));
            }
        }
        OriginalSource::Tarball { url } => {
            out.push_str(&format!("{indent}url: {}\n", yaml_scalar(url)));
        }
    }
}

fn write_locked_source(out: &mut String, indent: &str, source: &LockedSource) {
    out.push_str(&format!("{indent}type: {}\n", source.source_type()));
    match source {
        LockedSource::Path { path, nar_hash } => {
            out.push_str(&format!("{indent}path: {}\n", yaml_scalar(path)));
            out.push_str(&format!("{indent}nar_hash: {}\n", yaml_scalar(nar_hash)));
        }
        LockedSource::Git { url, rev, nar_hash } => {
            out.push_str(&format!("{indent}url: {}\n", yaml_scalar(url)));
            out.push_str(&format!("{indent}rev: {}\n", yaml_scalar(rev)));
            out.push_str(&format!("{indent}nar_hash: {}\n", yaml_scalar(nar_hash)));
        }
        LockedSource::Tarball { url, nar_hash } => {
            out.push_str(&format!("{indent}url: {}\n", yaml_scalar(url)));
            out.push_str(&format!("{indent}nar_hash: {}\n", yaml_scalar(nar_hash)));
        }
    }
}

#[derive(Debug)]
pub enum LockfileError {
    Parse(serde_yaml::Error),
    LegacyVersion(u32),
    UnsupportedVersion(u32),
    InvalidPackage { package: String, message: String },
    UnemittableYamlKey { section: &'static str, key: String },
}

impl fmt::Display for LockfileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(error) => write!(f, "failed to parse lock YAML: {error}"),
            Self::LegacyVersion(version) => write!(
                f,
                "federation lock version {version} is insecure and is no longer accepted; move or remove the old lock and regenerate every package with `brainbrew lock update`"
            ),
            Self::UnsupportedVersion(version) => {
                write!(f, "unsupported federation lock version {version}")
            }
            Self::InvalidPackage { package, message } => {
                write!(f, "locked package {package}: {message}")
            }
            Self::UnemittableYamlKey { section, key } => {
                write!(f, "lockfile {section} key {key:?} cannot be emitted safely")
            }
        }
    }
}

impl std::error::Error for LockfileError {}

fn validate_version(version: u32) -> Result<(), LockfileError> {
    match version {
        LOCKFILE_VERSION => Ok(()),
        1 => Err(LockfileError::LegacyVersion(1)),
        other => Err(LockfileError::UnsupportedVersion(other)),
    }
}

fn invalid_package(package: &str, message: impl Into<String>) -> LockfileError {
    LockfileError::InvalidPackage {
        package: package.to_owned(),
        message: message.into(),
    }
}

fn validate_package(package_id: &str, package: &LockedPackage) -> Result<(), LockfileError> {
    if package.manifest.is_empty() {
        return Err(invalid_package(package_id, "manifest must not be empty"));
    }
    if package.package.version.is_empty() {
        return Err(invalid_package(
            package_id,
            "package.version must not be empty",
        ));
    }
    if package.original.source_type() != package.locked.source_type() {
        return Err(invalid_package(
            package_id,
            format!(
                "original source type {} does not match locked source type {}",
                package.original.source_type(),
                package.locked.source_type()
            ),
        ));
    }
    validate_original_source(package_id, &package.original)?;
    validate_locked_source(package_id, &package.locked)
}

fn validate_original_source(
    package_id: &str,
    source: &OriginalSource,
) -> Result<(), LockfileError> {
    match source {
        OriginalSource::Path { path } => validate_nonempty(package_id, "original.path", path),
        OriginalSource::Git {
            url,
            reference,
            rev,
        } => {
            validate_nonempty(package_id, "original.url", url)?;
            validate_remote_url(package_id, "original.url", url)?;
            if reference.is_some() && rev.is_some() {
                return Err(invalid_package(
                    package_id,
                    "original git source must not contain both ref and rev",
                ));
            }
            if let Some(reference) = reference {
                validate_nonempty(package_id, "original.ref", reference)?;
            }
            if let Some(rev) = rev {
                validate_nonempty(package_id, "original.rev", rev)?;
            }
            Ok(())
        }
        OriginalSource::Tarball { url } => {
            validate_nonempty(package_id, "original.url", url)?;
            validate_archive_url(package_id, "original.url", url)
        }
    }
}

fn validate_locked_source(package_id: &str, source: &LockedSource) -> Result<(), LockfileError> {
    match source {
        LockedSource::Path { path, nar_hash } => {
            validate_nonempty(package_id, "locked.path", path)?;
            validate_nar_hash(package_id, nar_hash)
        }
        LockedSource::Git { url, rev, nar_hash } => {
            validate_nonempty(package_id, "locked.url", url)?;
            validate_remote_url(package_id, "locked.url", url)?;
            if !is_full_git_commit(rev) {
                return Err(invalid_package(
                    package_id,
                    "locked.rev must be a full lowercase 40-character Git commit",
                ));
            }
            validate_nar_hash(package_id, nar_hash)
        }
        LockedSource::Tarball { url, nar_hash } => {
            validate_nonempty(package_id, "locked.url", url)?;
            validate_archive_url(package_id, "locked.url", url)?;
            validate_nar_hash(package_id, nar_hash)
        }
    }
}

fn validate_remote_url(package_id: &str, field: &str, value: &str) -> Result<(), LockfileError> {
    if value.starts_with("https://") {
        Ok(())
    } else {
        Err(invalid_package(
            package_id,
            format!("{field} must use HTTPS; HTTP and other remote schemes are forbidden"),
        ))
    }
}

fn validate_archive_url(package_id: &str, field: &str, value: &str) -> Result<(), LockfileError> {
    if value.starts_with("https://") || value.starts_with("file://") || !value.contains("://") {
        Ok(())
    } else {
        Err(invalid_package(
            package_id,
            format!("{field} must use HTTPS or an explicit local file path"),
        ))
    }
}

fn validate_nonempty(package_id: &str, field: &str, value: &str) -> Result<(), LockfileError> {
    if value.is_empty() {
        Err(invalid_package(
            package_id,
            format!("{field} must not be empty"),
        ))
    } else {
        Ok(())
    }
}

fn validate_nar_hash(package_id: &str, value: &str) -> Result<(), LockfileError> {
    validate_sha256_sri(value)
        .map_err(|reason| invalid_package(package_id, format!("locked.nar_hash {reason}")))
}

/// Validate exact canonical SRI spelling for one 32-byte SHA-256 digest.
pub fn validate_sha256_sri(value: &str) -> Result<(), &'static str> {
    let Some(encoded) = value.strip_prefix("sha256-") else {
        return Err("must be a canonical SRI SHA-256 string (`sha256-<base64>`)");
    };
    let decoded = BASE64_STANDARD
        .decode(encoded)
        .map_err(|_| "must be a canonical SRI SHA-256 string (`sha256-<base64>`)")?;
    if decoded.len() != 32 || BASE64_STANDARD.encode(&decoded) != encoded {
        return Err("must be a canonical SRI SHA-256 string (`sha256-<base64>`)");
    }
    Ok(())
}

/// Whether a revision is an immutable full GitHub SHA-1 commit id.
pub fn is_full_git_commit(value: &str) -> bool {
    value.len() == 40
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[derive(Deserialize)]
struct LockVersionYaml {
    version: u32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FederationLockYaml {
    version: u32,
    #[serde(default)]
    packages: BTreeMap<String, LockedPackageYaml>,
}

impl FederationLockYaml {
    fn into_lock(self) -> Result<FederationLock, LockfileError> {
        validate_version(self.version)?;
        let packages = self
            .packages
            .into_iter()
            .map(|(id, package)| {
                validate_yaml_key("packages", &id)?;
                let package = package.into_locked_package();
                validate_package(&id, &package)?;
                Ok((id, package))
            })
            .collect::<Result<_, LockfileError>>()?;
        Ok(FederationLock {
            version: self.version,
            packages,
        })
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LockedPackageYaml {
    manifest: String,
    package: LockedPackageMetadataYaml,
    original: OriginalSourceYaml,
    locked: LockedSourceYaml,
}

impl LockedPackageYaml {
    fn into_locked_package(self) -> LockedPackage {
        LockedPackage {
            manifest: self.manifest,
            package: self.package.into_metadata(),
            original: self.original.into_source(),
            locked: self.locked.into_source(),
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LockedPackageMetadataYaml {
    version: String,
}

impl LockedPackageMetadataYaml {
    fn into_metadata(self) -> LockedPackageMetadata {
        LockedPackageMetadata {
            version: self.version,
        }
    }
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "lowercase", deny_unknown_fields)]
enum OriginalSourceYaml {
    Path {
        path: String,
    },
    Git {
        url: String,
        #[serde(default, rename = "ref")]
        reference: Option<String>,
        #[serde(default)]
        rev: Option<String>,
    },
    Tarball {
        url: String,
    },
}

impl OriginalSourceYaml {
    fn into_source(self) -> OriginalSource {
        match self {
            Self::Path { path } => OriginalSource::Path { path },
            Self::Git {
                url,
                reference,
                rev,
            } => OriginalSource::Git {
                url,
                reference,
                rev,
            },
            Self::Tarball { url } => OriginalSource::Tarball { url },
        }
    }
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "lowercase", deny_unknown_fields)]
enum LockedSourceYaml {
    Path {
        path: String,
        nar_hash: String,
    },
    Git {
        url: String,
        rev: String,
        nar_hash: String,
    },
    Tarball {
        url: String,
        nar_hash: String,
    },
}

impl LockedSourceYaml {
    fn into_source(self) -> LockedSource {
        match self {
            Self::Path { path, nar_hash } => LockedSource::Path { path, nar_hash },
            Self::Git { url, rev, nar_hash } => LockedSource::Git { url, rev, nar_hash },
            Self::Tarball { url, nar_hash } => LockedSource::Tarball { url, nar_hash },
        }
    }
}
