use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use brain_brew_formats::manifest;
use brain_brew_formats::package_semver;

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
        if let Some(previous) = by_id.insert(package.id.clone(), (*path, package)) {
            let conflict = if previous.1.version == package.version {
                "duplicate package identity"
            } else {
                "conflicting package versions"
            };
            return Err(format!(
                "{conflict} {}: {} in {} and {} in {}",
                package.id,
                previous.1.version,
                previous.0.display(),
                package.version,
                path.display()
            ));
        }
    }

    let mut graph = BTreeMap::<String, Vec<DependencyEdge>>::new();
    for (path, manifest) in packages {
        let Some(package) = &manifest.package else {
            continue;
        };
        let mut dependencies = Vec::new();
        for declaration in &package.depends_on {
            let dependency = package_semver::parse_exact_dependency(declaration).map_err(|reason| {
                format!(
                    "invalid package dependency {declaration:?} declared by {}@{} in {}: {reason}",
                    package.id,
                    package.version,
                    path.display()
                )
            })?;
            let Some((dependency_path, dependency_package)) = by_id.get(&dependency.id) else {
                return Err(format!(
                    "package dependency {}@{} required by {}@{} in {} was not found",
                    dependency.id,
                    dependency.version,
                    package.id,
                    package.version,
                    path.display()
                ));
            };
            if dependency_package.version != dependency.version {
                return Err(format!(
                    "package dependency {}@{} required by {}@{} in {} resolved to version {} in {}",
                    dependency.id,
                    dependency.version,
                    package.id,
                    package.version,
                    path.display(),
                    dependency_package.version,
                    dependency_path.display()
                ));
            }
            dependencies.push(DependencyEdge {
                target: dependency.id,
                declaration: declaration.clone(),
            });
        }

        if let Some(base_package) = &package.base_package {
            let Some((base_path, base)) = by_id.get(base_package) else {
                // This should also be diagnosed by the exact dependency pass,
                // but retain a specific fail-closed guard for constructed data.
                return Err(format!(
                    "base package {base_package} required by {}@{} in {} was not found",
                    package.id,
                    package.version,
                    path.display()
                ));
            };
            let compatible = package_semver::requirements_match(
                &package.compatible_base_versions,
                &base.version,
            )
            .map_err(|error| {
                format!(
                    "invalid compatible_base_versions declared by {}@{} in {}: {error}",
                    package.id,
                    package.version,
                    path.display()
                )
            })?;
            if !compatible {
                return Err(format!(
                    "base package {}@{} in {} is incompatible with {}@{} in {}; compatible_base_versions requires one of [{}]",
                    base.id,
                    base.version,
                    base_path.display(),
                    package.id,
                    package.version,
                    path.display(),
                    package.compatible_base_versions.join(" OR ")
                ));
            }
        }
        graph.insert(package.id.clone(), dependencies);
    }

    let mut complete = BTreeSet::new();
    let mut active = Vec::new();
    for package in graph.keys() {
        visit_package(package, &graph, &by_id, &mut complete, &mut active)?;
    }
    Ok(())
}

#[derive(Clone)]
struct DependencyEdge {
    target: String,
    declaration: String,
}

fn visit_package(
    package: &str,
    graph: &BTreeMap<String, Vec<DependencyEdge>>,
    packages: &BTreeMap<String, (&PathBuf, &manifest::PackageMetadata)>,
    complete: &mut BTreeSet<String>,
    active: &mut Vec<(String, Option<String>)>,
) -> Result<(), String> {
    if complete.contains(package) {
        return Ok(());
    }
    if let Some(index) = active
        .iter()
        .position(|(candidate, _)| candidate == package)
    {
        let cycle = &active[index..];
        let mut trace = String::from("package dependency cycle:");
        for (id, incoming) in cycle {
            let (path, metadata) = packages[id];
            trace.push_str(&format!(
                "\n  {}@{} ({})",
                metadata.id,
                metadata.version,
                path.display()
            ));
            let edge = incoming
                .as_ref()
                .expect("a dependency cycle has an outgoing edge");
            trace.push_str(&format!(" --depends_on {edge}-->"));
        }
        let (path, metadata) = packages[package];
        trace.push_str(&format!(
            "\n  {}@{} ({})",
            metadata.id,
            metadata.version,
            path.display()
        ));
        return Err(trace);
    }
    active.push((package.to_owned(), None));
    for dependency in graph.get(package).into_iter().flatten() {
        if let Some(last) = active.last_mut() {
            last.1 = Some(dependency.declaration.clone());
        }
        visit_package(&dependency.target, graph, packages, complete, active)?;
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
