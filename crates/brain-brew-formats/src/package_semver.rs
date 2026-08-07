//! Pure Semantic Version parsing used by manifest package metadata.
//!
//! Package dependency pins are exact versions. Compatibility requirements use
//! the `semver` crate grammar: commas join comparators with AND, while separate
//! manifest list entries are alternatives joined with OR.

use std::fmt;

use semver::{Version, VersionReq};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExactDependency {
    pub id: String,
    pub version: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackageSemverError(String);

impl fmt::Display for PackageSemverError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for PackageSemverError {}

pub fn canonical_version(value: &str) -> Result<String, PackageSemverError> {
    Version::parse(value)
        .map(|version| version.to_string())
        .map_err(|error| PackageSemverError(format!("expected a Semantic Version: {error}")))
}

pub fn parse_exact_dependency(value: &str) -> Result<ExactDependency, PackageSemverError> {
    if value.matches('@').count() != 1 {
        return Err(PackageSemverError(
            "expected an exact dependency pin <package-id>@<SemVer>".to_owned(),
        ));
    }
    let (id, version) = value.split_once('@').expect("separator count checked");
    validate_package_id(id)?;
    let version = canonical_version(version).map_err(|error| {
        PackageSemverError(format!(
            "expected an exact dependency pin <package-id>@<SemVer>: {error}"
        ))
    })?;
    Ok(ExactDependency {
        id: id.to_owned(),
        version,
    })
}

pub fn canonical_requirement(value: &str) -> Result<String, PackageSemverError> {
    if value.trim().is_empty() {
        return Err(PackageSemverError(
            "Semantic Version requirement must not be empty".to_owned(),
        ));
    }
    VersionReq::parse(value)
        .map(|requirement| requirement.to_string())
        .map_err(|error| {
            PackageSemverError(format!("expected a Semantic Version requirement: {error}"))
        })
}

pub fn requirements_match(
    requirements: &[String],
    version: &str,
) -> Result<bool, PackageSemverError> {
    let version = Version::parse(version).map_err(|error| {
        PackageSemverError(format!("expected a Semantic Version to match: {error}"))
    })?;
    requirements
        .iter()
        .map(|requirement| {
            VersionReq::parse(requirement)
                .map(|requirement| requirement.matches(&version))
                .map_err(|error| {
                    PackageSemverError(format!(
                        "invalid Semantic Version requirement {requirement:?}: {error}"
                    ))
                })
        })
        .try_fold(false, |matched, current| Ok(matched || current?))
}

pub fn validate_package_id(value: &str) -> Result<(), PackageSemverError> {
    if value.is_empty() {
        return Err(PackageSemverError(
            "package ID must not be empty".to_owned(),
        ));
    }
    if value
        .chars()
        .any(|character| character.is_whitespace() || matches!(character, '@' | ':'))
    {
        return Err(PackageSemverError(
            "package ID must not contain whitespace, '@', or ':'".to_owned(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requirement_lists_are_or_and_commas_are_and() {
        let requirements = vec![">=1.2.0, <2.0.0".to_owned(), "=3.0.0".to_owned()];
        assert!(requirements_match(&requirements, "1.5.0").unwrap());
        assert!(requirements_match(&requirements, "3.0.0").unwrap());
        assert!(!requirements_match(&requirements, "2.5.0").unwrap());
    }

    #[test]
    fn prereleases_require_an_explicit_matching_prerelease_comparator() {
        assert!(!requirements_match(&[">=1.0.0".to_owned()], "2.0.0-alpha.1").unwrap());
        assert!(
            requirements_match(&[">=2.0.0-alpha.1, <2.0.0".to_owned()], "2.0.0-alpha.2").unwrap()
        );
    }
}
