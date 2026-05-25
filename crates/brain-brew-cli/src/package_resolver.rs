use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use brain_brew_formats::manifest;

/// Discover local Federated Deck package manifests and validate their package dependencies.
pub(crate) fn discover_package_manifests(roots: &[PathBuf]) -> Result<Vec<PathBuf>, String> {
    let mut paths = BTreeSet::new();
    for root in roots {
        collect_manifests(root, &mut paths)?;
    }
    Ok(paths.into_iter().collect())
}

pub(crate) fn validate_package_dependencies(
    packages: &[(&PathBuf, &manifest::FederatedDeckManifest)],
) -> Result<(), String> {
    let mut by_id = BTreeMap::new();
    for (path, manifest) in packages {
        let Some(package) = &manifest.package else {
            continue;
        };
        if let Some(previous) = by_id.insert(package.id.clone(), (*path, package.version.clone())) {
            return Err(format!(
                "duplicate package id {} in {} and {}",
                package.id,
                previous.0.display(),
                path.display()
            ));
        }
    }

    for (path, manifest) in packages {
        let Some(package) = &manifest.package else {
            continue;
        };
        for dependency in &package.depends_on {
            let (dependency_id, expected_version) = parse_dependency(dependency);
            let Some((dependency_path, actual_version)) = by_id.get(dependency_id) else {
                return Err(format!(
                    "package dependency {dependency_id} required by {} in {} was not found",
                    package.id,
                    path.display()
                ));
            };
            if let Some(expected_version) = expected_version
                && actual_version != expected_version
            {
                return Err(format!(
                    "package dependency {dependency_id}@{expected_version} required by {} in {} resolved to version {} in {}",
                    package.id,
                    path.display(),
                    actual_version,
                    dependency_path.display()
                ));
            }
        }
    }

    Ok(())
}

fn collect_manifests(root: &Path, paths: &mut BTreeSet<PathBuf>) -> Result<(), String> {
    let metadata = fs::metadata(root).map_err(|error| format!("{}: {error}", root.display()))?;
    if metadata.is_file() {
        if root.file_name().and_then(|name| name.to_str()) == Some("brainbrew.yaml") {
            paths.insert(root.to_path_buf());
        }
        return Ok(());
    }

    let manifest = root.join("brainbrew.yaml");
    if manifest.exists() {
        paths.insert(manifest);
    }

    for entry in fs::read_dir(root).map_err(|error| format!("{}: {error}", root.display()))? {
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path();
        if path.is_dir() {
            collect_manifests(&path, paths)?;
        }
    }

    Ok(())
}

fn parse_dependency(dependency: &str) -> (&str, Option<&str>) {
    dependency
        .split_once('@')
        .map_or((dependency, None), |(id, version)| (id, Some(version)))
}
