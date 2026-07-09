use std::collections::BTreeMap;
use std::fmt::{self, Write as _};

use brain_brew_core::{InvalidStableId, MediaReference, StableId};
use serde::Deserialize;

use crate::yaml_scalar::{key as yaml_key, scalar as yaml_scalar};

/// Parse a standalone media-map source file.
///
/// The root mapping is the same shape as the canonical deck's inline `media:`
/// section: stable media ID keys mapped to `{ path, sha256 }` values.
pub fn from_str(input: &str) -> Result<BTreeMap<StableId, MediaReference>, MediaMapYamlError> {
    crate::strict_yaml::reject_duplicate_keys(input).map_err(MediaMapYamlError::Parse)?;
    let file: BTreeMap<String, MediaYaml> =
        serde_yaml::from_str(input).map_err(MediaMapYamlError::Parse)?;
    file.into_iter()
        .map(|(id, media)| {
            let stable_id = StableId::new(id)?;
            Ok((stable_id.clone(), media.into_media(stable_id)))
        })
        .collect()
}

/// Parse and re-emit a standalone media-map source file using deterministic formatting.
pub fn format_str(input: &str) -> Result<String, MediaMapYamlError> {
    let media = from_str(input)?;
    Ok(to_string(&media))
}

/// Emit a standalone media-map source file using the canonical inline media ordering and scalar rules.
pub fn to_string(media: &BTreeMap<StableId, MediaReference>) -> String {
    if media.is_empty() {
        return "{}\n".to_owned();
    }

    let mut out = String::new();
    for (id, media) in media {
        writeln!(
            out,
            "{}:",
            yaml_key(id.as_str()).expect("stable id is an emittable key")
        )
        .expect("writing to a string cannot fail");
        writeln!(out, "  path: {}", yaml_scalar(&media.path))
            .expect("writing to a string cannot fail");
        writeln!(out, "  sha256: {}", yaml_scalar(&media.sha256))
            .expect("writing to a string cannot fail");
    }
    out
}

#[derive(Debug)]
pub enum MediaMapYamlError {
    Parse(serde_yaml::Error),
    StableId(InvalidStableId),
}

impl fmt::Display for MediaMapYamlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(error) => write!(f, "failed to parse media map YAML: {error}"),
            Self::StableId(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for MediaMapYamlError {}

impl From<InvalidStableId> for MediaMapYamlError {
    fn from(error: InvalidStableId) -> Self {
        Self::StableId(error)
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MediaYaml {
    path: String,
    sha256: String,
}

impl MediaYaml {
    fn into_media(self, id: StableId) -> MediaReference {
        MediaReference {
            id,
            path: self.path,
            sha256: self.sha256,
        }
    }
}
