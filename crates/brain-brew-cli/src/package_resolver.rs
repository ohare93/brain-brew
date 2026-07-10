use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use brain_brew_formats::manifest;

use crate::package_tree;

/// Discover local Federated Deck package manifests and validate their package dependencies.
pub(crate) fn discover_package_manifests(roots: &[PathBuf]) -> Result<Vec<PathBuf>, String> {
    let mut paths = BTreeSet::new();
    let mut visited = BTreeSet::new();
    for root in roots {
        collect_manifests(root, &mut paths, &mut visited)?;
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
            let conflict = if previous.1 == package.version {
                "duplicate package identity"
            } else {
                "conflicting package versions"
            };
            return Err(format!(
                "{conflict} {}: {} in {} and {} in {}",
                package.id,
                previous.1,
                previous.0.display(),
                package.version,
                path.display()
            ));
        }
    }

    let mut graph = BTreeMap::<String, Vec<String>>::new();
    for (path, manifest) in packages {
        let Some(package) = &manifest.package else {
            continue;
        };
        let mut dependencies = Vec::new();
        for dependency in &package.depends_on {
            let (dependency_id, expected_version) =
                parse_dependency(dependency).map_err(|reason| {
                    format!(
                        "invalid package dependency {dependency:?} declared by {} in {}: {reason}",
                        package.id,
                        path.display()
                    )
                })?;
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
            dependencies.push(dependency_id.to_owned());
        }
        graph.insert(package.id.clone(), dependencies);
    }

    let mut complete = BTreeSet::new();
    let mut active = Vec::new();
    for package in graph.keys() {
        visit_package(package, &graph, &mut complete, &mut active)?;
    }
    Ok(())
}

fn visit_package(
    package: &str,
    graph: &BTreeMap<String, Vec<String>>,
    complete: &mut BTreeSet<String>,
    active: &mut Vec<String>,
) -> Result<(), String> {
    if complete.contains(package) {
        return Ok(());
    }
    if let Some(index) = active.iter().position(|candidate| candidate == package) {
        let mut cycle = active[index..].to_vec();
        cycle.push(package.to_owned());
        return Err(format!("package dependency cycle: {}", cycle.join(" -> ")));
    }
    active.push(package.to_owned());
    for dependency in graph.get(package).into_iter().flatten() {
        visit_package(dependency, graph, complete, active)?;
    }
    active.pop();
    complete.insert(package.to_owned());
    Ok(())
}

fn collect_manifests(
    root: &Path,
    paths: &mut BTreeSet<PathBuf>,
    visited: &mut BTreeSet<PathBuf>,
) -> Result<(), String> {
    let metadata =
        fs::symlink_metadata(root).map_err(|error| format!("{}: {error}", root.display()))?;
    if metadata.file_type().is_symlink() {
        return Err(format!(
            "package discovery rejected symlink {}",
            root.display()
        ));
    }
    if metadata.is_file() {
        if root.file_name().and_then(|name| name.to_str()) == Some("brainbrew.yaml") {
            paths.insert(
                fs::canonicalize(root).map_err(|error| format!("{}: {error}", root.display()))?,
            );
        }
        return Ok(());
    }
    let canonical =
        fs::canonicalize(root).map_err(|error| format!("{}: {error}", root.display()))?;
    if !visited.insert(canonical) {
        return Ok(());
    }

    let manifest = root.join("brainbrew.yaml");
    if manifest.is_file() {
        paths.insert(
            fs::canonicalize(&manifest)
                .map_err(|error| format!("{}: {error}", manifest.display()))?,
        );
    }

    let mut entries = fs::read_dir(root)
        .map_err(|error| format!("{}: {error}", root.display()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    entries.sort_by_key(fs::DirEntry::file_name);
    for entry in entries {
        let name = entry.file_name();
        if package_tree::should_skip(&name.to_string_lossy()) {
            continue;
        }
        let path = entry.path();
        let metadata =
            fs::symlink_metadata(&path).map_err(|error| format!("{}: {error}", path.display()))?;
        if metadata.file_type().is_symlink() {
            return Err(format!(
                "package discovery rejected symlink {}",
                path.display()
            ));
        }
        if metadata.is_dir() {
            collect_manifests(&path, paths, visited)?;
        }
    }
    Ok(())
}

fn parse_dependency(dependency: &str) -> Result<(&str, Option<&str>), &'static str> {
    if dependency.is_empty() {
        return Err("dependency ID is empty");
    }
    let (id, version) = dependency
        .split_once('@')
        .map_or((dependency, None), |(id, version)| (id, Some(version)));
    if id.is_empty() {
        return Err("dependency ID is empty");
    }
    if id.chars().any(char::is_whitespace) {
        return Err("dependency ID contains whitespace");
    }
    if dependency.matches('@').count() > 1 {
        return Err("dependency contains more than one @ separator");
    }
    if version.is_some_and(str::is_empty) {
        return Err("dependency version is empty");
    }
    Ok((id, version))
}

#[cfg(test)]
mod tests {
    use super::parse_dependency;

    #[test]
    fn dependency_parser_rejects_ambiguous_specs() {
        for invalid in ["", "@1", "pkg@", "pkg@1@2", "bad id@1"] {
            assert!(parse_dependency(invalid).is_err(), "{invalid}");
        }
        assert_eq!(parse_dependency("pkg").unwrap(), ("pkg", None));
        assert_eq!(parse_dependency("pkg@1").unwrap(), ("pkg", Some("1")));
    }
}
