//! Reusable format codecs for Brain Brew.
//!
//! This crate contains strict CanonicalDeck YAML support, CrowdAnki codecs, and
//! media helpers. It depends on `brain-brew-core`, but does not own domain
//! behavior.

pub mod canonical_source_document;
pub mod canonical_yaml;
pub mod crowdanki;
pub mod lockfile;
pub mod manifest;
pub mod media;
pub mod media_map;
pub mod note_type_map;
pub mod overlay_source_document;
pub mod package_semver;
pub mod safe_relative_path;
pub mod source_document;
pub mod source_includes;
pub mod strict_yaml;
pub mod yaml_scalar;

pub use brain_brew_core as core;

/// Name of the formats crate.
pub const CRATE_NAME: &str = env!("CARGO_PKG_NAME");

#[cfg(test)]
mod tests {
    use super::{CRATE_NAME, core};

    #[test]
    fn exposes_formats_crate_name() {
        assert_eq!(CRATE_NAME, "brain-brew-formats");
    }

    #[test]
    fn can_reach_core_crate() {
        assert_eq!(core::CRATE_NAME, "brain-brew-core");
    }
}
