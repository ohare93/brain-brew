use std::collections::BTreeMap;
use std::fmt;

use serde::Deserialize;

use crate::yaml_scalar::{
    is_emittable_key as is_emittable_yaml_key, key as yaml_key, scalar as yaml_scalar,
};

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
    pub original: Option<LockedSource>,
    pub locked: LockedSource,
}

/// Package metadata captured at lock time.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LockedPackageMetadata {
    pub version: String,
}

/// Original or locked source reference for one package input.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LockedSource {
    pub source_type: String,
    pub url: Option<String>,
    pub path: Option<String>,
    pub reference: Option<String>,
    pub rev: Option<String>,
    pub nar_hash: Option<String>,
}

/// Parse a federation lock file from strict YAML.
pub fn from_str(input: &str) -> Result<FederationLock, LockfileError> {
    crate::strict_yaml::reject_duplicate_keys(input).map_err(LockfileError::Parse)?;
    let yaml: FederationLockYaml = serde_yaml::from_str(input).map_err(LockfileError::Parse)?;
    yaml.into_lock()
}

/// Parse and re-emit a federation lock file using deterministic formatting.
pub fn format_str(input: &str) -> Result<String, LockfileError> {
    Ok(to_string(&from_str(input)?))
}

/// Emit a federation lock file as deterministic YAML.
pub fn to_string(lock: &FederationLock) -> String {
    let mut out = String::new();
    out.push_str(&format!("version: {}\n", lock.version));
    if lock.packages.is_empty() {
        out.push_str("packages: {}\n");
        return out;
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
        if let Some(original) = &package.original {
            out.push_str("    original:\n");
            write_source(&mut out, "      ", original);
        }
        out.push_str("    locked:\n");
        write_source(&mut out, "      ", &package.locked);
    }
    out
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

fn write_source(out: &mut String, indent: &str, source: &LockedSource) {
    out.push_str(&format!(
        "{indent}type: {}\n",
        yaml_scalar(&source.source_type)
    ));
    if let Some(url) = &source.url {
        out.push_str(&format!("{indent}url: {}\n", yaml_scalar(url)));
    }
    if let Some(path) = &source.path {
        out.push_str(&format!("{indent}path: {}\n", yaml_scalar(path)));
    }
    if let Some(reference) = &source.reference {
        out.push_str(&format!("{indent}ref: {}\n", yaml_scalar(reference)));
    }
    if let Some(rev) = &source.rev {
        out.push_str(&format!("{indent}rev: {}\n", yaml_scalar(rev)));
    }
    if let Some(nar_hash) = &source.nar_hash {
        out.push_str(&format!("{indent}nar_hash: {}\n", yaml_scalar(nar_hash)));
    }
}

#[derive(Debug)]
pub enum LockfileError {
    Parse(serde_yaml::Error),
    UnsupportedVersion(u32),
    MissingLockedSource(String),
    UnemittableYamlKey { section: &'static str, key: String },
}

impl fmt::Display for LockfileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(error) => write!(f, "failed to parse lock YAML: {error}"),
            Self::UnsupportedVersion(version) => {
                write!(f, "unsupported federation lock version {version}")
            }
            Self::MissingLockedSource(package) => {
                write!(f, "locked package {package} must include a locked source")
            }
            Self::UnemittableYamlKey { section, key } => {
                write!(f, "lockfile {section} key {key:?} cannot be emitted safely")
            }
        }
    }
}

impl std::error::Error for LockfileError {}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FederationLockYaml {
    version: u32,
    #[serde(default)]
    packages: BTreeMap<String, LockedPackageYaml>,
}

impl FederationLockYaml {
    fn into_lock(self) -> Result<FederationLock, LockfileError> {
        if self.version != 1 {
            return Err(LockfileError::UnsupportedVersion(self.version));
        }
        let packages = self
            .packages
            .into_iter()
            .map(|(id, package)| {
                validate_yaml_key("packages", &id)?;
                Ok((id.clone(), package.into_locked_package(id)?))
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
    #[serde(default)]
    original: Option<LockedSourceYaml>,
    #[serde(default)]
    locked: Option<LockedSourceYaml>,
}

impl LockedPackageYaml {
    fn into_locked_package(self, id: String) -> Result<LockedPackage, LockfileError> {
        let Some(locked) = self.locked else {
            return Err(LockfileError::MissingLockedSource(id));
        };
        Ok(LockedPackage {
            manifest: self.manifest,
            package: self.package.into_metadata(),
            original: self.original.map(LockedSourceYaml::into_source),
            locked: locked.into_source(),
        })
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
#[serde(deny_unknown_fields)]
struct LockedSourceYaml {
    #[serde(rename = "type")]
    source_type: String,
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    path: Option<String>,
    #[serde(default, rename = "ref")]
    reference: Option<String>,
    #[serde(default)]
    rev: Option<String>,
    #[serde(default)]
    nar_hash: Option<String>,
}

impl LockedSourceYaml {
    fn into_source(self) -> LockedSource {
        LockedSource {
            source_type: self.source_type,
            url: self.url,
            path: self.path,
            reference: self.reference,
            rev: self.rev,
            nar_hash: self.nar_hash,
        }
    }
}
